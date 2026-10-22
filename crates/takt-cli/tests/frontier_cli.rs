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

/// The case is opened before the store is told. When the open fails — here a protocol id
/// nothing provides, the same as a relative `--protocol` path given from the wrong directory
/// (README.md:24-25) — the run exits 1, no case directory is left, and the story stays
/// `proposed`, so a later run can take it: the store is told only once the run has a case to
/// work.
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

/// The names under `<repo>/.engineering/cases`, sorted; `None` when the directory is absent.
fn case_names(repo: &Path) -> Option<Vec<String>> {
    let dir = repo.join(".engineering/cases");
    if !dir.exists() {
        return None;
    }
    let mut names: Vec<String> = fs::read_dir(&dir)
        .expect("a readable cases directory")
        .map(|e| {
            e.expect("an entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    Some(names)
}

/// Runs the built `takt` binary with `args` and `aep` on the path replaced by the script at
/// `stub`: the binary asks `Aep::default()`, which is `aep` by name.
fn takt_with_aep(stub_dir: &Path, args: &[&str]) -> (i32, String, String) {
    let path = format!(
        "{}:{}",
        stub_dir.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args(args)
        .env("PATH", path)
        .output()
        .expect("takt runs");
    (
        output.status.code().expect("an exit code"),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// An `aep` on the path that lists three `proposed` stories, shows any, and refuses to move
/// every one of them; written as `aep` into a directory of its own under `dir`.
fn refuse_all_aep(dir: &Path) -> PathBuf {
    let bin = dir.join("stub-bin");
    fs::create_dir_all(&bin).expect("a stub directory");
    let script = r##"#!/bin/sh
case "$3" in
  list)
    printf '%s\n' '[{"id":"story:c","kind":"story","status":"proposed","title":"C","path":"story/c.md","relations":[],"refs":[],"blocked_by":[]},{"id":"story:a","kind":"story","status":"proposed","title":"A","path":"story/a.md","relations":[],"refs":[],"blocked_by":[]},{"id":"story:b","kind":"story","status":"proposed","title":"B","path":"story/b.md","relations":[],"refs":[],"blocked_by":[]}]'
    ;;
  show)
    printf '{"id":"%s","kind":"story","status":"proposed","title":"Title of %s","body":"# Story\\n"}\n' "$4" "$4"
    ;;
  move)
    printf '%s\n' "$4 is proposed; the profile wants it to serve an objective first" >&2
    exit 1
    ;;
  *)
    printf 'unexpected: %s\n' "$*" >&2
    exit 2
    ;;
esac
"##;
    let path = bin.join("aep");
    fs::write(&path, script).expect("the stub is written");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("executable");
    }
    // A case opens only on a clean tree: the stub is the test's, not work in progress.
    let exclude = dir.join(".git/info/exclude");
    let mut text = fs::read_to_string(&exclude).unwrap_or_default();
    text.push_str("stub-bin/\n");
    fs::write(&exclude, text).expect("the stub is ignored");
    bin
}

/// A case input outside the protocol's declared values: `CaseDir::open` refuses it before it
/// writes anything, so the run exits 1, asks for no move (the story stays `proposed`) and
/// leaves no case directory. An operator's typo in `--input risk=…` reaches it. The protocol is
/// `software.change/1`, which declares `risk`, so the value check is what refuses it, not the
/// binary's refusal of an input the protocol does not declare.
#[test]
fn a_run_whose_input_is_refused_leaves_the_story_proposed_and_no_case_directory() {
    let repo = repository("one-ready", "adv2-input");
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
        "software.change/1",
        "--input",
        "risk=bogus",
    ]);
    assert_eq!(code, 1, "the run fails: stdout {out} stderr {err}");
    assert!(
        err.contains("the input risk takes one of") && err.contains("not `bogus`"),
        "the value check of CaseDir::open names the input: {err}"
    );
    assert!(!out.contains("frontier: took"), "{out}");
    assert_eq!(
        status(&repo, "story:ready-one"),
        "proposed",
        "stdout {out} stderr {err}"
    );
    assert_eq!(
        case_names(&repo),
        None,
        "a run that took no story leaves no case directory; stdout {out} stderr {err}"
    );
}

/// `--case-id` with no `--intent` names the story's case from the binary: the id is passed
/// through to the case opened on the taken story, and the store lists the story `active`.
#[test]
fn a_given_case_id_names_the_story_case_from_the_binary() {
    let repo = repository("one-ready", "adv2-case-id");
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
        "--case-id",
        "given",
    ]);
    assert_eq!(
        code, 7,
        "the gate with no gates document, ahead of the step budget: stdout {out} stderr {err}"
    );
    assert!(
        out.starts_with("frontier: took story:ready-one: Make the check pass\n"),
        "{out}"
    );
    assert_eq!(case_names(&repo), Some(vec!["given".to_owned()]));
    let intent =
        fs::read_to_string(repo.join(".engineering/cases/given/intent.md")).expect("intent.md");
    assert!(
        intent.starts_with("Make the check pass\n\n# Story: ready-one\n"),
        "{intent}"
    );
    assert_eq!(status(&repo, "story:ready-one"), "active");
}

/// Every ready story refused by the store, through the binary: each refusal is printed
/// verbatim in id order, then `frontier: nothing ready`, exit 0, and no `cases/` directory is
/// left, since each refused story's case was opened and removed in turn.
#[test]
fn a_store_refusing_every_story_exits_zero_with_nothing_ready_and_no_cases_directory() {
    let repo = repository("one-ready", "adv2-all-refused");
    let bin = refuse_all_aep(&repo);
    let (code, out, err) = takt_with_aep(
        &bin,
        &[
            "run",
            "--repo",
            repo.to_str().expect("utf-8"),
            "--harness",
            "none",
            "--iterations",
            "1",
            "--test-cmd",
            "true",
        ],
    );
    assert_eq!(code, 0, "stdout {out} stderr {err}");
    assert_eq!(err, "");
    assert_eq!(
        out,
        "frontier: story:a not taken: story:a is proposed; the profile wants it to serve an objective first\n\
         frontier: story:b not taken: story:b is proposed; the profile wants it to serve an objective first\n\
         frontier: story:c not taken: story:c is proposed; the profile wants it to serve an objective first\n\
         frontier: nothing ready\n"
    );
    assert_eq!(
        case_names(&repo),
        None,
        "no case and no empty cases directory after every story was refused"
    );
}
