//! Adversary cases for wave 54, library level.
//!
//! - story:a-git-read-error-leaves-no-process: a bounded git read leaves no process of this
//!   test binary behind, on the over-limit path and on a git that exits non-zero.
//! - story:test-cmd-checked-before-open: `TestsRun::check_program` admits what the test step
//!   can start and refuses what it cannot, the step being `TestsRun::observe` under `none`.
//!
//! Every case here takes one lock: the process-wide checks (the children of this process, the
//! `PATH` of this process) must not see another case's work.

#[path = "support/own_home.rs"]
mod own_home;

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, MutexGuard, PoisonError};

use takt::validator::{TestsRun, Validator as _};

static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

fn one_at_a_time() -> MutexGuard<'static, ()> {
    ONE_AT_A_TIME.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A scratch directory under this crate's build directory, removed on drop.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn scratch(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("w54-adversary-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("a scratch directory");
    Scratch(dir)
}

fn git(tree: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(args)
        .current_dir(tree)
        .output()
        .expect("git runs");
    assert!(output.status.success(), "git {args:?}");
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// A repository at `outer/repo` holding `files` (path, bytes, mode) in one commit; its head.
fn repository(outer: &Path, files: &[(&str, &[u8], u32)]) -> (PathBuf, String) {
    let repo = outer.join("repo");
    fs::create_dir_all(&repo).expect("a directory");
    for (path, bytes, mode) in files {
        let at = repo.join(path);
        fs::create_dir_all(at.parent().expect("a parent")).expect("a directory");
        fs::write(&at, bytes).expect("a file");
        fs::set_permissions(&at, fs::Permissions::from_mode(*mode)).expect("a mode");
    }
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "--allow-empty", "-m", "start"]);
    let head = git(&repo, &["rev-parse", "HEAD"]);
    (repo, head)
}

/// The process ids that are children of this process, zombies included, read from every
/// thread's `children` list.
fn children() -> Vec<String> {
    let mut found = Vec::new();
    for task in fs::read_dir("/proc/self/task")
        .expect("the tasks")
        .flatten()
    {
        if let Ok(text) = fs::read_to_string(task.path().join("children")) {
            found.extend(text.split_whitespace().map(str::to_owned));
        }
    }
    found.sort();
    found
}

/// story:a-git-read-error-leaves-no-process: `run_bounded` past its limit kills git and waits
/// for it; no process of this binary is left, not even an unwaited one.
#[test]
fn w54_a_run_bounded_past_its_limit_leaves_no_git_process() {
    let _one = one_at_a_time();
    let outer = scratch("bounded-over");
    let big: Vec<u8> = (0..(4u32 << 20)).map(|i| b'a' + (i % 26) as u8).collect();
    let (repo, _) = repository(&outer.0, &[("big.txt", &big, 0o644)]);
    assert_eq!(children(), Vec::<String>::new(), "no child before the call");
    let read = takt::git::run_bounded(&repo, &["log", "--patch"], 64).expect("git runs");
    assert!(read.is_none(), "past 64 bytes the read is none");
    assert_eq!(
        children(),
        Vec::<String>::new(),
        "story:a-git-read-error-leaves-no-process: a git left behind past the limit"
    );
}

/// story:a-git-read-error-leaves-no-process: a git that exits non-zero is an error naming what
/// git said, and leaves no process.
#[test]
fn w54_a_run_bounded_on_a_failing_git_names_it_and_leaves_no_process() {
    let _one = one_at_a_time();
    let outer = scratch("bounded-fail");
    let (repo, _) = repository(&outer.0, &[("a.txt", b"a\n", 0o644)]);
    let error = takt::git::run_bounded(&repo, &["log", "no-such-revision-w54"], 1 << 20)
        .expect_err("an unknown revision is an error");
    let shown = format!("{error:#}");
    assert!(
        shown.contains("git log no-such-revision-w54") && shown.contains("no-such-revision-w54"),
        "{shown}"
    );
    assert_eq!(
        children(),
        Vec::<String>::new(),
        "story:a-git-read-error-leaves-no-process: a failing git left behind"
    );
}

/// Whether the test step, `TestsRun::observe` under `none`, ran `command` in `repo` at `head`
/// and it passed.
fn step_passes(command: &str, repo: &Path, head: &str) -> (bool, String) {
    match TestsRun::new(command)
        .expect("a command")
        .observe(repo, head, 1)
    {
        Ok(observation) => (
            observation.summary.contains("exited 0: pass"),
            observation.summary,
        ),
        Err(error) => (false, format!("{error:#}")),
    }
}

/// story:test-cmd-checked-before-open: the check admits exactly what the step starts. With an
/// empty entry on takt's `PATH` (the current directory to `execvp`, and the step starts the
/// program in the tree), a bare name at the root of the tree is started by the step; the check
/// must not refuse it.
#[test]
fn w54_b_check_program_agrees_with_the_step_on_an_empty_path_entry() {
    let _one = one_at_a_time();
    let outer = scratch("empty-path-entry");
    let (repo, head) = repository(
        &outer.0,
        &[("w54-run-tests", b"#!/bin/sh\nexit 0\n", 0o755)],
    );
    let before = std::env::var("PATH").expect("a PATH");
    // SAFETY: every case of this binary holds `one_at_a_time`, so no other thread of it reads
    // the environment while it changes; the PATH is put back before the lock is let go.
    unsafe { std::env::set_var("PATH", format!("{before}:")) };
    let (ran, said) = step_passes("w54-run-tests", &repo, &head);
    let checked = TestsRun::new("w54-run-tests")
        .expect("a command")
        .check_program(&repo);
    // SAFETY: as above.
    unsafe { std::env::set_var("PATH", &before) };
    assert_eq!(
        checked.is_ok(),
        ran,
        "story:test-cmd-checked-before-open: the step {} the program (summary: {said}); the check {}",
        if ran { "starts" } else { "does not start" },
        match &checked {
            Ok(()) => "admits it".to_owned(),
            Err(error) => format!("refuses it: {error:#}"),
        }
    );
}

/// story:test-cmd-checked-before-open: a file whose only execute bit is another's (mode 0o001,
/// owned by this user) cannot be started by its owner; the check must refuse it as the step
/// would fail to start it.
#[test]
fn w54_b_check_program_agrees_with_the_step_on_an_execute_bit_for_others_only() {
    let _one = one_at_a_time();
    let outer = scratch("others-only");
    let (repo, head) = repository(&outer.0, &[("check.sh", b"#!/bin/sh\nexit 0\n", 0o755)]);
    fs::set_permissions(repo.join("check.sh"), fs::Permissions::from_mode(0o701)).expect("a mode");
    let (ran, said) = step_passes("./check.sh", &repo, &head);
    let checked = TestsRun::new("./check.sh")
        .expect("a command")
        .check_program(&repo);
    fs::set_permissions(repo.join("check.sh"), fs::Permissions::from_mode(0o001)).expect("a mode");
    let (ran_none, said_none) = step_passes("./check.sh", &repo, &head);
    let checked_none = TestsRun::new("./check.sh")
        .expect("a command")
        .check_program(&repo);
    fs::set_permissions(repo.join("check.sh"), fs::Permissions::from_mode(0o755)).expect("a mode");
    assert_eq!(
        checked.is_ok(),
        ran,
        "mode 0o701: the step {said}; the check {checked:?}"
    );
    assert_eq!(
        checked_none.is_ok(),
        ran_none,
        "story:test-cmd-checked-before-open: mode 0o001 (execute for others only): the step {}: {said_none}; the check {}",
        if ran_none {
            "starts it"
        } else {
            "cannot start it"
        },
        match &checked_none {
            Ok(()) => "admits it".to_owned(),
            Err(error) => format!("refuses it: {error:#}"),
        }
    );
}

/// story:test-cmd-checked-before-open: a dangling link, a directory and a link out of the tree
/// to an executable, each named by path: the check agrees with the step on each.
#[test]
fn w54_b_check_program_agrees_with_the_step_on_links_and_directories() {
    let _one = one_at_a_time();
    let outer = scratch("links");
    let outside = outer.0.join("outside.sh");
    fs::write(&outside, b"#!/bin/sh\nexit 0\n").expect("a file");
    fs::set_permissions(&outside, fs::Permissions::from_mode(0o755)).expect("a mode");
    let (repo, _) = repository(&outer.0, &[("keep.txt", b"k\n", 0o644)]);
    std::os::unix::fs::symlink(outer.0.join("no-such-target.sh"), repo.join("dangling.sh"))
        .expect("a dangling link");
    std::os::unix::fs::symlink(&outside, repo.join("out.sh")).expect("a link out");
    fs::create_dir_all(repo.join("dir.sh")).expect("a directory");
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "links"]);
    let head = git(&repo, &["rev-parse", "HEAD"]);
    let mut disagree = Vec::new();
    for program in ["./dangling.sh", "./dir.sh", "./out.sh"] {
        let (ran, said) = step_passes(program, &repo, &head);
        let checked = TestsRun::new(program)
            .expect("a command")
            .check_program(&repo);
        if checked.is_ok() != ran {
            disagree.push(format!("{program}: step {ran} ({said}); check {checked:?}"));
        }
    }
    assert!(
        disagree.is_empty(),
        "story:test-cmd-checked-before-open: {disagree:#?}"
    );
}
