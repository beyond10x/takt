//! story:local-forge-merges-into-a-target-branch, second adversary pass: a request whose head the
//! hub's base already holds is not merged again. Two ways reach it: a run killed between the
//! compare-and-swap of the base and the write of the record (the record still says open), and an
//! orchestrator that merged the head by hand before a takt run (the story's "What is wrong").
//! Either way the next `merge_with` must leave the base where it is, not stack an empty merge
//! commit on it that moves every other run's compare-and-swap.

#[path = "support/own_home.rs"]
mod own_home;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use takt::forge::local::Local;
use takt::forge::{Forge, MergeMessage, PullRequest};
use takt::git::CommitIdentity;

fn git(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args([
            "-c",
            "commit.gpgsign=false",
            "-c",
            "core.hooksPath=/dev/null",
        ])
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .current_dir(dir)
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {args:?} in {}: {}",
        dir.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("utf-8")
        .trim()
        .to_owned()
}

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("local-forge-adv-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("a scratch directory");
        Self(dir)
    }

    fn hub(&self) -> PathBuf {
        let seed = self.0.join("seed");
        fs::create_dir_all(&seed).expect("a directory");
        git(&seed, &["init", "-q", "-b", "integration"]);
        fs::write(seed.join("README.md"), "one\n").expect("a file");
        git(&seed, &["add", "-A"]);
        git(&seed, &["commit", "-q", "-m", "start"]);
        git(&self.0, &["clone", "-q", "--bare", "seed", "hub.git"]);
        fs::remove_dir_all(&seed).expect("the seed is removed");
        self.0.join("hub.git")
    }

    fn clone(&self, name: &str) -> PathBuf {
        git(&self.0, &["clone", "-q", "hub.git", name]);
        self.0.join(name)
    }

    /// A clone that shares no object file with the hub, as `--forge local` requires of a tree.
    fn clone_no_local(&self, name: &str) -> PathBuf {
        git(&self.0, &["clone", "-q", "--no-local", "hub.git", name]);
        self.0.join(name)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn local(scratch: &Scratch, tree: &Path) -> Local {
    Local::for_tree(
        tree,
        vec!["true".to_owned()],
        scratch.0.join("state"),
        CommitIdentity::new("Test Bot", "bot@example.invalid").expect("an identity"),
    )
    .expect("a hub")
}

/// A change on `takt/a` in `tree`, pushed and its request opened into `integration`: the head
/// and the number.
fn delivered(forge: &Local, tree: &Path, artifacts: &Path) -> (String, u64) {
    git(tree, &["checkout", "-q", "-b", "takt/a"]);
    fs::write(tree.join("a.txt"), "a\n").expect("a file");
    git(tree, &["add", "-A"]);
    git(tree, &["commit", "-q", "-m", "a"]);
    let head = git(tree, &["rev-parse", "HEAD"]);
    forge.push(tree, "takt/a").expect("pushed");
    let number = forge
        .open_pull_request(
            tree,
            artifacts,
            &PullRequest {
                branch: "takt/a".to_owned(),
                base: "integration".to_owned(),
                title: "deliver a".to_owned(),
                body: String::new(),
            },
        )
        .expect("opened");
    (head, number)
}

fn message() -> MergeMessage {
    MergeMessage {
        title: "merge a".to_owned(),
        body: String::new(),
    }
}

/// A run killed after the compare-and-swap moved the base and before the record said merged:
/// the next run reads the request open, judges the base, which holds the head, and merges.
/// The base must stay at the merge already made.
#[test]
fn a_merge_retried_after_the_record_was_not_written_does_not_merge_again() {
    let scratch = Scratch::new("retry");
    let hub = scratch.hub();
    let a = scratch.clone_no_local("a");
    let forge = local(&scratch, &a);
    let artifacts = scratch.0.join("artifacts");
    let (head, number) = delivered(&forge, &a, &artifacts);
    let record = hub.join(format!("takt/requests/{number}.json"));
    let open = fs::read(&record).expect("the record");

    forge.target_tip(&a, "integration").expect("a tip");
    forge
        .merge_with(&a, &artifacts, number, &head, &message())
        .expect("merged");
    let merged = git(&hub, &["rev-parse", "refs/heads/integration"]);
    // The write of the record never happened: the process died after `update-ref`.
    fs::write(&record, &open).expect("the record as it was before the merge");

    let next = local(&scratch, &a);
    next.target_tip(&a, "integration").expect("a tip");
    let _ = next.merge_with(&a, &artifacts, number, &head, &message());
    assert_eq!(
        git(&hub, &["rev-parse", "refs/heads/integration"]),
        merged,
        "the base already holds the head through {merged}; a second merge stacked an empty merge commit on it: {}",
        git(&hub, &["log", "--oneline", "-3", "refs/heads/integration"])
    );
}

/// A tree made the way the story and the unit's own tests make it, `git clone <hub> <tree>`,
/// hard-links the hub's object files into the tree's `.git/objects`: a step that can write the
/// tree's git directory writes the hub's objects in place. The first pass refused this for the
/// checks' repository; the tree is accepted with it.
#[test]
fn a_tree_sharing_object_files_with_the_hub_is_refused() {
    use std::os::unix::fs::MetadataExt as _;
    let scratch = Scratch::new("hard-links");
    let _hub = scratch.hub();
    let a = scratch.clone("a");
    let mut shared = Vec::new();
    let mut stack = vec![a.join(".git/objects")];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).expect("a directory") {
            let path = entry.expect("an entry").path();
            let meta = fs::symlink_metadata(&path).expect("metadata");
            if meta.is_dir() {
                stack.push(path);
            } else if meta.nlink() > 1 {
                shared.push(path);
            }
        }
    }
    assert!(
        !shared.is_empty(),
        "the fixture: a local clone hard-links the hub's objects"
    );
    let accepted = Local::for_tree(
        &a,
        vec!["true".to_owned()],
        scratch.0.join("state"),
        CommitIdentity::new("Test Bot", "bot@example.invalid").expect("an identity"),
    );
    assert!(
        accepted.is_err(),
        "a tree whose object files are the hub's ({} shared) was accepted as a clone outside the hub",
        shared.len()
    );
}

/// The orchestrator merged the head into the hub's base by hand, as the story's "What is wrong"
/// says it does today; a takt run on the request afterwards finds the request open and the head
/// already on the base. The base must stay at the hand merge.
#[test]
fn a_head_merged_by_hand_is_not_merged_again() {
    let scratch = Scratch::new("by-hand");
    let hub = scratch.hub();
    let a = scratch.clone_no_local("a");
    let forge = local(&scratch, &a);
    let artifacts = scratch.0.join("artifacts");
    let (head, number) = delivered(&forge, &a, &artifacts);

    let orchestrator = scratch.clone("orchestrator");
    git(&orchestrator, &["fetch", "-q", "origin", "takt/a"]);
    git(
        &orchestrator,
        &["merge", "-q", "--no-ff", "--no-edit", "FETCH_HEAD"],
    );
    git(
        &orchestrator,
        &["push", "-q", "origin", "HEAD:refs/heads/integration"],
    );
    let by_hand = git(&hub, &["rev-parse", "refs/heads/integration"]);

    forge.target_tip(&a, "integration").expect("a tip");
    let _ = forge.merge_with(&a, &artifacts, number, &head, &message());
    assert_eq!(
        git(&hub, &["rev-parse", "refs/heads/integration"]),
        by_hand,
        "the base already holds the head through the hand merge {by_hand}; takt stacked an empty merge commit on it: {}",
        git(&hub, &["log", "--oneline", "-3", "refs/heads/integration"])
    );
}
