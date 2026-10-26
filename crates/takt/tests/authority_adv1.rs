//! Adversary pass 1 against the authority hardening: the cases the unit's own suite does not
//! state.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use takt::case::{CaseDir, grant};
use takt::evidence::Record;
use takt::{Decision, LoadedProtocol};

const PROTOCOL: &str = "story.delivery/1";

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
    String::from_utf8(output.stdout)
        .expect("utf-8")
        .trim()
        .to_owned()
}

struct Scratch(PathBuf);

impl std::ops::Deref for Scratch {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn scratch(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("authority-adv1-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(&dir).expect("scratch dir");
    fs::write(dir.join("README.md"), "a story\n").expect("a file");
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "start"]);
    Scratch(dir)
}

fn protocol() -> LoadedProtocol {
    LoadedProtocol::builtin(PROTOCOL).expect("the protocol loads")
}

fn record(n: usize, kind: &str, result: &str, revision: &str) -> Record {
    Record::new(
        format!("e{n}"),
        kind,
        Some(result.to_owned()),
        "implementation",
        revision,
        "authority-adv1",
    )
}

fn action<'a>(decision: &'a Decision, id: &str) -> &'a str {
    decision
        .actions
        .get(id)
        .map(String::as_str)
        .unwrap_or("absent")
}

fn evaluate(case: &CaseDir) -> Decision {
    let seq = case.evaluations().expect("counts") + 1;
    case.evaluate(seq).expect("evaluates")
}

/// The four records the delivery owes up to the merge, at the case's current revision, then one
/// evaluation, which finds `repository.merge` approval-required.
fn to_the_gate(case: &mut CaseDir, first: usize) {
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
        case.add_evidence(&record(first + n, kind, result, &revision))
            .expect("the record is written");
    }
    let decision = evaluate(case);
    assert_eq!(
        action(&decision, "repository.merge"),
        "approval-required",
        "{}",
        decision.json
    );
}

/// The gap between `resume` and the run's hold: run A resumes the case at R1 (the grant's
/// revision) and lets go; before A takes its hold, a commit R2 lands on the case's branch and
/// run B resumes the case, brings it to the gate at R2 and lets go. A then holds and evaluates.
/// Canon is handed `case.yaml` from disk (R2, its evidence complete at R2) and the authority
/// filtered at A's in-memory revision (R1), so the grant given at R1 admits the merge at R2:
/// "repository.merge is not admitted at any other revision" does not hold.
#[test]
fn adv1_a_run_resumed_before_another_moved_the_case_is_not_admitted_the_merge_at_the_new_revision()
{
    let tree = scratch("stale");
    let mut case = CaseDir::open(&tree, protocol(), "stale", "deliver the story", &[])
        .expect("the case opens");
    fs::write(tree.join("implemented.txt"), "done\n").expect("a file");
    git(&tree, &["add", "implemented.txt"]);
    git(&tree, &["commit", "-q", "-m", "implemented"]);
    let first = git(&tree, &["rev-parse", "HEAD"]);
    case.set_revision("implementation", &first)
        .expect("the revision moves");
    to_the_gate(&mut case, 1);
    let approval = grant(&tree, "stale", "repository.merge", "human:operator").expect("granted");
    assert_eq!(approval.revision, first);
    drop(case);

    // Run A: resumed at R1, not yet holding (what `Run::resume` leaves for `Run::run`).
    let a = CaseDir::resume(&tree, "stale").expect("A resumes");
    assert_eq!(
        a.revision("implementation").as_deref(),
        Some(first.as_str())
    );

    // A commit on the case's branch, and run B, which resumes, reaches the gate at R2, lets go.
    fs::write(tree.join("after-the-grant.txt"), "unreviewed\n").expect("a file");
    git(&tree, &["add", "after-the-grant.txt"]);
    git(&tree, &["commit", "-q", "-m", "after the grant"]);
    let second = git(&tree, &["rev-parse", "HEAD"]);
    {
        let mut b = CaseDir::resume(&tree, "stale").expect("B resumes");
        assert_eq!(
            b.revision("implementation").as_deref(),
            Some(second.as_str())
        );
        b.hold().expect("B holds");
        to_the_gate(&mut b, 5);
        b.release().expect("B lets go");
    }

    // Run A takes its hold and evaluates.
    a.hold().expect("A holds");
    let decision = evaluate(&a);
    a.release().expect("A lets go");
    assert_ne!(
        action(&decision, "repository.merge"),
        "admissible",
        "the grant given at {first} admitted the merge with the case at {second}: {}",
        decision.json
    );
}

/// `has_evidence` says of itself: "a record that does not read is an error naming its file,
/// never an answer either way". A record cut short to nothing (a write that created the file
/// and died) does not read as a record; `has_evidence` is an error naming its file, not `false`.
#[test]
fn adv1_an_empty_evidence_record_is_an_error_from_has_evidence() {
    let tree = scratch("empty");
    let case = CaseDir::open(&tree, protocol(), "empty", "deliver the story", &[])
        .expect("the case opens");
    fs::write(case.dir.join("evidence").join("0001-cut.json"), "").expect("an empty record");
    let answer = case.has_evidence("test_result", "implementation");
    assert!(answer.is_err(), "an empty record was an answer: {answer:?}");
    let error = format!("{:#}", answer.expect_err("checked"));
    assert!(error.contains("0001-cut.json"), "{error}");
}

/// Reachability probe for the case above: the run evaluates before it asks `has_evidence`, and
/// the evaluation refuses an empty record naming its file.
#[test]
fn adv1_probe_evaluate_refuses_an_empty_record_naming_it() {
    let tree = scratch("empty-eval");
    let case = CaseDir::open(&tree, protocol(), "empty-eval", "deliver the story", &[])
        .expect("the case opens");
    fs::write(case.dir.join("evidence").join("0001-cut.json"), "").expect("an empty record");
    let error = format!("{:#}", case.evaluate(1).expect_err("an empty record"));
    assert!(error.contains("0001-cut.json"), "{error}");
}
