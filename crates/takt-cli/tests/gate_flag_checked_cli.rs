//! story:gate-flag-names-a-protocol-action: `takt run --gate <action>` names an
//! approval-required action of the protocol the case runs on, one its `actions` declares with a
//! `requires` entry naming a capability. Another is refused with exit 1, naming the protocol and
//! its approval-required actions, before the case opens or resumes: no case directory, no `takt/`
//! branch, nothing of a resumed case changed. `--case-id` given with `--case` is refused the same
//! way, since a resumed case keeps its own id.

#[path = "../../takt/tests/support/own_home.rs"]
mod own_home;

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

/// A scratch directory under this crate's build directory, removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn repo(&self) -> PathBuf {
        self.0.join("repo")
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

/// A scratch directory holding a committed repository at `<scratch>/repo` with an executable
/// `check.sh`, the test command every run here gives.
fn world(name: &str) -> Scratch {
    use std::os::unix::fs::PermissionsExt as _;
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("gate-flag-checked-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    let repo = dir.join("repo");
    fs::create_dir_all(&repo).expect("the repository");
    let check = repo.join("check.sh");
    fs::write(&check, "#!/bin/sh\nexit 0\n").expect("the test command");
    fs::set_permissions(&check, fs::Permissions::from_mode(0o755)).expect("its mode");
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "start"]);
    Scratch(dir)
}

/// `takt run --repo <repo> --harness none --forge none --test-cmd ./check.sh` with `extra`, run
/// in the repository; its exit code, stdout and stderr.
fn run(scratch: &Scratch, extra: &[&str]) -> (i32, String, String) {
    let repo = scratch.repo();
    let mut args = vec![
        "run",
        "--repo",
        repo.to_str().expect("utf-8"),
        "--harness",
        "none",
        "--forge",
        "none",
        "--test-cmd",
        "./check.sh",
    ];
    args.extend_from_slice(extra);
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args(&args)
        .current_dir(&repo)
        .output()
        .expect("takt runs");
    (
        output.status.code().expect("an exit code"),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// The case directories under `<repo>/.takt/cases`, sorted.
fn case_dirs(scratch: &Scratch) -> Vec<String> {
    let mut entries: Vec<String> = fs::read_dir(scratch.repo().join(".takt/cases"))
        .map(|entries| {
            entries
                .map(|entry| {
                    entry
                        .expect("an entry")
                        .file_name()
                        .to_string_lossy()
                        .into_owned()
                })
                .collect()
        })
        .unwrap_or_default();
    entries.sort();
    entries
}

/// Every file under `dir`, by its path relative to `dir`, with its bytes.
fn files(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, into: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in fs::read_dir(dir).expect("the directory reads") {
            let path = entry.expect("an entry").path();
            if path.is_dir() {
                walk(root, &path, into);
            } else {
                let bytes = fs::read(&path).expect("the file reads");
                into.insert(
                    path.strip_prefix(root).expect("under the root").to_owned(),
                    bytes,
                );
            }
        }
    }
    let mut into = BTreeMap::new();
    walk(dir, dir, &mut into);
    into
}

/// What a refusal must leave as it was: every file of the repository's cases, every branch with
/// its commit, and where the tree is.
fn state(scratch: &Scratch) -> (BTreeMap<PathBuf, Vec<u8>>, String, String) {
    let repo = scratch.repo();
    let cases = repo.join(".takt/cases");
    let cases = if cases.is_dir() {
        files(&cases)
    } else {
        BTreeMap::new()
    };
    (
        cases,
        git(
            &repo,
            &[
                "for-each-ref",
                "--format=%(refname) %(objectname)",
                "refs/heads",
            ],
        ),
        git(&repo, &["rev-parse", "--abbrev-ref", "HEAD"]),
    )
}

/// The story's predicate: `--gate no.such.action` on an intent, which opens on
/// `software.change/1`, is refused with exit 1 naming the protocol and its approval-required
/// actions, `repository.merge` among them, and no case directory and no `takt/` branch exist.
#[test]
fn a_gate_naming_no_approval_required_action_is_refused_before_the_case_opens() {
    let scratch = world("intent");
    let (code, out, err) = run(
        &scratch,
        &[
            "--intent",
            "probe",
            "--iterations",
            "3",
            "--gate",
            "no.such.action",
        ],
    );
    assert_eq!(code, 1, "{out}\n{err}");
    assert!(err.contains("no.such.action"), "the gate given: {err}");
    assert!(err.contains("software.change/1"), "the protocol: {err}");
    assert!(
        err.contains("repository.merge"),
        "the approval-required actions: {err}"
    );
    assert_eq!(
        case_dirs(&scratch),
        Vec::<String>::new(),
        "no case directory"
    );
    assert_eq!(
        git(&scratch.repo(), &["branch", "--list", "takt/*"]),
        "",
        "no case branch"
    );
    assert_eq!(
        git(&scratch.repo(), &["rev-parse", "--abbrev-ref", "HEAD"]),
        "main",
        "the tree stays on main"
    );
}

/// `--gate repository.merge`, an approval-required action of `software.change/1`, gets past the
/// check: the case opens.
#[test]
fn a_gate_naming_an_approval_required_action_gets_past_the_check() {
    let scratch = world("merge");
    let (code, out, err) = run(
        &scratch,
        &[
            "--intent",
            "probe",
            "--case-id",
            "probe-1",
            "--iterations",
            "0",
            "--gate",
            "repository.merge",
        ],
    );
    assert!(
        scratch
            .repo()
            .join(".takt/cases/probe-1/case.yaml")
            .is_file(),
        "the case opens; exit {code}\n{out}\n{err}"
    );
}

/// Opens the case `a` on an intent, with no step taken.
fn open_a(scratch: &Scratch) {
    let (code, out, err) = run(
        scratch,
        &["--intent", "probe", "--case-id", "a", "--iterations", "0"],
    );
    assert!(
        scratch.repo().join(".takt/cases/a/case.yaml").is_file(),
        "the case a opens; exit {code}\n{out}\n{err}"
    );
}

/// On a resume the gate is checked against the resumed case's own protocol: `--case a --gate
/// no.such.action` is refused with exit 1 naming it and its approval-required actions, and
/// nothing is changed.
#[test]
fn a_gate_naming_no_action_of_a_resumed_case_is_refused_before_it_resumes() {
    let scratch = world("resume");
    open_a(&scratch);
    let before = state(&scratch);
    let (code, out, err) = run(
        &scratch,
        &[
            "--case",
            "a",
            "--iterations",
            "0",
            "--gate",
            "no.such.action",
        ],
    );
    assert_eq!(code, 1, "{out}\n{err}");
    assert!(err.contains("no.such.action"), "the gate given: {err}");
    assert!(err.contains("software.change/1"), "the protocol: {err}");
    assert!(
        err.contains("repository.merge"),
        "the approval-required actions: {err}"
    );
    assert!(state(&scratch) == before, "nothing changed:\n{out}\n{err}");
}

/// `--case a --case-id zzz` is refused with exit 1: a resumed case keeps its own id, so the
/// `--case-id` would be dropped. No case `zzz` exists and `a` is as it was.
#[test]
fn a_case_id_given_with_case_is_refused() {
    let scratch = world("case-id");
    open_a(&scratch);
    let before = state(&scratch);
    let (code, out, err) = run(
        &scratch,
        &["--case", "a", "--case-id", "zzz", "--iterations", "0"],
    );
    assert_eq!(code, 1, "{out}\n{err}");
    assert!(err.contains("--case-id zzz"), "the flag named: {err}");
    assert_eq!(case_dirs(&scratch), vec!["a".to_owned()], "no case zzz");
    assert_eq!(
        git(&scratch.repo(), &["branch", "--list", "takt/zzz"]),
        "",
        "no branch takt/zzz"
    );
    assert!(state(&scratch) == before, "a untouched:\n{out}\n{err}");
}
