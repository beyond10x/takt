//! `takt run --case-id <id>` with an id whose branch `takt/<id>` git would not accept exits 1
//! with takt's own message naming the id and the rule, before anything is written: no case
//! directory, no branch, the tree on the branch it was on.

#[path = "../../takt/tests/support/own_home.rs"]
mod own_home;

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

/// The fixture stores of the library crate, one directory each.
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../takt/tests/fixtures");

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
        (
            "a..b",
            "`..`, which a path reads as the directory above, and git refuses takt/a..b as a branch name",
        ),
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
        assert!(!repo.join(".takt/cases").exists(), "{id}: a case dir");
        assert!(
            git(repo, &["branch", "--list", "takt/*"]).is_empty(),
            "{id}: a branch was made"
        );
        assert_eq!(git(repo, &["rev-parse", "--abbrev-ref", "HEAD"]), "main");
    }
}

/// `takt run --case <x>` checks `<x>` before anything under it is read: a relative path that
/// climbs out of the cases directory to a directory holding a `case.yaml` and a
/// `protocol.yaml` is refused with `check_id`'s refusal, never by the protocol it would read.
#[test]
fn run_with_case_refuses_a_path_before_reading_anything_under_it() {
    let scratch = repository("case-path");
    let repo = &scratch.0;
    let planted = repo.join(".engineering/planted");
    fs::create_dir_all(&planted).expect("a directory");
    fs::create_dir_all(repo.join(".takt/cases")).expect("a directory");
    fs::write(planted.join("case.yaml"), "format: canon-case/1\n").expect("a file");
    fs::write(planted.join("protocol.yaml"), "planted: not a protocol\n").expect("a file");
    for (id, rule) in [
        ("../planted", "`..`"),
        ("x/y", "path separator"),
        ("a~b", "`~`"),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_takt"))
            .args(["run", "--repo"])
            .arg(repo)
            .args([
                "--case",
                id,
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
        assert!(
            err.contains(&format!("the case id `{id}` is refused")),
            "{id}: {err}"
        );
        assert!(err.contains(rule), "{id}: {rule} not named: {err}");
        assert!(
            !err.contains("unknown field"),
            "{id}: the planted protocol was read: {err}"
        );
    }
}

/// `takt scenario run` checks the case id it mints, `scenario-<id>-trial-<n>`, before a trial
/// branch is cut: a scenario id long enough to take the case id past 250 bytes is refused with
/// `check_id`'s refusal, no `trial/*` and no `takt/*` branch made, the tree left on `main`.
#[test]
fn scenario_run_refuses_a_minted_case_id_before_any_branch_is_cut() {
    let scratch = repository("scenario");
    let repo = &scratch.0;
    copy_dir(&Path::new(FIXTURES).join("one-ready"), repo);
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "-q", "-m", "seed"]);
    let seed = git(repo, &["rev-parse", "HEAD"]);
    let scenario = "s".repeat(240);
    fs::write(
        repo.join(".engineering/scenarios.conf"),
        format!("[scenario \"{scenario}\"]\n\tstory = story:ready-one\n\tseed = {seed}\n"),
    )
    .expect("a file");
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "-q", "-m", "declare"]);
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args(["scenario", "run", "--repo"])
        .arg(repo)
        .args([
            &scenario,
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
    // Without the bot route the trial number is the instant's digits and a nonce, all digits.
    let refused = err
        .split_once("the case id `")
        .and_then(|(_, rest)| rest.split_once("` is refused"))
        .map(|(id, _)| id)
        .unwrap_or_else(|| panic!("no case id refused: {err}"));
    let trial = refused
        .strip_prefix(&format!("scenario-{scenario}-trial-"))
        .unwrap_or_else(|| panic!("not the minted case id: {refused}"));
    assert!(
        !trial.is_empty() && trial.bytes().all(|b| b.is_ascii_digit()),
        "{refused}"
    );
    assert!(err.contains("250"), "{err}");
    assert!(
        git(repo, &["branch", "--list", "trial/*"]).is_empty(),
        "{err}"
    );
    assert!(
        git(repo, &["branch", "--list", "takt/*"]).is_empty(),
        "{err}"
    );
    assert_eq!(git(repo, &["rev-parse", "--abbrev-ref", "HEAD"]), "main");
}
