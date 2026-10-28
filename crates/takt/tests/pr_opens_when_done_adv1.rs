//! story:wave11-review-follow-ups, adversary pass 1 on `change.read` (item 7): a revision whose
//! merge into the base branch conflicts is `ahead`, since there is a change to publish, and the
//! summary says the merge conflicts. These are the conflicts git resolves by leaving the base's
//! side in the tree it writes: a file the case deleted that the base modified, and a binary
//! file both changed. Each case is named for what it asserts.

#[path = "support/own_home.rs"]
mod own_home;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

use takt::forge::{Forge, Memory};
use takt::harness::{Harness, StepContext, StepReport};
use takt::validator::{ChangeRead, PrOpen, Validator};
use takt::{Run, RunOptions};

const PROTOCOL: &str = "story.delivery/1";

fn git(tree: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(args)
        .current_dir(tree)
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("utf-8")
        .trim()
        .to_owned()
}

struct Scratch(PathBuf);

impl std::ops::Deref for Scratch {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
        let _ = fs::remove_dir_all(self.0.with_extension("other"));
    }
}

/// A repository on `main` whose one commit holds `README.md`, `notes.txt` and the binary
/// `blob.bin`.
fn scratch(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "pr-opens-when-done-adv1-{}-{name}",
        std::process::id()
    ));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(&dir).expect("scratch dir");
    fs::write(dir.join("README.md"), "a story\n").expect("a file");
    fs::write(dir.join("notes.txt"), "notes\n").expect("a file");
    fs::write(dir.join("blob.bin"), b"bin\0one\n").expect("a file");
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "start"]);
    Scratch(dir)
}

/// Never asked for a step.
struct NoHarness;

impl Harness for NoHarness {
    fn name(&self) -> &str {
        "none"
    }

    fn step(&self, _: &StepContext<'_>) -> anyhow::Result<StepReport> {
        panic!("no step is asked for")
    }
}

/// A case `name` opened on `main`, `mine` applied to the tree and committed on its branch as
/// its implementation revision, and `theirs` applied and committed on `main` after; the tree
/// and the revision.
fn conflicting(name: &str, mine: impl Fn(&Path), theirs: impl Fn(&Path)) -> (Scratch, String) {
    let tree = scratch(name);
    let memory = Rc::new(Memory::default());
    let forge: Rc<dyn Forge> = memory;
    let validators: Vec<Box<dyn Validator>> = vec![Box::new(PrOpen::new(forge))];
    let options = RunOptions {
        protocol: PROTOCOL.to_owned(),
        intent: "deliver".to_owned(),
        inputs: Vec::new(),
        iterations: Some(1),
        hours: None,
        tokens: None,
        case_id: Some(name.to_owned()),
        gate: None,
    };
    let mut run = Run::open(&tree, options, &NoHarness, &validators).expect("the case opens");
    mine(&tree);
    git(
        &tree,
        &["add", "-A", "--", "README.md", "notes.txt", "blob.bin"],
    );
    git(&tree, &["commit", "-q", "-m", "the fix"]);
    let revision = git(&tree, &["rev-parse", "HEAD"]);
    run.case
        .set_revision("implementation", &revision)
        .expect("the revision moves");
    drop(run);
    let other = tree.with_extension("other");
    let _ = fs::remove_dir_all(&other);
    git(
        &tree,
        &[
            "worktree",
            "add",
            "-q",
            other.to_str().expect("utf-8"),
            "main",
        ],
    );
    theirs(&other);
    git(&other, &["add", "-A"]);
    git(&other, &["commit", "-q", "-m", "a conflicting change"]);
    git(
        &tree,
        &[
            "worktree",
            "remove",
            "--force",
            other.to_str().expect("utf-8"),
        ],
    );
    (tree, revision)
}

/// The case deletes `notes.txt`; the base modifies it meanwhile. The merge conflicts
/// (modify/delete) and git leaves the base's version in the tree it writes, so that tree is the
/// base's tip outside `.engineering/`. The deletion is still a change to publish: `ahead`.
#[test]
fn change_read_reads_ahead_when_the_case_deletes_a_file_the_base_modified() {
    let (tree, revision) = conflicting(
        "modify-delete",
        |tree| fs::remove_file(tree.join("notes.txt")).expect("deleted"),
        |other| fs::write(other.join("notes.txt"), "notes changed\n").expect("modified"),
    );
    let read = ChangeRead.observe(&tree, &revision, 1).expect("observes");
    assert!(read.summary.contains("conflicts"), "{}", read.summary);
    assert_eq!(
        read.records[0].result.as_deref(),
        Some("ahead"),
        "a conflicting change read as nothing to publish: {}",
        read.summary
    );
}

/// The case and the base both change the binary `blob.bin`. The merge conflicts and git leaves
/// the base's side in the tree it writes. The case's change is still one to publish: `ahead`.
#[test]
fn change_read_reads_ahead_when_both_sides_change_a_binary_file() {
    let (tree, revision) = conflicting(
        "binary",
        |tree| fs::write(tree.join("blob.bin"), b"bin\0mine\n").expect("changed"),
        |other| fs::write(other.join("blob.bin"), b"bin\0theirs\n").expect("changed"),
    );
    let read = ChangeRead.observe(&tree, &revision, 1).expect("observes");
    assert!(read.summary.contains("conflicts"), "{}", read.summary);
    assert_eq!(
        read.records[0].result.as_deref(),
        Some("ahead"),
        "a conflicting change read as nothing to publish: {}",
        read.summary
    );
}
