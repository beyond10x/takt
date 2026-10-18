//! `takt run --case-id <id>` with an id whose branch `takt/<id>` git would not accept exits 1
//! with takt's own message naming the id and the rule, before anything is written: no case
//! directory, no branch, the tree on the branch it was on.

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

/// A scratch repository under this crate's build directory, removed on drop.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn repository(name: &str) -> Scratch {
    let repo = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("case-id-cli-{}-{name}", std::process::id()));
    if repo.exists() {
        fs::remove_dir_all(&repo).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(&repo).expect("a directory");
    fs::write(repo.join("README.md"), "a story\n").expect("a file");
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "start"]);
    Scratch(repo)
}

#[test]
fn run_with_a_case_id_git_refuses_as_a_branch_exits_1_naming_it_with_nothing_written() {
    let scratch = repository("run");
    let repo = &scratch.0;
    for (id, rule) in [
        ("a~b", "`~`"),
        ("foo.", "ends with `.`"),
        ("a..b", "`..`"),
        ("a@{b", "`@{`"),
        ("x.lock", "ends with `.lock`"),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_takt"))
            .args(["run", "--repo"])
            .arg(repo)
            .args([
                "--case-id",
                id,
                "--intent",
                "probe",
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
        assert_eq!(output.status.code(), Some(1), "{id}: {err}");
        assert!(err.contains(&format!("the case id `{id}`")), "{id}: {err}");
        assert!(err.contains(rule), "{id}: {rule} not named: {err}");
        assert!(!err.contains("git checkout"), "{id}: git's refusal: {err}");
        assert!(
            !repo.join(".engineering/cases").exists(),
            "{id}: a case dir"
        );
        assert!(
            git(repo, &["branch", "--list", "takt/*"]).is_empty(),
            "{id}: a branch was made"
        );
        assert_eq!(git(repo, &["rev-parse", "--abbrev-ref", "HEAD"]), "main");
    }
}
