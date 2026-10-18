//! The `takt` binary over a copy of a fixture planning store, asked for a unit with no
//! `--intent`: what the store says about the story once the run has ended, whichever way it
//! ended, and the exit codes the binary maps a frontier answer to.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The fixture stores of the library crate, one directory each.
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../takt/tests/fixtures");

fn git(tree: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(args)
        .current_dir(tree)
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?}");
}

/// A scratch git repository under this crate's build directory, removed on drop.
struct Scratch(PathBuf);

impl std::ops::Deref for Scratch {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            let message = format!("remove the scratch tree {}: {error}", self.0.display());
            if std::thread::panicking() {
                eprintln!("{message}");
            } else {
                panic!("{message}");
            }
        }
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

fn scratch(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("frontier-cli-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(&dir).expect("scratch dir");
    Scratch(dir)
}

/// A committed copy of the fixture store `fixture` in a scratch tree of its own.
fn repository(fixture: &str, name: &str) -> Scratch {
    let dir = scratch(name);
    copy_dir(&Path::new(FIXTURES).join(fixture), &dir);
    fs::write(dir.join("check.txt"), "broken\n").expect("a file");
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "start"]);
    dir
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

/// The status the store lists `id` at.
fn status(repo: &Path, id: &str) -> String {
    takt::frontier::Aep::default()
        .list(repo)
        .expect("the store lists")
        .into_iter()
        .find(|a| a.id == id)
        .map(|a| a.status)
        .unwrap_or_else(|| panic!("{id} is listed"))
}

/// The story is taken (`proposed` → `active`) before the protocol is loaded and the case is
/// opened. When that fails — here a protocol id nothing provides, the same as a relative
/// `--protocol` path given from the wrong directory (README.md:24-25) or a protocol whose
/// declared inputs were not given — the run exits 1 with no case, and the story is left
/// `active`, from where the store's story lifecycle offers only `implemented` and `archived`:
/// no later run can take it. The store is only told once the run has a case to work.
#[test]
fn a_run_that_fails_after_the_take_leaves_the_story_proposed_and_opens_no_case() {
    let repo = repository("one-ready", "orphan");
    let (code, out, err) = takt(&[
        "run",
        "--repo",
        repo.to_str().expect("utf-8"),
        "--harness",
        "none",
        "--iterations",
        "1",
        "--test-cmd",
        "true",
        "--protocol",
        "no-such/1",
    ]);
    assert_eq!(code, 1, "the run fails: stdout {out} stderr {err}");
    assert!(
        err.contains("no-such/1"),
        "the error names the protocol: {err}"
    );
    assert!(
        !repo.join(".engineering/cases").exists(),
        "no case was opened"
    );
    assert_eq!(
        status(&repo, "story:ready-one"),
        "proposed",
        "a run that opened no case must not leave the story taken; stdout {out} stderr {err}"
    );
}

/// A repository with no planning store is refused by the binary with exit 1 and the refusal on
/// stderr; the store above it (this repository's own) is never consulted.
#[test]
fn a_repository_without_a_store_exits_one_and_names_the_missing_store() {
    let dir = scratch("no-store");
    fs::write(dir.join("README.md"), "no store\n").expect("a file");
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "start"]);
    let (code, out, err) = takt(&[
        "run",
        "--repo",
        dir.to_str().expect("utf-8"),
        "--harness",
        "none",
        "--iterations",
        "1",
        "--test-cmd",
        "true",
    ]);
    assert_eq!(code, 1, "stdout {out} stderr {err}");
    assert!(err.contains("no planning store"), "{err}");
    assert!(!out.contains("frontier: took"), "{out}");
    assert!(!dir.join(".engineering").exists());
}

/// Nothing ready: the binary says so on stdout and exits 0, with no case directory.
#[test]
fn nothing_ready_exits_zero_from_the_binary() {
    let repo = repository("none-ready", "nothing");
    let (code, out, err) = takt(&[
        "run",
        "--repo",
        repo.to_str().expect("utf-8"),
        "--harness",
        "none",
        "--iterations",
        "1",
        "--test-cmd",
        "true",
    ]);
    assert_eq!(code, 0, "stdout {out} stderr {err}");
    assert_eq!(out, "frontier: nothing ready\n");
    assert_eq!(err, "");
    assert!(!repo.join(".engineering/cases").exists());
}
