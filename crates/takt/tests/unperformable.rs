//! story:forge-none-stops-when-only-forge-actions-remain and
//! story:delivery-refuses-an-obligation-nobody-performs: a run stops `Unperformable`, naming the
//! claim and the performer it lacks, at the first step where no offered or due action produces
//! evidence an open obligation's claim reads and an admissible action nobody in the run performs
//! is the only producer of a claim that does not hold. It takes no harness call there, and a
//! resume takes none either. With the performer registered, nothing changes.

use std::cell::Cell;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

use takt::evidence::Record;
use takt::forge::{Checks, Forge, Memory};
use takt::harness::{Harness, StepContext, StepReport};
use takt::scorecard::{HeldOut, Row};
use takt::validator::{ChangeRead, CiRead, GateRun, PrMerged, PrOpen, TestsRun, Validator};
use takt::{Run, RunOptions};

const PROTOCOL: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../protocols/story-delivery/1.yaml"
);

/// A gates document whose one assertion is true in a repository with a manifest.
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

/// A committed scratch repository with a manifest and, when `gates`, a gates document.
fn scratch(name: &str, gates: bool) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("unperformable-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(&dir).expect("scratch dir");
    fs::write(dir.join("Cargo.toml"), "[package]\nname = \"scratch\"\n").expect("a file");
    if gates {
        fs::create_dir_all(dir.join(".engineering")).expect("a directory");
        fs::write(dir.join(".engineering/gates.yaml"), GATES).expect("a file");
    }
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "start"]);
    Scratch(dir)
}

/// Counts its calls; hands the change over when offered, else reports an edit that changes
/// nothing. Each call reports tokens, as a model does.
#[derive(Default)]
struct Counting {
    calls: Cell<usize>,
}

impl Harness for Counting {
    fn name(&self) -> &str {
        "counting"
    }

    fn step(&self, context: &StepContext<'_>) -> anyhow::Result<StepReport> {
        self.calls.set(self.calls.get() + 1);
        let action = if context.offered.contains(&"change.handover".to_owned()) {
            "change.handover"
        } else {
            "implementation.edit"
        };
        Ok(StepReport {
            action: action.to_owned(),
            summary: "the double".to_owned(),
            tokens: Some(29_000),
        })
    }
}

fn options(name: &str) -> RunOptions {
    RunOptions {
        protocol: PROTOCOL.to_owned(),
        intent: "deliver the story".to_owned(),
        inputs: Vec::new(),
        iterations: Some(10),
        hours: None,
        tokens: None,
        case_id: Some(name.to_owned()),
        gate: Some("repository.merge".to_owned()),
    }
}

/// The validators `takt run` registers for a delivery: the tests and the change read always,
/// the gate with a gates document, the forge's three with a forge.
fn validators(gates: bool, forge: Option<Rc<dyn Forge>>) -> Vec<Box<dyn Validator>> {
    let mut validators: Vec<Box<dyn Validator>> = vec![
        Box::new(TestsRun::new("true").expect("a command")),
        Box::new(ChangeRead),
    ];
    if gates {
        validators.push(Box::new(GateRun::new(".engineering/gates.yaml")));
    }
    if let Some(forge) = forge {
        validators.push(Box::new(PrOpen::new(forge.clone())));
        validators.push(Box::new(CiRead::new(forge.clone())));
        validators.push(Box::new(PrMerged::new(forge)));
    }
    validators
}

fn record(id: &str, kind: &str, result: Option<&str>, revision: &str, producer: &str) -> Record {
    Record::new(
        id,
        kind,
        result.map(str::to_owned),
        "implementation",
        revision,
        producer,
    )
}

/// Opens the case `name` in `tree` and moves it to "ahead": the work committed, its tests and
/// gate passing, the change handed over and a change to publish recorded, as the steps before
/// record them.
fn ahead(tree: &Path, name: &str, validators: &[Box<dyn Validator>], harness: &dyn Harness) {
    let mut run = Run::open(tree, options(name), harness, validators).expect("the case opens");
    fs::write(tree.join("work.txt"), "done\n").expect("a file");
    git(tree, &["add", "work.txt"]);
    git(tree, &["commit", "-q", "-m", "the work"]);
    let revision = git(tree, &["rev-parse", "HEAD"]);
    run.case
        .set_revision("implementation", &revision)
        .expect("the revision moves");
    for evidence in [
        record("e1", "test_result", Some("pass"), &revision, "test"),
        record("e2", "gate_result", Some("pass"), &revision, "test"),
        record(
            "e3",
            "handover",
            None,
            &revision,
            takt::run::CHOICE_PRODUCER,
        ),
        record(
            "e4",
            "change_record",
            Some("ahead"),
            &revision,
            "takt.change-read",
        ),
    ] {
        run.case.add_evidence(&evidence).expect("written");
    }
}

/// Resumes the case `name` and runs it to its stop: the stop as printed, and the output.
fn resume(
    tree: &Path,
    name: &str,
    validators: &[Box<dyn Validator>],
    harness: &dyn Harness,
) -> (String, String) {
    let mut run =
        Run::resume(tree, options(name), name, harness, validators).expect("the case resumes");
    run.stop_when_unperformable();
    let mut out = Vec::new();
    let stop = run.run(&mut out).expect("the run stops without an error");
    (stop.to_string(), String::from_utf8(out).expect("utf-8"))
}

/// Story forge-none, its acceptance: with no forge registered, a case at "ahead" stops naming
/// `pr.opened` and the forge, with no harness call; a resume of it takes none either.
#[test]
fn with_no_forge_a_case_ahead_stops_unperformable_and_a_resume_calls_no_harness() {
    let tree = scratch("forge-none", false);
    let harness = Counting::default();
    let validators = validators(false, None);
    ahead(&tree, "forge-none", &validators, &harness);

    let (stop, text) = resume(&tree, "forge-none", &validators, &harness);
    assert_eq!(
        stop, "Unperformable (pr.opened; nothing performs it without the forge)",
        "{text}"
    );
    assert_eq!(harness.calls.get(), 0, "a harness call after ahead: {text}");

    let (stop, text) = resume(&tree, "forge-none", &validators, &harness);
    assert_eq!(
        stop, "Unperformable (pr.opened; nothing performs it without the forge)",
        "{text}"
    );
    assert_eq!(
        harness.calls.get(),
        0,
        "the resume called the harness: {text}"
    );
}

/// Story forge-none, its second bullet: with a forge the same case opens its pull request and
/// stops for the merge's grant, as it did before.
#[test]
fn with_a_forge_a_case_ahead_opens_its_pull_request_as_before() {
    let tree = scratch("forge-memory", false);
    let harness = Counting::default();
    let memory = Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    });
    let validators = validators(false, Some(memory.clone()));
    ahead(&tree, "forge-memory", &validators, &harness);

    let (stop, text) = resume(&tree, "forge-memory", &validators, &harness);
    assert_eq!(stop, "ApprovalRequired (repository.merge)", "{text}");
    assert!(text.contains("evidence: pull_request open"), "{text}");
    assert_eq!(memory.opened.borrow().len(), 1, "{text}");
    assert_eq!(harness.calls.get(), 0, "{text}");
}

/// Story delivery-refuses, its acceptance: a delivery case in a repository with no gates
/// document stops after its tests ran, before any harness call, naming `gate.green` and the
/// gates document; a forge is registered, so nothing else is missing.
#[test]
fn without_a_gates_document_a_delivery_stops_before_any_harness_call() {
    let tree = scratch("no-gates", false);
    let harness = Counting::default();
    let forge: Rc<dyn Forge> = Rc::new(Memory::default());
    let validators = validators(false, Some(forge));
    let mut run =
        Run::open(&tree, options("no-gates"), &harness, &validators).expect("the case opens");
    run.stop_when_unperformable();
    let mut out = Vec::new();
    let stop = run.run(&mut out).expect("the run stops without an error");
    let text = String::from_utf8(out).expect("utf-8");
    assert_eq!(
        stop.to_string(),
        "Unperformable (gate.green; nothing performs it without the gates document)",
        "{text}"
    );
    assert_eq!(harness.calls.get(), 0, "a harness call: {text}");
    assert_eq!(run.tokens, 0, "{text}");
    assert!(text.contains("step 1: tests.run {}"), "{text}");
    assert!(!text.contains("step 2:"), "{text}");
}

/// The library's default: a caller that does not turn the rule on
/// (`Run::stop_when_unperformable`, which the CLI calls for every run) gets the run as before,
/// the harness offered its step in the same case that stops `Unperformable` above.
#[test]
fn without_the_opt_in_a_library_run_offers_the_harness_its_step() {
    let tree = scratch("no-gates-default", false);
    let harness = Counting::default();
    let forge: Rc<dyn Forge> = Rc::new(Memory::default());
    let validators = validators(false, Some(forge));
    let mut run = Run::open(&tree, options("no-gates-default"), &harness, &validators)
        .expect("the case opens");
    let mut out = Vec::new();
    let stop = run.run(&mut out).expect("the run stops without an error");
    let text = String::from_utf8(out).expect("utf-8");
    assert!(harness.calls.get() >= 1, "stopped {stop}: {text}");
    assert!(!stop.to_string().starts_with("Unperformable"), "{text}");
}

/// Story delivery-refuses, its second bullet: with a gates document the gate runs and the
/// harness is offered its judgement step, as before.
#[test]
fn with_a_gates_document_a_delivery_runs_the_gate_and_offers_the_harness_its_step() {
    let tree = scratch("gates", true);
    let harness = Counting::default();
    let forge: Rc<dyn Forge> = Rc::new(Memory::default());
    let validators = validators(true, Some(forge));
    let mut run =
        Run::open(&tree, options("gates"), &harness, &validators).expect("the case opens");
    run.stop_when_unperformable();
    let mut out = Vec::new();
    let stop = run.run(&mut out).expect("the run stops without an error");
    let text = String::from_utf8(out).expect("utf-8");
    assert!(text.contains("evidence: gate_result pass"), "{text}");
    assert!(harness.calls.get() >= 1, "stopped {stop}: {text}");
    assert!(!stop.to_string().starts_with("Unperformable"), "{text}");
}

/// The scorecard counts an `Unperformable` run as no completion: a scenario run that stopped
/// so, its held-out check failing, does not disagree with it.
#[test]
fn an_unperformable_stop_is_no_completion() {
    let row = Row {
        case: "scenario-s-trial-0".to_owned(),
        scenario: Some("s".to_owned()),
        outcome: "failed".to_owned(),
        stop: "Unperformable".to_owned(),
        held_out: Some(HeldOut {
            passed: false,
            result: "fail".to_owned(),
        }),
        steps: 3,
        tokens: 0,
        refusals: 0,
        no_progress: 0,
        errors: 0,
        wall_ms: 0,
        saturated: false,
        binary: None,
        opened_binary: None,
        completed: None,
    };
    assert!(!row.disagrees());
}

/// Writes the case `case` under `repo` with one run per stop line, each `(stop, completed)`;
/// `completed` none writes a line without the field, as a binary before it did.
fn stopped_runs(repo: &Path, case: &str, stops: &[(&str, Option<bool>)]) {
    let dir = repo.join(".engineering/cases").join(case);
    fs::create_dir_all(&dir).expect("a case directory");
    let mut text = String::new();
    for (n, (stop, completed)) in stops.iter().enumerate() {
        let at = format!("2026-10-19T10:0{n}:00.000000Z");
        text.push_str(&format!(
            "{{\"at\":\"{at}\",\"step\":0,\"event\":\"opened\",\"case\":\"{case}\",\"protocol\":\"story.delivery/1\",\"inputs\":[]}}\n"
        ));
        let completed = completed.map_or(String::new(), |completed| {
            format!(",\"completed\":{completed}")
        });
        text.push_str(&format!(
            "{{\"at\":\"{at}\",\"step\":1,\"event\":\"stop\",\"stop\":\"{stop}\",\"steps\":1,\"tokens\":0{completed}}}\n"
        ));
    }
    fs::write(dir.join("events.jsonl"), text).expect("the record");
}

/// The case `case`'s outcome as the scorecard of `repo` gives it.
fn outcome_of(repo: &Path, case: &str) -> String {
    takt::scorecard::Scorecard::read(repo)
        .expect("the scorecard reads")
        .cases()
        .into_iter()
        .find(|found| found.case == case)
        .expect("the case is scored")
        .outcome
}

/// The stop line says whether it is a completion, as the binary that stopped knows its own
/// stops; the scorecard reads that first. A stop this binary does not know, written with
/// `completed: false`, is no completion: the case's outcome is its last run's, not that stop;
/// one written with `completed: true` is a completion, and the case's outcome. A line without
/// the field reads by the list of known stops, as before.
#[test]
fn the_scorecard_scores_a_stop_by_the_completed_its_writer_recorded() {
    let repo = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("unperformable-{}-completed", std::process::id()));
    let _ = fs::remove_dir_all(&repo);
    let _scratch = Scratch(repo.clone());

    stopped_runs(
        &repo,
        "unknown-not-completed",
        &[("DiskFull (2 GiB left)", Some(false)), ("NoProgress", None)],
    );
    assert_eq!(outcome_of(&repo, "unknown-not-completed"), "NoProgress");

    stopped_runs(
        &repo,
        "unknown-completed",
        &[("Shipped (v2)", Some(true)), ("NoProgress", Some(false))],
    );
    assert_eq!(outcome_of(&repo, "unknown-completed"), "Shipped");

    stopped_runs(
        &repo,
        "older-lines",
        &[("Completed (accepted)", None), ("NoProgress", None)],
    );
    assert_eq!(outcome_of(&repo, "older-lines"), "accepted");
}

/// The run writes `completed` on its stop line: `false` for an `Unperformable` stop.
#[test]
fn the_stop_line_records_whether_the_stop_is_a_completion() {
    let tree = scratch("stop-line", false);
    let harness = Counting::default();
    let forge: Rc<dyn Forge> = Rc::new(Memory::default());
    let validators = validators(false, Some(forge));
    let mut run =
        Run::open(&tree, options("stop-line"), &harness, &validators).expect("the case opens");
    run.stop_when_unperformable();
    let mut out = Vec::new();
    run.run(&mut out).expect("the run stops without an error");
    let text = fs::read_to_string(tree.join(".engineering/cases/stop-line/events.jsonl"))
        .expect("the record");
    let stop = text
        .lines()
        .rfind(|line| line.contains("\"event\":\"stop\""))
        .expect("a stop line");
    assert!(stop.contains("\"completed\":false"), "{stop}");
}
