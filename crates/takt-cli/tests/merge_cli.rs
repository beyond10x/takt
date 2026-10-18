//! The `takt` binary around the merge under authority, at its edges: the run after a story's
//! case completed, and a run with no harness on a case that holds everything but the grant.
//! Adversary pass 1 over story:merge-under-authority.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use takt::CaseDir;
use takt::evidence::Record;

/// The fixture stores of the library crate, one directory each.
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../takt/tests/fixtures");

/// The fixture protocol of the library crate that declares `gate_result` and `gate.green`.
const GATE_PROTOCOL: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../takt/tests/fixtures/gate-only/1.yaml"
);

/// The story-delivery protocol, relative to this crate.
const DELIVERY_PROTOCOL: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../protocols/story-delivery/1.yaml"
);

/// A gates document whose one assertion is true in a repository with a manifest.
const GATES: &str = "format: engineering-gates/1\nlanguage: canon-expr/1\ngates:\n  - id: manifest\n    \
                     text: The repository has its manifest.\n    assertions:\n      \
                     - 'fs.file.exists(\"Cargo.toml\")'\n";

fn git(tree: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(args)
        .current_dir(tree)
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?}");
}

/// A scratch directory under this crate's build directory, removed on drop.
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

/// A committed copy of the fixture store `fixture` with a manifest and a gates document whose
/// one assertion is true, in a scratch tree of its own: `name` keeps the cases of this process
/// apart.
fn repository(fixture: &str, name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("merge-cli-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    copy_dir(&Path::new(FIXTURES).join(fixture), &dir);
    fs::write(dir.join("Cargo.toml"), "[package]\nname = \"scratch\"\n").expect("a file");
    fs::write(dir.join(".engineering/gates.yaml"), GATES).expect("a file");
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "start"]);
    Scratch(dir)
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

/// `takt run --repo R` with no intent and no case, twice: the first takes the ready story and
/// completes its case through the gate; the second finds the story `active` with a finished
/// case and takes nothing else. A finished case is not an open case, so the second run does not
/// resume it: it says nothing is ready, or takes the next ready story.
#[test]
fn the_run_after_a_story_case_completed_does_not_resume_it() {
    let repo = repository("one-ready", "completed");
    let path = repo.to_str().expect("utf-8");
    let args = [
        "run",
        "--repo",
        path,
        "--harness",
        "none",
        "--protocol",
        GATE_PROTOCOL,
        "--test-cmd",
        "true",
    ];
    let (code, out, err) = takt(&args);
    assert_eq!(code, 0, "stdout {out} stderr {err}");
    assert!(
        out.contains("frontier: took story:ready-one"),
        "stdout {out} stderr {err}"
    );
    assert!(
        out.contains("stopped: Completed (accepted)"),
        "stdout {out} stderr {err}"
    );
    let (code, out, err) = takt(&args);
    assert!(
        !out.contains("frontier: resumed story:ready-one"),
        "the finished case is resumed by every later run on the repository, and no other story is ever taken: exit {code} stdout {out} stderr {err}"
    );
    assert!(
        out.contains("frontier: nothing ready"),
        "exit {code} stdout {out} stderr {err}"
    );
}

/// A run with no harness on a case that holds everything but the grant: the four records are
/// true at the revision, `repository.merge` is approval-required, and nothing deterministic is
/// left. The story: the run stops at `ApprovalRequired (repository.merge)` with nothing merged,
/// exit 0, and `takt approve` is the next step.
#[test]
fn a_run_with_no_harness_on_a_case_that_needs_only_the_grant_stops_approval_required() {
    let repo = repository("one-ready", "no-harness");
    let path = repo.to_str().expect("utf-8");
    let (code, out, err) = takt(&[
        "run",
        "--repo",
        path,
        "--intent",
        "deliver the story",
        "--case-id",
        "gated",
        "--protocol",
        DELIVERY_PROTOCOL,
        "--harness",
        "none",
        "--forge",
        "none",
        "--test-cmd",
        "true",
        "--iterations",
        "0",
    ]);
    assert_eq!(code, 3, "stdout {out} stderr {err}");
    assert!(
        out.contains("stopped: Iterations (0)"),
        "stdout {out} stderr {err}"
    );
    // The records the delivery produces up to the merge, as the validators write them.
    let mut case = CaseDir::resume(&repo, "gated").expect("the case reads back");
    let revision = case
        .revision("implementation")
        .expect("implementation has a revision");
    for (n, (kind, result)) in [
        ("test_result", "pass"),
        ("gate_result", "pass"),
        ("pull_request", "open"),
        ("ci_result", "pass"),
    ]
    .iter()
    .enumerate()
    {
        case.add_evidence(&Record::new(
            format!("e{n}"),
            *kind,
            Some((*result).to_owned()),
            "implementation",
            &revision,
            "merge-cli-test",
        ))
        .expect("the record is written");
    }
    let (code, out, err) = takt(&[
        "run",
        "--repo",
        path,
        "--case",
        "gated",
        "--harness",
        "none",
        "--forge",
        "none",
        "--test-cmd",
        "true",
    ]);
    assert_eq!(code, 0, "stdout {out} stderr {err}");
    assert!(
        out.contains("stopped: ApprovalRequired (repository.merge)"),
        "stdout {out} stderr {err}"
    );
}
