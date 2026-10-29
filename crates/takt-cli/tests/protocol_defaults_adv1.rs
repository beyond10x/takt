//! Adversary pass 1 on the `--protocol` refusal for a resumed case: the acceptance says a
//! `--protocol` other than the case's "exits non-zero naming both protocols and touching
//! nothing". The refusal runs in `main.rs` after `Run::resume`, and `CaseDir::resume` checks the
//! tree out onto the case's branch and rewrites the case's implementation revision before it.

#[path = "../../takt/tests/support/own_home.rs"]
mod own_home;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../takt/tests/fixtures");

const GATES: &str = "format: engineering-gates/1\nlanguage: canon-expr/1\ngates:\n  - id: manifest\n    \
                     text: The repository has its manifest.\n    assertions:\n      \
                     - 'fs.file.exists(\"Cargo.toml\")'\n";

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

struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("a directory");
    for entry in fs::read_dir(from).expect("a readable fixture") {
        let entry = entry.expect("an entry");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("a file type").is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).expect("a copied file");
        }
    }
}

fn repository(fixture: Option<&str>, name: &str) -> (Scratch, PathBuf) {
    let outer = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "protocol-defaults-adv1-{}-{name}",
        std::process::id()
    ));
    if outer.exists() {
        fs::remove_dir_all(&outer).expect("a stale scratch tree is removed");
    }
    let repo = outer.join("repo");
    match fixture {
        Some(fixture) => copy_dir(&Path::new(FIXTURES).join(fixture), &repo),
        None => fs::create_dir_all(&repo).expect("a directory"),
    }
    fs::write(repo.join("Cargo.toml"), "[package]\nname = \"scratch\"\n").expect("a file");
    fs::create_dir_all(repo.join(".engineering")).expect("a directory");
    fs::write(repo.join(".engineering/gates.yaml"), GATES).expect("a file");
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "start"]);
    (Scratch(outer), repo)
}

fn takt(args: &[&str]) -> (i32, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args(args)
        .env_remove("B10X_GATES_POLICY")
        .output()
        .expect("takt runs");
    (
        output.status.code().expect("an exit code"),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// Every file under `dir` but `.git`, and the tree's HEAD commit, branch and status.
fn everything(dir: &Path) -> (BTreeMap<PathBuf, Vec<u8>>, String, String, String) {
    fn walk(root: &Path, dir: &Path, files: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in fs::read_dir(dir).expect("a readable directory") {
            let path = entry.expect("an entry").path();
            if path.file_name().is_some_and(|name| name == ".git") {
                continue;
            }
            if path.is_dir() {
                walk(root, &path, files);
            } else {
                let relative = path.strip_prefix(root).expect("under the root").to_owned();
                files.insert(relative, fs::read(&path).expect("a readable file"));
            }
        }
    }
    let mut files = BTreeMap::new();
    walk(dir, dir, &mut files);
    (
        files,
        git(dir, &["rev-parse", "HEAD"]),
        git(dir, &["symbolic-ref", "-q", "HEAD"]),
        git(dir, &["status", "--porcelain", "--untracked-files=all"]),
    )
}

fn no_steps(repo: &str) -> Vec<&str> {
    vec![
        "run",
        "--repo",
        repo,
        "--harness",
        "none",
        "--test-cmd",
        "true",
        "--forge",
        "none",
        "--iterations",
        "0",
    ]
}

/// `--case` with another `--protocol`, the tree on the base branch since the case's last run
/// (the operator went back to `main`): the refusal must leave the tree on `main`.
#[test]
fn adv_a_refused_protocol_for_a_case_leaves_the_tree_on_the_branch_it_was_on() {
    let (_outer, repo) = repository(None, "off-branch");
    let path = repo.to_str().expect("utf-8");
    let mut args = no_steps(path);
    args.extend(["--intent", "a change", "--case-id", "change"]);
    let (code, out, err) = takt(&args);
    assert_eq!(code, 3, "stdout {out} stderr {err}");
    assert_eq!(
        git(&repo, &["symbolic-ref", "-q", "HEAD"]),
        "refs/heads/takt/change"
    );
    git(&repo, &["checkout", "-q", "main"]);
    let before = everything(&repo);
    let mut args = no_steps(path);
    args.extend(["--case", "change", "--protocol", "story.delivery/1"]);
    let (code, out, err) = takt(&args);
    assert_eq!(code, 1, "refused: stdout {out} stderr {err}");
    assert!(err.contains("nothing was changed"), "{err}");
    let after = everything(&repo);
    assert_eq!(
        (&after.1, &after.2, &after.3),
        (&before.1, &before.2, &before.3),
        "the refusal says nothing was changed, and the tree's HEAD/branch/status moved"
    );
    assert_eq!(after.0, before.0, "the refusal changed a file");
}

/// The trial-run-1 path: a store story whose case runs on `software.change/1`, the tree back on
/// `main`, and a bare run with `--protocol story.delivery/1` to put it right. Refused, and the
/// tree must stay on `main`.
#[test]
fn adv_a_refused_protocol_for_the_store_resumed_case_leaves_the_tree_on_its_branch() {
    let (_outer, repo) = repository(Some("one-ready"), "store-off-branch");
    let path = repo.to_str().expect("utf-8");
    let mut args = no_steps(path);
    args.extend(["--protocol", "software.change/1"]);
    let (code, out, err) = takt(&args);
    assert_eq!(code, 3, "stdout {out} stderr {err}");
    assert!(out.contains("frontier: took story:ready-one"), "{out}");
    git(&repo, &["checkout", "-q", "main"]);
    let before = everything(&repo);
    let mut args = no_steps(path);
    args.extend(["--protocol", "story.delivery/1"]);
    let (code, out, err) = takt(&args);
    assert_eq!(code, 1, "refused: stdout {out} stderr {err}");
    assert!(err.contains("nothing was changed"), "{err}");
    let after = everything(&repo);
    assert_eq!(
        (&after.1, &after.2, &after.3),
        (&before.1, &before.2, &before.3),
        "the refusal says nothing was changed, and the tree's HEAD/branch/status moved"
    );
    assert_eq!(after.0, before.0, "the refusal changed a file");
}

/// A commit on the case's branch between two runs, then `--case` with another `--protocol`:
/// the refusal must not rewrite the case's files.
#[test]
fn adv_a_refused_protocol_for_a_case_does_not_rewrite_its_revision() {
    let (_outer, repo) = repository(None, "moved-head");
    let path = repo.to_str().expect("utf-8");
    let mut args = no_steps(path);
    args.extend(["--intent", "a change", "--case-id", "change"]);
    let (code, out, err) = takt(&args);
    assert_eq!(code, 3, "stdout {out} stderr {err}");
    fs::write(repo.join("feature.txt"), "a feature\n").expect("a file");
    git(&repo, &["add", "feature.txt"]);
    git(&repo, &["commit", "-q", "-m", "the feature"]);
    let before = everything(&repo);
    let mut args = no_steps(path);
    args.extend(["--case", "change", "--protocol", "story.delivery/1"]);
    let (code, out, err) = takt(&args);
    assert_eq!(code, 1, "refused: stdout {out} stderr {err}");
    assert!(err.contains("nothing was changed"), "{err}");
    let after = everything(&repo);
    let changed: Vec<&PathBuf> = after
        .0
        .iter()
        .filter(|(path, bytes)| before.0.get(*path) != Some(*bytes))
        .map(|(path, _)| path)
        .collect();
    assert!(
        changed.is_empty(),
        "the refusal says nothing was changed, and it rewrote {changed:?}"
    );
    assert_eq!(
        (&after.1, &after.2, &after.3),
        (&before.1, &before.2, &before.3)
    );
}
