//! Adversary pass 2: `takt run --case <id>` joins the id to the cases directory and reads the
//! case's `protocol.yaml` there before `check_id` sees the id, though `check_id`'s doc says
//! every id `takt run --case` gives passes it before it is joined to a path.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

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

/// A scratch directory under this crate's build directory, removed on drop.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn run_with_case_reads_a_protocol_outside_the_cases_directory_before_check_id() {
    let scratch = Scratch(
        Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("case-id-cli-adv2-{}", std::process::id())),
    );
    if scratch.0.exists() {
        fs::remove_dir_all(&scratch.0).expect("a stale scratch tree is removed");
    }
    let repo = scratch.0.join("repo");
    fs::create_dir_all(&repo).expect("a directory");
    fs::write(repo.join("README.md"), "a story\n").expect("a file");
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "start"]);
    // A directory outside the repository that looks like a case: a `case.yaml` and a
    // `protocol.yaml` holding something no protocol is.
    let outside = scratch.0.join("outside");
    fs::create_dir_all(&outside).expect("a directory");
    fs::write(outside.join("case.yaml"), "format: canon-case/1\n").expect("a file");
    fs::write(
        outside.join("protocol.yaml"),
        "secret: outside the cases directory\n",
    )
    .expect("a file");
    let id = outside.to_str().expect("utf-8").to_owned();
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args(["run", "--repo"])
        .arg(&repo)
        .args([
            "--case",
            &id,
            "--harness",
            "none",
            "--forge",
            "none",
            "--iterations",
            "1",
            "--test-cmd",
            "true",
        ])
        .output()
        .expect("takt runs");
    let err = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{err}");
    assert!(
        err.contains(&format!("the case id `{id}` is refused")),
        "not check_id's refusal, the protocol outside was read first: {err}"
    );
}
