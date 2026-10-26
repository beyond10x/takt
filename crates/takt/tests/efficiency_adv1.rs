//! Adversary pass 1 on the efficiency unit (story:pr-opens-when-change-is-done,
//! story:no-model-step-for-approval, story:no-inspect-for-stateless-harness): the choice
//! recorder, the hand-over and `pr.open`, the approval stop and the stateless filter, each case
//! named for the defect it shows. Fake harnesses and the in-memory forge only.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

use takt::evidence::Record;
use takt::forge::{Checks, Forge, Memory};
use takt::harness::{Harness, StepContext, StepReport};
use takt::validator::{CiRead, Observation, PrOpen, Validator};
use takt::{Run, RunOptions, Stop};

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
        .join(format!("efficiency-adv1-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(&dir).expect("scratch dir");
    fs::write(dir.join("README.md"), "a story\n").expect("a file");
    fs::write(dir.join("run.sh"), "#!/bin/sh\n").expect("a file");
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["config", "core.fileMode", "true"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "start"]);
    Scratch(dir)
}

/// The records of `kind` under `case_dir`, each as its JSON.
fn records(case_dir: &Path, kind: &str) -> Vec<serde_json::Value> {
    let Ok(entries) = fs::read_dir(case_dir.join("evidence")) else {
        return Vec::new();
    };
    entries
        .map(|entry| {
            serde_json::from_str(
                &fs::read_to_string(entry.expect("an entry").path()).expect("a file"),
            )
            .expect("json")
        })
        .filter(|record: &serde_json::Value| record["kind"] == kind)
        .collect()
}

/// Always `pass` at every revision: tests and a gate that stay green.
struct Green {
    action: &'static str,
    kind: &'static str,
}

impl Validator for Green {
    fn name(&self) -> &str {
        self.action
    }

    fn action(&self) -> &str {
        self.action
    }

    fn kind(&self) -> &str {
        self.kind
    }

    fn subject(&self) -> &str {
        "implementation"
    }

    fn observe(&self, _: &Path, revision: &str, seq: usize) -> anyhow::Result<Observation> {
        Ok(Observation {
            summary: "pass".to_owned(),
            artifact: None,
            records: vec![Record::new(
                format!("{}-{seq}", self.kind),
                self.kind,
                Some("pass".to_owned()),
                self.subject(),
                revision,
                self.name(),
            )],
        })
    }
}

/// One move of [`Script`].
#[derive(Clone, Copy)]
enum Move {
    /// `implementation.edit`, appending a line to the file.
    Edit(&'static str),
    /// `change.handover`, changing nothing.
    HandOver,
    /// Any named action, changing nothing.
    Name(&'static str),
}

/// Plays its moves in order and keeps every offer it was given; once they are played it names
/// the merge when it is offered as an approval, else an edit that changes nothing.
struct Script {
    moves: RefCell<VecDeque<Move>>,
    offered: RefCell<Vec<Vec<String>>>,
    calls: Cell<usize>,
}

impl Script {
    fn new(moves: &[Move]) -> Self {
        Self {
            moves: RefCell::new(moves.iter().copied().collect()),
            offered: RefCell::new(Vec::new()),
            calls: Cell::new(0),
        }
    }
}

impl Harness for Script {
    fn name(&self) -> &str {
        "script"
    }

    fn step(&self, context: &StepContext<'_>) -> anyhow::Result<StepReport> {
        self.calls.set(self.calls.get() + 1);
        self.offered.borrow_mut().push(context.offered.to_vec());
        let next = self.moves.borrow_mut().pop_front();
        let action = match next {
            Some(Move::Edit(path)) => {
                let at = context.tree.join(path);
                let before = fs::read_to_string(&at).unwrap_or_default();
                fs::write(&at, format!("{before}{}\n", self.calls.get()))?;
                "implementation.edit"
            }
            Some(Move::HandOver) => "change.handover",
            Some(Move::Name(action)) => action,
            None if context.approvals.contains(&"repository.merge".to_owned()) => {
                "repository.merge"
            }
            None => "implementation.edit",
        };
        Ok(StepReport {
            action: action.to_owned(),
            summary: "scripted".to_owned(),
            tokens: Some(100),
        })
    }
}

/// A delivery run with green tests and gate, and `pr.open` and `ci.read` over `memory`: the
/// stop (or the error), what was printed, and a copy of the case directory.
fn delivered(
    name: &str,
    memory: &Rc<Memory>,
    harness: &dyn Harness,
) -> (Result<Stop, String>, String, PathBuf) {
    let tree = scratch(name);
    let forge: Rc<dyn Forge> = memory.clone();
    let validators: Vec<Box<dyn Validator>> = vec![
        Box::new(Green {
            action: "tests.run",
            kind: "test_result",
        }),
        Box::new(Green {
            action: "gate.run",
            kind: "gate_result",
        }),
        Box::new(PrOpen::new(forge.clone())),
        Box::new(CiRead::new(forge)),
    ];
    let options = RunOptions {
        protocol: PROTOCOL.to_owned(),
        intent: "remove a todo by its number".to_owned(),
        inputs: Vec::new(),
        iterations: Some(40),
        hours: None,
        tokens: None,
        case_id: Some(name.to_owned()),
        gate: Some("repository.merge".to_owned()),
    };
    let mut run = Run::open(&tree, options, harness, &validators).expect("the case opens");
    let mut out = Vec::new();
    let stop = run.run(&mut out).map_err(|error| format!("{error:#}"));
    let dir = run.case.dir.clone();
    drop(run);
    let kept = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "efficiency-adv1-{}-{name}-case",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&kept);
    copy(&dir, &kept);
    (stop, String::from_utf8(out).expect("utf-8"), kept)
}

fn copy(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("a directory");
    for entry in fs::read_dir(from).expect("a directory") {
        let entry = entry.expect("an entry");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("a type").is_dir() {
            copy(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).expect("a copy");
        }
    }
}

/// A run of the protocol document `yaml` with no validator and `harness`: the stop (or the
/// error), what was printed, and a copy of the case directory.
fn run_protocol(
    name: &str,
    yaml: &str,
    inputs: &[(&str, &str)],
    harness: &dyn Harness,
) -> (Result<Stop, String>, String, PathBuf) {
    let tree = scratch(name);
    let protocol = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "efficiency-adv1-{}-{name}.yaml",
        std::process::id()
    ));
    fs::write(&protocol, yaml).expect("a protocol");
    let validators: Vec<Box<dyn Validator>> = Vec::new();
    let options = RunOptions {
        protocol: protocol.display().to_string(),
        intent: "deliver".to_owned(),
        inputs: inputs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect(),
        iterations: Some(5),
        hours: None,
        tokens: None,
        case_id: Some(name.to_owned()),
        gate: None,
    };
    let mut run = Run::open(&tree, options, harness, &validators).expect("the case opens");
    let mut out = Vec::new();
    let stop = run.run(&mut out).map_err(|error| format!("{error:#}"));
    let dir = run.case.dir.clone();
    drop(run);
    let kept = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "efficiency-adv1-{}-{name}-case",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&kept);
    copy(&dir, &kept);
    let _ = fs::remove_file(&protocol);
    (stop, String::from_utf8(out).expect("utf-8"), kept)
}

/// The choice-kind rule (every producer `effect: read`, no predicate naming a `result`) is
/// met by an observation kind as much as by a choice. A protocol whose only claim about
/// continuous integration is that a `ci_result` exists (`ci.observed`, no `result`), with
/// `ci.read` declared `effect: read`, run with no forge validator: the harness names
/// `ci.read`, changes nothing, and takt writes a `ci_result` itself, by `takt.choice`, from the
/// model's choice; the case is accepted on it. Evidence of an observation from no validator.
#[test]
fn adv_an_observation_kind_read_without_a_result_is_written_by_takt_from_the_models_choice() {
    let yaml = "format: protocol/1\n\
        protocol:\n  id: ci.observed.only\n  revision: 1\n\
        artifacts:\n  implementation:\n    description: the code\n\
        evidence_kinds:\n  ci_result:\n    description: the result of continuous integration on a revision\n\
        claims:\n  ci.observed:\n    description: continuous integration reported on the revision\n    true_when:\n      evidence:\n        kind: ci_result\n        subject: implementation\n\
        actions:\n  implementation.edit:\n    description: change the code\n    effect: write\n\
        \x20 ci.read:\n    description: read the continuous integration result\n    effect: read\n    may_produce:\n      - evidence: ci_result\n\
        outcomes:\n  accepted:\n    description: continuous integration reported\n    requires:\n      claim: ci.observed\n";
    let harness = Script::new(&[Move::Name("ci.read")]);
    let (stop, text, case_dir) = run_protocol("ci-choice", yaml, &[], &harness);
    let by_takt: Vec<serde_json::Value> = records(&case_dir, "ci_result")
        .into_iter()
        .filter(|record| record["producer"] == takt::run::CHOICE_PRODUCER)
        .collect();
    assert!(
        by_takt.is_empty(),
        "takt wrote a ci_result from the harness's choice, no forge read: {by_takt:?}\nstopped {stop:?}\n{text}"
    );
    assert_ne!(
        stop,
        Ok(Stop::Completed("accepted".to_owned())),
        "accepted on a ci_result no validator wrote: {text}"
    );
}

/// The acceptance of story:pr-opens-when-change-is-done: "no `pr.open` before the first
/// implementation commit". The protocol offers `change.handover` at the seed (its only
/// precondition is that the claim is unknown), so a harness that hands over first gets a
/// `pr.open` step at the seed revision, before anything was committed.
#[test]
fn adv_a_hand_over_at_the_seed_takes_pr_open_before_the_first_implementation_commit() {
    let memory = Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    });
    let harness = Script::new(&[Move::HandOver, Move::Edit("work.txt"), Move::HandOver]);
    let (stop, text, _) = delivered("seed-hand-over", &memory, &harness);
    let first_commit = text.find("effect: committed").unwrap_or(usize::MAX);
    let first_pr_open = text.find(": pr.open {}").unwrap_or(usize::MAX);
    assert!(
        first_pr_open > first_commit,
        "pr.open taken at the seed, before the first implementation commit (stopped {stop:?}):\n{text}"
    );
}

/// A protocol whose choice kind is read about no artifact (`evidence: {kind: ack}`, no
/// `subject`, which `protocol/1` allows): the action is offered to a stateless harness as one
/// that records a choice, the harness takes it, and only then does takt find it cannot bind the
/// record: the run ends on an error after the model's tokens were spent.
#[test]
fn adv_a_choice_kind_read_about_no_artifact_is_offered_and_then_ends_the_run_on_an_error() {
    let yaml = "format: protocol/1\n\
        protocol:\n  id: ack.only\n  revision: 1\n\
        artifacts:\n  implementation:\n    description: the code\n\
        evidence_kinds:\n  ack:\n    description: the harness acknowledged\n\
        claims:\n  acked:\n    description: acknowledged\n    true_when:\n      evidence:\n        kind: ack\n\
        actions:\n  implementation.edit:\n    description: change the code\n    effect: write\n\
        \x20 ack.give:\n    description: acknowledge\n    effect: read\n    may_produce:\n      - evidence: ack\n\
        outcomes:\n  accepted:\n    description: acknowledged\n    requires:\n      claim: acked\n";
    let harness = Script::new(&[Move::Name("ack.give")]);
    // Since correction 2 such a protocol is refused when the case would open, before any step
    // and before anything is written: `ack.give` is then never offered.
    let tree = scratch("ack-no-subject-open");
    let protocol = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "efficiency-adv1-{}-ack-open.yaml",
        std::process::id()
    ));
    fs::write(&protocol, yaml).expect("a protocol");
    let options = RunOptions {
        protocol: protocol.display().to_string(),
        intent: "deliver".to_owned(),
        inputs: Vec::new(),
        iterations: Some(5),
        hours: None,
        tokens: None,
        case_id: Some("ack-open".to_owned()),
        gate: None,
    };
    let validators: Vec<Box<dyn Validator>> = Vec::new();
    let opened = Run::open(&tree, options, &harness, &validators).err();
    let _ = fs::remove_file(&protocol);
    let error = format!("{:#}", opened.expect("the protocol is refused at open"));
    assert!(error.contains("ack"), "{error}");
    assert!(harness.offered.borrow().is_empty());
}

/// story:no-model-step-for-approval's acceptance, on the shape the self-improvement protocol
/// has (`candidate.merge` / `candidate.merge_judge_surface`): the one open claim is decided by
/// a merge that two actions may produce, each needing its own capability, one of them blocked
/// for good by the case's input. Only the grant of `merge.normal` stands between the case and
/// its outcome, yet the rule asks that every producer be waiting for a grant, finds the blocked
/// one is not, and calls the harness.
#[test]
fn adv_a_claim_whose_other_producer_is_blocked_by_an_input_calls_the_harness_at_the_gate() {
    let yaml = "format: protocol/1\n\
        protocol:\n  id: two.merges\n  revision: 1\n\
        case:\n  inputs:\n    surface:\n      description: what the change touches\n      values: [normal, judge]\n\
        artifacts:\n  implementation:\n    description: the code\n\
        evidence_kinds:\n  merge_observation:\n    description: the merge\n\
        claims:\n  merged:\n    description: merged\n    true_when:\n      evidence:\n        kind: merge_observation\n        result: merged\n        subject: implementation\n\
        actions:\n  implementation.edit:\n    description: change the code\n    effect: write\n\
        \x20 merge.normal:\n    description: merge a normal change\n    effect: write\n    precondition:\n      input: surface\n      in: [normal]\n    requires:\n      - capability: merge.normal\n    may_produce:\n      - evidence: merge_observation\n\
        \x20 merge.judge:\n    description: merge a judge change\n    effect: write\n    precondition:\n      input: surface\n      in: [judge]\n    requires:\n      - capability: merge.judge\n    may_produce:\n      - evidence: merge_observation\n\
        outcomes:\n  accepted:\n    description: merged\n    requires:\n      claim: merged\n";
    let harness = Script::new(&[]);
    let (stop, text, _) = run_protocol("two-merges", yaml, &[("surface", "normal")], &harness);
    assert!(
        text.contains("merge.normal (approval-required)") && text.contains("merge.judge (blocked)"),
        "the frontier is not the gate this case builds: {text}"
    );
    assert_eq!(
        harness.calls.get(),
        0,
        "the harness was called with only a grant left (stopped {stop:?}): {text}"
    );
    assert_eq!(
        stop,
        Ok(Stop::ApprovalRequired(vec!["merge.normal".to_owned()])),
        "{text}"
    );
}

/// `takt.work.ChoiceEffect`: "a step whose choice records the case already holds asks for
/// none". A choice action with no precondition, taken twice at one revision: the first step's
/// effect is the choice effect's, answering the record it wrote; the second's case already
/// holds that record, so the choice effect is asked for nothing, no record is written again,
/// and the run sees no progress.
#[test]
fn a_choice_taken_again_with_its_record_held_asks_the_choice_effect_for_none() {
    let yaml = "format: protocol/1\n\
        protocol:\n  id: ack.twice\n  revision: 1\n\
        artifacts:\n  implementation:\n    description: the code\n\
        evidence_kinds:\n  ack:\n    description: the harness acknowledged\n  review:\n    description: a review of the code\n\
        claims:\n  acked:\n    description: acknowledged\n    true_when:\n      evidence:\n        kind: ack\n        subject: implementation\n\
        \x20 reviewed:\n    description: reviewed\n    true_when:\n      evidence:\n        kind: review\n        result: ok\n        subject: implementation\n\
        actions:\n  implementation.edit:\n    description: change the code\n    effect: write\n\
        \x20 ack.give:\n    description: acknowledge\n    effect: read\n    may_produce:\n      - evidence: ack\n\
        \x20 review.write:\n    description: review the code\n    effect: write\n    may_produce:\n      - evidence: review\n\
        outcomes:\n  accepted:\n    description: acknowledged and reviewed\n    requires:\n      all:\n        - claim: acked\n        - claim: reviewed\n";
    let harness = Script::new(&[Move::Name("ack.give"), Move::Name("ack.give")]);
    let (stop, text, case_dir) = run_protocol("ack-twice", yaml, &[], &harness);
    assert_eq!(stop, Ok(Stop::NoProgress), "{text}");
    let acks = records(&case_dir, "ack");
    assert_eq!(acks.len(), 1, "one ack record: {acks:?}\n{text}");
    assert_eq!(acks[0]["producer"], takt::run::CHOICE_PRODUCER, "{text}");
    let observations: Vec<serde_json::Value> = serde_json::from_slice(
        &fs::read(case_dir.join("observations.json")).expect("observations"),
    )
    .expect("observations parse");
    let effects: Vec<String> = observations
        .iter()
        .filter(|observation| observation["source"] == "effect")
        .map(|observation| observation["payload"].to_string())
        .collect();
    assert_eq!(
        effects.len(),
        2,
        "two effect observations: {effects:?}\n{text}"
    );
    assert!(
        effects[0].contains("\"records\"") && effects[0].contains("ack-"),
        "the first step's effect is the choice effect's, naming its record: {effects:?}"
    );
    assert!(
        !effects[1].contains("\"records\""),
        "the second step asked the choice effect: {effects:?}"
    );
}
