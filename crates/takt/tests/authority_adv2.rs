//! Adversary pass 2 against the authority hardening: interleavings across cases in one tree,
//! the evidence reader's promise, and case ids at the edges.

#[path = "support/own_home.rs"]
mod own_home;

use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use takt::case::{Authority, CASES_DIR, CaseDir, grant};
use takt::evidence::Record;
use takt::harness::{Harness, StepContext, StepReport};
use takt::validator::Validator;
use takt::{Decision, LoadedProtocol, Run, RunOptions, Stop};

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
        .join(format!("authority-adv2-{}-{name}", std::process::id()));
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
        "authority-adv2",
    )
}

fn action<'a>(decision: &'a Decision, id: &str) -> &'a str {
    decision
        .actions
        .get(id)
        .map(String::as_str)
        .unwrap_or("absent")
}

/// A case `id` opened under `tree`, one commit on its branch, at the gate there; its revision.
fn at_the_gate(tree: &Path, id: &str) -> String {
    let mut case =
        CaseDir::open(tree, protocol(), id, "deliver the story", &[]).expect("the case opens");
    fs::write(tree.join(format!("{id}.txt")), "done\n").expect("a file");
    git(tree, &["add", &format!("{id}.txt")]);
    git(tree, &["commit", "-q", "-m", id]);
    let revision = git(tree, &["rev-parse", "HEAD"]);
    case.set_revision("implementation", &revision)
        .expect("the revision moves");
    for (n, (kind, result)) in [
        ("test_result", "pass"),
        ("gate_result", "pass"),
        ("pull_request", "open"),
        ("ci_result", "pass"),
    ]
    .iter()
    .enumerate()
    {
        case.add_evidence(&record(n + 1, kind, result, &revision))
            .expect("the record is written");
    }
    let seq = case.evaluations().expect("counts") + 1;
    let decision = case.evaluate(seq).expect("evaluates");
    assert_eq!(
        action(&decision, "repository.merge"),
        "approval-required",
        "{}",
        decision.json
    );
    revision
}

/// Stands in for the operator at a second terminal: while the run on one case is in its
/// judgement step, `takt approve` is given for a sibling case in the same tree, which waits at
/// its gate and which nobody holds. The step itself touches nothing.
struct OperatorApprovesSibling {
    sibling: String,
    granted: RefCell<Option<Result<String, String>>>,
}

impl Harness for OperatorApprovesSibling {
    fn name(&self) -> &str {
        "operator-approves-sibling"
    }

    fn step(&self, context: &StepContext<'_>) -> Result<StepReport, anyhow::Error> {
        if self.granted.borrow().is_none() {
            let answer = grant(
                context.tree,
                &self.sibling,
                "repository.merge",
                "human:operator",
            )
            .map(|approval| approval.revision)
            .map_err(|error| format!("{error:#}"));
            *self.granted.borrow_mut() = Some(answer);
        }
        Ok(StepReport {
            action: context
                .offered
                .first()
                .cloned()
                .unwrap_or_else(|| "nothing-offered".to_owned()),
            summary: "looked around, changed nothing".to_owned(),
            tokens: None,
        })
    }
}

/// A grant given to case B (at its gate, not held) is told to the operator as granted, and then
/// erased: the run on case A in the same tree snapshots the whole cases directory before its
/// judgement step and puts it back after, so B's `authority.yaml` is reverted to "no grant",
/// and A's step is refused as if its harness had written into the cases directory. The hold is
/// per case; the snapshot is per tree.
///
/// This case states today's behaviour, a known defect outside story:authority-hardening: the
/// judgement-step guard in `run.rs` is filed as story:case-scoped-step-guard. When that story
/// lands, flip the assertions: B keeps its grant and A's step is not refused for it.
#[test]
fn adv2_a_grant_on_a_sibling_case_given_during_another_case_s_judgement_step_is_reverted_today() {
    let tree = scratch("sibling");
    let sibling = at_the_gate(&tree, "b");
    git(&tree, &["checkout", "-q", "main"]);

    let harness = OperatorApprovesSibling {
        sibling: "b".to_owned(),
        granted: RefCell::new(None),
    };
    let none: Vec<Box<dyn Validator>> = Vec::new();
    let options = RunOptions {
        protocol: PROTOCOL.to_owned(),
        intent: "deliver another story".to_owned(),
        inputs: Vec::new(),
        iterations: Some(1),
        hours: None,
        tokens: None,
        case_id: Some("a".to_owned()),
        gate: Some("repository.merge".to_owned()),
    };
    let mut run = Run::open(&tree, options, &harness, &none).expect("the run on a opens");
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let out = String::from_utf8_lossy(&out).into_owned();

    let told = harness
        .granted
        .borrow()
        .clone()
        .expect("the step ran, and the operator approved b during it");
    assert_eq!(
        told.as_deref(),
        Ok(sibling.as_str()),
        "the operator's approval of b was refused; run: {stop:?}\n{out}"
    );
    let authority = Authority::read(&tree.join(CASES_DIR).join("b"))
        .expect("b's authority reads")
        .unwrap_or_default();
    assert!(
        !authority.grants_at("repository.merge", &sibling),
        "b kept the grant given during a's judgement step: story:case-scoped-step-guard has landed, so flip this assertion (b keeps its grant at {sibling}); run on a: {stop:?}\n{out}"
    );
    match &stop {
        Ok(Stop::StepRefused(why)) => assert!(
            why.contains(".takt/cases/b/authority.yaml"),
            "a's step was refused for something other than b's grant: {why}; if story:case-scoped-step-guard has landed, flip this assertion (a's step is not refused for a sibling's grant)\n{out}"
        ),
        other => panic!(
            "a's step was not refused for b's grant: {other:?}; if story:case-scoped-step-guard has landed, flip this assertion (a's step is not refused for a sibling's grant)\n{out}"
        ),
    }
}

/// `has_evidence` says: "a record that does not read is an error naming its file, never an
/// answer either way". It answers at the first record that matches and never reads the ones
/// after it, so a broken record after a matching one is an answer, `true`.
#[test]
fn adv2_has_evidence_answers_nothing_while_a_record_does_not_read() {
    let tree = scratch("first-match");
    let mut case = CaseDir::open(&tree, protocol(), "first-match", "deliver the story", &[])
        .expect("the case opens");
    let revision = case
        .revision("implementation")
        .expect("implementation has a revision");
    case.add_evidence(&record(1, "test_result", "pass", &revision))
        .expect("the record is written");
    let evidence = case.dir.join("evidence");
    fs::write(evidence.join("9999-broken.json"), "{ not a record").expect("a broken record");
    let answer = case.has_evidence("test_result", "implementation");
    assert!(
        answer.is_err(),
        "a record that does not read (9999-broken.json) beside a matching one was an answer: {answer:?}"
    );
}

/// Case ids at the edges that `check_id` lets through on Linux: non-ASCII letters, a trailing
/// dot, a `.lock` suffix, Windows device names and drive syntax, a right-to-left override, a
/// line separator, ids of 250 and 300 bytes. Each either opens a case whose directory is one
/// name directly under the cases directory, or is refused with nothing written: no case
/// directory, no branch, the tree left on the branch it was on.
#[test]
fn adv2_probe_edge_case_ids_open_one_directory_or_leave_nothing() {
    let tree = scratch("edges");
    let cases = tree.join(CASES_DIR);
    let long = "a".repeat(250);
    let too_long = "b".repeat(300);
    let ids: Vec<&str> = vec![
        "fix-\u{fc}ber",
        "x.",
        "x.lock",
        "CON",
        "nul",
        "C:evil",
        "a:b",
        "rtl-\u{202e}txt.exe",
        "ls\u{2028}ep",
        "-rf",
        &long,
        &too_long,
    ];
    let mut report = Vec::new();
    for id in ids {
        git(&tree, &["checkout", "-q", "main"]);
        let shown = id.escape_debug().to_string();
        let shown = if shown.len() > 24 {
            format!("{}..({} bytes)", &shown[..12], id.len())
        } else {
            shown
        };
        match CaseDir::open(&tree, protocol(), id, "edge", &[]) {
            Ok(case) => {
                assert_eq!(case.dir.parent(), Some(cases.as_path()), "{shown}");
                assert!(case.dir.join("case.yaml").is_file(), "{shown}");
                report.push(format!("{shown}: opened"));
            }
            Err(error) => {
                let error = format!("{error:#}");
                assert!(
                    !cases.join(id).exists(),
                    "{shown}: a refused id left {error}"
                );
                assert_eq!(
                    git(&tree, &["rev-parse", "--abbrev-ref", "HEAD"]),
                    "main",
                    "{shown}: a refused id moved the tree: {error}"
                );
                let branch = format!("takt/{id}");
                assert!(
                    git(&tree, &["branch", "--list", &branch]).is_empty(),
                    "{shown}: a refused id left its branch: {error}"
                );
                report.push(format!("{shown}: refused"));
            }
        }
    }
    eprintln!("{}", report.join("\n"));
}
