//! The `takt` binary around the merge under authority, at its edges: the run after a story's
//! case completed, and a run with no harness on a case that holds everything but the grant.
//! Adversary pass 1 over story:merge-under-authority.

#[path = "../../takt/tests/support/own_home.rs"]
mod own_home;

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

/// The story-delivery protocol, the engineering protocols' built-in.
const DELIVERY_PROTOCOL: &str = "story.delivery/1";

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
    fs::write(
        dir.join(".engineering/commit-identity.conf"),
        "[identity]\n\tname = Test Bot\n\temail = bot@example.invalid\n",
    )
    .expect("the commit identity is written");
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

/// A scratch directory beside `repo`, `<repo>-forge`, holding a bare hub `hub.git` with
/// `repo`'s `main`, named by `repo`'s `origin`, and a run file `run.yaml` whose forge is `local`
/// with `true` as its checks: the directory, the hub and the run file. Nothing reaches a network.
fn local_hub(repo: &Path) -> (Scratch, PathBuf, PathBuf) {
    let outer = PathBuf::from(format!("{}-forge", repo.display()));
    if outer.exists() {
        fs::remove_dir_all(&outer).expect("a stale hub is removed");
    }
    fs::create_dir_all(&outer).expect("a directory");
    let hub = outer.join("hub.git");
    git(
        repo,
        &[
            "init",
            "-q",
            "--bare",
            "-b",
            "main",
            hub.to_str().expect("utf-8"),
        ],
    );
    git(
        repo,
        &["remote", "add", "origin", hub.to_str().expect("utf-8")],
    );
    git(repo, &["push", "-q", "origin", "main"]);
    let config = outer.join("run.yaml");
    fs::write(&config, "forge: local\nlocal_checks: [\"true\"]\n").expect("the run file");
    (Scratch(outer), hub, config)
}

/// `git <args>` in `tree`; its standard output, trimmed.
fn git_out(tree: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(tree)
        .output()
        .expect("git runs");
    assert!(output.status.success(), "git {args:?}");
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// The whole delivery through the binary, on the default protocol for a store story and the
/// local forge, whose hub is a bare repository on this machine: a plain run takes the ready
/// story; the operator commits the work on the case's branch; the next plain run resumes the
/// case, runs the tests and the gate, opens the pull request, reads its checks and stops at the
/// gate, exit 4; `takt approve` grants the merge, records it on the story and commits that
/// record on the case's branch; the next plain run resumes the case on a clean tree at the
/// revision the grant was given at and merges the pull request through the forge, once, and the
/// case completes, exit 0.
#[test]
fn approve_then_the_resumed_run_merges() {
    let repo = repository("one-ready", "approve-merge");
    let (_forge, hub, config) = local_hub(&repo);
    let start = git_out(&hub, &["rev-parse", "refs/heads/main"]);
    let path = repo.to_str().expect("utf-8");
    let run = [
        "run",
        "--repo",
        path,
        "--harness",
        "none",
        "--test-cmd",
        "true",
        "--protocol",
        "story.delivery/1",
        "--config",
        config.to_str().expect("utf-8"),
    ];
    let mut first = run.to_vec();
    first.extend(["--iterations", "0"]);
    let (code, out, err) = takt(&first);
    assert_eq!(code, 3, "stdout {out} stderr {err}");
    assert!(
        out.contains("frontier: took story:ready-one"),
        "stdout {out} stderr {err}"
    );
    assert!(out.contains(" on story.delivery/1 in "), "{out}");
    assert!(out.contains("forge: local ("), "{out}");
    let id = git_out(&repo, &["branch", "--show-current"])
        .strip_prefix("takt/")
        .expect("the tree is on the case's branch")
        .to_owned();

    fs::write(repo.join("feature.txt"), "the work\n").expect("a file");
    git(&repo, &["add", "feature.txt"]);
    git(&repo, &["commit", "-q", "-m", "the work"]);
    let worked = git_out(&repo, &["rev-parse", "HEAD"]);

    let (code, out, err) = takt(&run);
    assert_eq!(code, 4, "stdout {out} stderr {err}");
    assert!(
        out.contains(&format!(
            "frontier: resumed story:ready-one: Make the check pass (case {id})"
        )),
        "{out}"
    );
    for line in [
        "evidence: test_result pass",
        "evidence: gate_result pass",
        "evidence: pull_request open",
        "evidence: ci_result pass",
        "stopped: ApprovalRequired (repository.merge)",
    ] {
        assert!(out.contains(line), "{line}: stdout {out} stderr {err}");
    }
    assert_eq!(
        git_out(&hub, &["rev-parse", "refs/heads/main"]),
        start,
        "nothing merged before the grant"
    );

    let (code, out, err) = takt(&[
        "approve",
        "--repo",
        path,
        &id,
        "repository.merge",
        "--by",
        "human:operator",
    ]);
    assert_eq!(code, 0, "stdout {out} stderr {err}");
    assert_eq!(
        git_out(&repo, &["log", "-1", "--format=%s"]),
        "takt: approve story:ready-one repository.merge"
    );
    assert_eq!(git_out(&repo, &["rev-parse", "HEAD^"]), worked);
    assert_eq!(
        git_out(
            &repo,
            &["status", "--porcelain", "--", ".", ":!.takt/cases"]
        ),
        ""
    );

    let (code, out, err) = takt(&run);
    assert_eq!(code, 0, "stdout {out} stderr {err}");
    assert!(
        out.contains(&format!(
            "frontier: resumed story:ready-one: Make the check pass (case {id})"
        )),
        "{out}"
    );
    assert!(!out.contains("refused"), "stdout {out} stderr {err}");
    assert!(
        out.contains(&format!(
            "merged pull request #1 at {} through local: merged",
            &worked[..8]
        )),
        "stdout {out} stderr {err}"
    );
    assert!(
        out.contains("evidence: merge_observation merged"),
        "stdout {out} stderr {err}"
    );
    assert!(
        out.contains("stopped: Completed"),
        "stdout {out} stderr {err}"
    );
    assert_eq!(
        git_out(
            &hub,
            &["rev-list", "--merges", "--count", &format!("{start}..main")]
        ),
        "1",
        "merged once"
    );
    assert_eq!(
        git_out(
            &repo,
            &["status", "--porcelain", "--", ".", ":!.takt/cases"]
        ),
        ""
    );
}

/// A run with no harness on a case that holds everything but the grant: the four records are
/// true at the revision, `repository.merge` is approval-required, and nothing deterministic is
/// left. The story: the run stops at `ApprovalRequired (repository.merge)` with nothing merged,
/// exit 4, and `takt approve` is the next step.
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
    // The change handed over at the revision, as takt records a `change.handover` step.
    case.add_evidence(&takt::evidence::Record::new(
        "handover",
        "handover",
        None,
        "implementation",
        &revision,
        takt::run::CHOICE_PRODUCER,
    ))
    .expect("the record is written");
    // A change to publish at the revision, as `change.read` records it.
    case.add_evidence(&takt::evidence::Record::new(
        "change-record",
        "change_record",
        Some("ahead".to_owned()),
        "implementation",
        &revision,
        "takt.change-read",
    ))
    .expect("the record is written");
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
    assert_eq!(
        code, 4,
        "ApprovalRequired exits 4, not Completed's 0: stdout {out} stderr {err}"
    );
    assert!(
        out.contains("stopped: ApprovalRequired (repository.merge)"),
        "stdout {out} stderr {err}"
    );
}
