//! `takt run --case <id>` on a tree whose cases directory is a symbolic link refuses before the
//! first step: exit 1 naming the link, no step taken, the tree on the branch it was on, and
//! nothing written through the link.

use std::collections::BTreeMap;
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

/// A scratch directory under this crate's build directory holding the tree at `tree/`, so the
/// copy made at `../x` stays inside it; removed on drop.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn repository(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("cases-swap-cli-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    let repo = dir.join("tree");
    fs::create_dir_all(&repo).expect("a directory");
    fs::write(repo.join("README.md"), "a story\n").expect("a file");
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "start"]);
    Scratch(dir)
}

/// `takt run --repo <repo>` with `args`, one step at most; its exit code, standard output and
/// standard error.
fn takt_run(repo: &Path, args: &[&str]) -> (i32, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args(["run", "--repo"])
        .arg(repo)
        .args(args)
        .args([
            "--harness",
            "none",
            "--forge",
            "none",
            "--iterations",
            "1",
            "--test-cmd",
            "true",
        ])
        .env_remove("B10X_GATES_POLICY")
        .output()
        .expect("takt runs");
    (
        output.status.code().expect("an exit code"),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// Every file under `dir` with its bytes, by path relative to it; a link is not followed.
fn files(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, into: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in fs::read_dir(dir).expect("a readable directory") {
            let path = entry.expect("an entry").path();
            let meta = fs::symlink_metadata(&path).expect("metadata");
            let relative = path
                .strip_prefix(root)
                .expect("under the root")
                .to_path_buf();
            if meta.is_dir() {
                walk(root, &path, into);
            } else if meta.is_file() {
                into.insert(relative, fs::read(&path).expect("a file"));
            }
        }
    }
    let mut into = BTreeMap::new();
    walk(dir, dir, &mut into);
    into
}

/// Acceptance of the story, its second half: a case opened by one run, its cases directory then
/// moved beside the tree and a link to it put in its place, is refused by `takt run --case`
/// before the first step, naming the link.
#[test]
fn run_with_case_refuses_a_cases_directory_that_is_a_link_before_the_first_step() {
    let scratch = repository("resume");
    let repo = scratch.0.join("tree");
    let (code, out, err) = takt_run(&repo, &["--intent", "probe", "--case-id", "linked"]);
    assert!(
        repo.join(".takt/cases/linked/case.yaml").is_file(),
        "the case opens ({code})\n{out}\n{err}"
    );
    git(&repo, &["checkout", "-q", "main"]);
    let status = Command::new("sh")
        .args(["-c", "mv .takt/cases ../x && ln -s ../../x .takt/cases"])
        .current_dir(&repo)
        .status()
        .expect("sh runs");
    assert!(status.success());
    let copy = scratch.0.join("x");
    let before = files(&copy);
    let (code, out, err) = takt_run(&repo, &["--case", "linked"]);
    assert_eq!(code, 1, "{out}\n{err}");
    assert!(
        err.contains(".takt/cases") && err.contains("a link"),
        "the refusal names the link: {err}"
    );
    assert!(!out.contains("step 1"), "a step was taken: {out}");
    assert_eq!(git(&repo, &["rev-parse", "--abbrev-ref", "HEAD"]), "main");
    assert_eq!(
        files(&copy),
        before,
        "something was written through the link"
    );
}
