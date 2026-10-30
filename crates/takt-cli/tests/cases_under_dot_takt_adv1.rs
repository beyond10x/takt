//! Adversarial cases for story:cases-live-under-dot-takt, through the built binary: what a test
//! command does to `.takt/.gitignore`, and a legacy case read in a tree whose `.takt` is not a
//! directory. Nothing reaches a model or a network.

#[path = "../../takt/tests/support/own_home.rs"]
mod own_home;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// A recorded `loop.improvement/1` turn, as the run record copied it.
const TURN: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../docs/runs/2026-10-18-improvement-turn-1/case"
);

/// A scratch directory under this crate's build directory, removed on drop.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn git(tree: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(args)
        .current_dir(tree)
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?}");
}

/// `git <args>` in `tree`; its standard output, trimmed.
fn git_out(tree: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(tree)
        .output()
        .expect("git runs");
    assert!(output.status.success(), "git {args:?}");
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// A committed repository with a README and `files`, at `<scratch>/tree`.
fn repository(name: &str, files: &[(&str, &str)]) -> (Scratch, PathBuf) {
    let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("dot-takt-cli-adv1-{}-{name}", std::process::id()));
    if outer.exists() {
        fs::remove_dir_all(&outer).expect("a stale scratch tree is removed");
    }
    let repo = outer.join("tree");
    fs::create_dir_all(&repo).expect("a repository");
    fs::write(repo.join("README.md"), "a story\n").expect("a file");
    for (path, text) in files {
        let path = repo.join(path);
        fs::create_dir_all(path.parent().expect("a parent")).expect("a parent directory");
        fs::write(&path, text).expect("a file");
    }
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "start"]);
    (Scratch(outer), repo)
}

/// Runs the built `takt` binary with `args`: the exit code, stdout and stderr.
fn takt(args: &[&str]) -> (i32, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args(args)
        .output()
        .expect("takt runs");
    (
        output.status.code().expect("an exit code"),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("a directory");
    for entry in fs::read_dir(from).expect("a readable directory") {
        let entry = entry.expect("an entry");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("a file type").is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).expect("a copied file");
        }
    }
}

/// Acceptance 1 under a test command that removes `.takt/.gitignore`, as a test that cleans
/// ignored files would: the test command runs code a model may have written, and what it leaves
/// is checked against the tree before it. The ignore file is what keeps every case out of
/// `git status`; removing it is admitted as an ignored file the observation removed, the run
/// goes on, and the case shows as untracked work, the very state the story removes.
#[test]
fn a_test_command_that_removes_the_takt_gitignore_leaves_git_status_clean() {
    let (_scratch, repo) = repository(
        "rm-ignore",
        &[("tests/run.sh", "#!/bin/sh\nrm -f .takt/.gitignore\n")],
    );
    let path = repo.to_str().expect("utf-8");
    let (code, out, err) = takt(&[
        "run",
        "--repo",
        path,
        "--intent",
        "probe",
        "--case-id",
        "rm-ignore",
        "--harness",
        "none",
        "--forge",
        "none",
        "--iterations",
        "1",
        "--test-cmd",
        "sh tests/run.sh",
    ]);
    assert!(
        repo.join(".takt/cases/rm-ignore/case.yaml").is_file(),
        "the case opens under .takt/cases ({code})\nstdout {out}\nstderr {err}"
    );
    assert_eq!(
        git_out(&repo, &["status", "--porcelain"]),
        "",
        "git status shows none of the run's records ({code})\nstdout {out}\nstderr {err}"
    );
    assert_eq!(
        fs::read_to_string(repo.join(".takt/.gitignore")).unwrap_or_default(),
        "*\n",
        "takt's .takt/.gitignore is there after the run ({code})\nstdout {out}\nstderr {err}"
    );
}

/// A case under the legacy root is still scored where it is (acceptance 6, the story's outcome)
/// in a repository that tracks a file named `.takt`: the new root cannot be there, so it holds
/// no case, and the readers list the legacy one.
#[test]
fn readers_find_a_legacy_case_when_dot_takt_is_a_file() {
    let (_scratch, repo) = repository("takt-file", &[(".takt", "a file of another tool\n")]);
    copy_dir(Path::new(TURN), &repo.join(".engineering/cases/turn-old"));
    let path = repo.to_str().expect("utf-8");
    let (code, out, err) = takt(&["scorecard", "--repo", path]);
    assert_eq!(code, 0, "stdout {out} stderr {err}");
    assert!(
        out.contains("turn-old"),
        "the scorecard lists the legacy case: stdout {out} stderr {err}"
    );
}
