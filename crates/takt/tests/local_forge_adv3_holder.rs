//! story:local-forge-merges-into-a-target-branch, third adversary pass. Three ways the local
//! forge's own records disagree with what it believes or runs:
//!
//! - a head the base already holds through a commit built on it (a hand merge of a branch with
//!   a fixup on top of the head) is recorded merged by `merge_with`, and the record it wrote is
//!   then not believed by `pull_request`, so the case can never read its request again;
//! - a tree whose `.git/objects` is a link to the hub's object directory shares every object file
//!   with the hub and is not refused;
//! - a pass recorded for a head under one `local_checks` is read as the pass of another
//!   `local_checks`, which never ran.

#[path = "support/own_home.rs"]
mod own_home;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use takt::forge::local::Local;
use takt::forge::{Checks, Forge, MergeMessage, PullRequest, PullRequestState};
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
            .join(format!("local-forge-adv3-{}-{name}", std::process::id()));
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
        git(&self.0, &["clone", "-q", "--no-local", "hub.git", name]);
        self.0.join(name)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn identity() -> CommitIdentity {
    CommitIdentity::new("Test Bot", "bot@example.invalid").expect("an identity")
}

fn local(scratch: &Scratch, tree: &Path, checks: &[&str]) -> Local {
    Local::for_tree(
        tree,
        checks.iter().map(|word| (*word).to_owned()).collect(),
        scratch.0.join("state"),
        identity(),
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

/// The orchestrator merged, by hand, a branch made of the case's head and one fixup commit on
/// it: the base holds the head, as the grandparent of the hand merge's second parent. The next
/// `merge_with` moves no ref and records the request merged by the hand merge
/// (`Local::holder`); `PrMerged` then reads the request again, and every read after that must
/// say merged at the head. It errors instead ("does not hold the head ... as itself or a
/// parent", `Local::merged_head`), so the validator's read after the merge fails, and so does
/// every later run's first read: the case can never record `pr.merged`.
#[test]
fn a_head_the_base_holds_through_a_later_commit_reads_merged_after_the_merge() {
    let scratch = Scratch::new("deep-holder");
    let hub = scratch.hub();
    let a = scratch.clone("a");
    let forge = local(&scratch, &a, &["true"]);
    let artifacts = scratch.0.join("artifacts");
    let (head, number) = delivered(&forge, &a, &artifacts);

    let orchestrator = scratch.clone("orchestrator");
    git(&orchestrator, &["fetch", "-q", "origin", "takt/a"]);
    git(
        &orchestrator,
        &["checkout", "-q", "-b", "fixup", "FETCH_HEAD"],
    );
    fs::write(orchestrator.join("fixup.txt"), "fixup\n").expect("a file");
    git(&orchestrator, &["add", "-A"]);
    git(&orchestrator, &["commit", "-q", "-m", "fixup on a"]);
    git(&orchestrator, &["checkout", "-q", "integration"]);
    git(
        &orchestrator,
        &["merge", "-q", "--no-ff", "--no-edit", "fixup"],
    );
    git(
        &orchestrator,
        &["push", "-q", "origin", "HEAD:refs/heads/integration"],
    );
    let by_hand = git(&hub, &["rev-parse", "refs/heads/integration"]);

    forge.target_tip(&a, "integration").expect("a tip");
    forge
        .merge_with(
            &a,
            &artifacts,
            number,
            &head,
            &MergeMessage {
                title: "merge a".to_owned(),
                body: String::new(),
            },
        )
        .expect("the base holds the head: recorded merged, no ref moved");
    assert_eq!(
        git(&hub, &["rev-parse", "refs/heads/integration"]),
        by_hand,
        "no ref moved"
    );
    let view = forge
        .pull_request(&a, number)
        .expect("the record merge_with just wrote is believed by the next read");
    assert_eq!(
        (view.state, view.head.sha),
        (PullRequestState::Merged, head)
    );
}

/// A tree whose object directory is a link to the hub's: every object the tree's repository
/// writes, packs or prunes (`git gc --prune=now` in the tree deletes the hub's objects its own
/// refs do not reach) is the hub's. `hub_of` reads the object files through the link, finds
/// each with one link, and admits the tree; it must refuse it as it refuses hard links and
/// alternates.
#[test]
fn a_tree_whose_object_directory_is_a_link_into_the_hub_is_refused() {
    let scratch = Scratch::new("objects-link");
    let hub = scratch.hub();
    let a = scratch.clone("a");
    let objects = a.join(".git/objects");
    fs::remove_dir_all(&objects).expect("the tree's own objects removed");
    std::os::unix::fs::symlink(hub.join("objects"), &objects).expect("a link to the hub's");
    git(&a, &["fsck", "--connectivity-only", "--no-progress"]);
    let error = Local::for_tree(
        &a,
        vec!["true".to_owned()],
        scratch.0.join("state"),
        identity(),
    )
    .expect_err("a tree whose objects are the hub's is refused");
    assert!(format!("{error:#}").contains("--no-local"), "{error:#}");
}

/// The run file's `local_checks` changed between two runs on one request (a placeholder `true`
/// replaced by the real check): the pass the first checks recorded for the head is read as the
/// pass of the second, which never runs. The second checks must run (here they write a marker
/// and fail).
#[test]
fn a_pass_under_other_checks_is_not_a_pass_of_these() {
    let scratch = Scratch::new("other-checks");
    let _hub = scratch.hub();
    let a = scratch.clone("a");
    let first = local(&scratch, &a, &["true"]);
    let artifacts = scratch.0.join("artifacts");
    let (_head, number) = delivered(&first, &a, &artifacts);
    assert_eq!(first.checks(&a, number).expect("checks"), Checks::Pass);

    let marker = scratch.0.join("second-checks-ran");
    let script = format!("touch '{}'; exit 1", marker.display());
    let second = local(&scratch, &a, &["sh", "-c", &script]);
    let read = second.checks(&a, number).expect("checks");
    assert!(
        marker.exists(),
        "the second local_checks never ran; the read answered {read:?} from the pass recorded under `true`"
    );
    assert_eq!(read, Checks::Fail);
}
