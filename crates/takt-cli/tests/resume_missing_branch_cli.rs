//! `takt run --case <id>` on a case whose branch `takt/<id>` is gone exits 1 with takt's own
//! message (story:resume-names-a-missing-branch): it names the missing branch, the last head
//! the case recorded and the next step, the `git branch` line that recreates the branch at
//! that head or giving the case up; no git pathspec text reaches the user, and nothing in the
//! case directory or among the refs changes.

#[path = "../../takt/tests/support/own_home.rs"]
mod own_home;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

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
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// A scratch repository under this crate's build directory, removed on drop.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// A committed repository on `main` with a second branch `trial/0` at the same commit.
fn repository(name: &str) -> Scratch {
    let repo = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "resume-missing-branch-{}-{name}",
        std::process::id()
    ));
    if repo.exists() {
        fs::remove_dir_all(&repo).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(&repo).expect("a directory");
    fs::write(repo.join("README.md"), "a story\n").expect("a file");
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["config", "user.name", "t"]);
    git(&repo, &["config", "user.email", "t@localhost"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "start"]);
    git(&repo, &["branch", "trial/0"]);
    Scratch(repo)
}

/// `takt run --repo <repo>` with `case` (`--case-id`/`--intent` or `--case`) and the flags of
/// the finding's predicate.
fn takt(repo: &Path, case: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_takt"))
        .args(["run", "--repo"])
        .arg(repo)
        .args(case)
        .args([
            "--harness",
            "none",
            "--forge",
            "none",
            "--iterations",
            "3",
            "--test-cmd",
            "true",
        ])
        .output()
        .expect("takt runs")
}

/// Every file under `dir`, by its path relative to `dir`, with its bytes.
fn files(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, into: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in fs::read_dir(dir).expect("a readable directory") {
            let path = entry.expect("an entry").path();
            if path.is_dir() {
                walk(root, &path, into);
            } else {
                let relative = path
                    .strip_prefix(root)
                    .expect("under the root")
                    .to_path_buf();
                into.insert(relative, fs::read(&path).expect("a readable file"));
            }
        }
    }
    let mut all = BTreeMap::new();
    walk(dir, dir, &mut all);
    all
}

/// Every ref of `repo` with the object it names, and where `HEAD` points.
fn refs(repo: &Path) -> (String, String) {
    (
        git(repo, &["for-each-ref", "--format=%(refname) %(objectname)"]),
        git(repo, &["symbolic-ref", "-q", "HEAD"]),
    )
}

#[test]
fn resume_of_a_case_whose_branch_is_gone_names_it_and_how_to_recover_with_nothing_written() {
    let scratch = repository("gone");
    let repo = &scratch.0;
    let opened = takt(repo, &["--intent", "a", "--case-id", "a"]);
    let case = repo.join(".takt/cases/a");
    assert!(
        case.join("case.yaml").is_file(),
        "the case opens: {}",
        String::from_utf8_lossy(&opened.stderr)
    );
    let head = git(repo, &["rev-parse", "refs/heads/takt/a"]);
    git(repo, &["checkout", "-q", "trial/0"]);
    git(repo, &["branch", "-q", "-D", "takt/a"]);
    let files_before = files(&case);
    let refs_before = refs(repo);

    let resumed = takt(repo, &["--case", "a"]);
    let err = String::from_utf8_lossy(&resumed.stderr);

    assert_eq!(resumed.status.code(), Some(1), "{err}");
    assert!(
        err.contains("takt/a") && err.contains("does not exist"),
        "the missing branch is not named: {err}"
    );
    assert!(
        err.contains(&head),
        "the last head the case recorded, {head}, is not named: {err}"
    );
    assert!(
        err.contains(&format!("git branch takt/a {head}")),
        "no line recreates the branch at that head: {err}"
    );
    assert!(
        err.contains(".takt/cases/a"),
        "giving the case up is not named: {err}"
    );
    assert!(
        !err.contains("pathspec"),
        "git's refusal reaches the user: {err}"
    );
    assert_eq!(
        files(&case),
        files_before,
        "the case directory changed: {err}"
    );
    assert_eq!(refs(repo), refs_before, "the refs changed: {err}");
}
