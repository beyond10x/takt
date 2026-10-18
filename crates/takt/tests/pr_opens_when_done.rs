//! story:pr-opens-when-change-is-done: a story run opens its pull request once the change is
//! complete, so continuous integration runs once per accepted unit, and no `pr.open` is taken at
//! the seed, before any change exists. A harness that edits the implementation, then writes the
//! tests, is the shape of scenario 1: on the base the run pushed and read CI after each edit.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

use takt::case::CASES_DIR;
use takt::evidence::Record;
use takt::forge::{Checks, Forge, Memory};
use takt::harness::{Harness, StepContext, StepReport};
use takt::validator::{ChangeRead, CiRead, Observation, PrOpen, Validator};
use takt::{Run, RunOptions, Stop};

const PROTOCOL: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../protocols/story-delivery/1.yaml"
);

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
        .join(format!("pr-opens-when-done-{}-{name}", std::process::id()));
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

/// A validator that observes `pass` for its action at every revision: the tests and the gate of
/// a repository whose work keeps them green.
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

/// Scenario 1's developer: edits the implementation, then writes the story's tests, one step
/// each; then requests the merge when it is offered, and otherwise hands the finished change
/// over by taking `change.handover`, changing nothing.
struct Developer {
    edits: RefCell<VecDeque<(&'static str, &'static str)>>,
    calls: Cell<usize>,
}

impl Developer {
    fn new() -> Self {
        Self {
            edits: RefCell::new(VecDeque::from([
                ("implementation.edit", "src/remove.txt"),
                ("tests.write", "tests/remove_test.txt"),
            ])),
            calls: Cell::new(0),
        }
    }
}

impl Harness for Developer {
    fn name(&self) -> &str {
        "developer"
    }

    fn step(&self, context: &StepContext<'_>) -> anyhow::Result<StepReport> {
        self.calls.set(self.calls.get() + 1);
        let next = self.edits.borrow_mut().pop_front();
        if let Some((action, path)) = next {
            let at = context.tree.join(path);
            fs::create_dir_all(at.parent().expect("a parent")).expect("a directory");
            fs::write(&at, format!("{action}\n")).expect("a file");
            return Ok(StepReport {
                action: action.to_owned(),
                summary: format!("wrote {path}"),
                tokens: Some(100),
            });
        }
        let action = if context.approvals.contains(&"repository.merge".to_owned()) {
            "repository.merge"
        } else {
            "change.handover"
        };
        Ok(StepReport {
            action: action.to_owned(),
            summary: "the change is complete".to_owned(),
            tokens: Some(100),
        })
    }
}

/// The records of `kind` under the case, each as its JSON.
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

/// The validator steps of `action` the run printed.
fn validator_steps(text: &str, action: &str) -> usize {
    text.lines()
        .filter(|line| line.starts_with("step ") && line.ends_with(&format!(": {action} {{}}")))
        .count()
}

/// Scenario 1 on the fake forge: the developer edits the implementation, then writes the tests.
/// The story's Expected before, on the base: `pr.open` three times (at the seed, after the
/// implementation, after the tests) and `ci_result pass` at two revisions before
/// `ApprovalRequired`. The acceptance: exactly one CI result, at the revision the grant is
/// given at, and no `pr.open` before the first implementation commit.
#[test]
fn the_pull_request_opens_once_the_change_is_complete_and_ci_runs_once() {
    let tree = scratch("scenario-1");
    let memory = Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    });
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
        Box::new(ChangeRead),
    ];
    let options = RunOptions {
        protocol: PROTOCOL.to_owned(),
        intent: "remove a todo by its number".to_owned(),
        inputs: Vec::new(),
        iterations: Some(40),
        hours: None,
        tokens: None,
        case_id: Some("scenario-1".to_owned()),
        gate: Some("repository.merge".to_owned()),
    };
    let harness = Developer::new();
    let mut run = Run::open(&tree, options, &harness, &validators).expect("the case opens");
    let mut out = Vec::new();
    let stop = run.run(&mut out).map_err(|error| format!("{error:#}"));
    let text = String::from_utf8(out).expect("utf-8");

    assert_eq!(
        stop,
        Ok(Stop::ApprovalRequired(vec!["repository.merge".to_owned()])),
        "{text}"
    );
    assert_eq!(
        validator_steps(&text, "pr.open"),
        1,
        "pr.open taken more than once: {text}"
    );
    let first_commit = text
        .find("effect: committed")
        .unwrap_or_else(|| panic!("no implementation commit: {text}"));
    let first_pr_open = text.find(": pr.open {}").unwrap_or(usize::MAX);
    assert!(
        first_pr_open > first_commit,
        "pr.open taken before the first implementation commit: {text}"
    );
    let case_dir = tree.join(CASES_DIR).join("scenario-1");
    let ci = records(&case_dir, "ci_result");
    let revision = run
        .case
        .revision("implementation")
        .expect("implementation has a revision");
    assert_eq!(ci.len(), 1, "CI results: {ci:?}\n{text}");
    assert_eq!(ci[0]["result"], "pass", "{text}");
    assert_eq!(
        ci[0]["subject_revision"],
        revision.as_str(),
        "the one CI result is not at the revision the grant is given at: {text}"
    );
    let calls = memory.calls.borrow().clone();
    assert_eq!(
        calls.iter().filter(|c| c.starts_with("open ")).count(),
        1,
        "{calls:?}"
    );
    assert_eq!(
        calls.iter().filter(|c| c.starts_with("push ")).count(),
        1,
        "one push for the one complete change: {calls:?}"
    );
    assert_eq!(
        calls.iter().filter(|c| c.starts_with("checks ")).count(),
        1,
        "{calls:?}"
    );
    eprintln!(
        "scenario 1 on the fake forge: steps {}, harness calls {}, pr.open {}, ci.read {}, CI results {}",
        run.steps,
        harness.calls.get(),
        validator_steps(&text, "pr.open"),
        validator_steps(&text, "ci.read"),
        ci.len()
    );
}

/// One step of a [`Scripted`] harness: an edit of a file, a hand-over (`change.handover`), or a
/// hand-over that also writes a file.
#[derive(Clone, Copy)]
enum Move {
    Edit(&'static str),
    HandOver,
    HandOverEditing(&'static str),
}

/// Plays its moves in order, keeping whether each step was offered `change.handover`; once
/// they are played it reports an edit that changes nothing.
struct Scripted {
    moves: RefCell<VecDeque<Move>>,
    handover_offered: RefCell<Vec<bool>>,
}

impl Scripted {
    fn new(moves: &[Move]) -> Self {
        Self {
            moves: RefCell::new(moves.iter().copied().collect()),
            handover_offered: RefCell::new(Vec::new()),
        }
    }
}

impl Harness for Scripted {
    fn name(&self) -> &str {
        "scripted"
    }

    fn step(&self, context: &StepContext<'_>) -> anyhow::Result<StepReport> {
        self.handover_offered
            .borrow_mut()
            .push(context.offered.contains(&"change.handover".to_owned()));
        let next = self.moves.borrow_mut().pop_front();
        let (action, path) = match next {
            Some(Move::Edit(path)) => ("implementation.edit", Some(path)),
            Some(Move::HandOver) => ("change.handover", None),
            Some(Move::HandOverEditing(path)) => ("change.handover", Some(path)),
            None => ("implementation.edit", None),
        };
        if let Some(path) = path {
            let at = context.tree.join(path);
            let before = fs::read_to_string(&at).unwrap_or_default();
            fs::write(
                &at,
                format!("{before}{}\n", self.handover_offered.borrow().len()),
            )?;
        }
        Ok(StepReport {
            action: action.to_owned(),
            summary: "scripted".to_owned(),
            tokens: None,
        })
    }
}

/// A delivery run with green tests and gate and `pr.open` and `ci.read` over `memory`: the
/// stop, what was printed, and the case directory.
fn delivered(name: &str, memory: &Rc<Memory>, harness: &dyn Harness) -> (Stop, String, PathBuf) {
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
        Box::new(ChangeRead),
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
    let stop = run.run(&mut out).expect("the run stops without an error");
    let dir = run.case.dir.clone();
    drop(run);
    // The scratch tree goes when this returns; what the cases read is copied out first.
    let kept = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "pr-opens-when-done-{}-{name}-case",
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

/// At the seed the harness that edits first takes no `pr.open`: the protocol blocks it until
/// the change is handed over, and nothing reaches the forge.
#[test]
fn at_the_seed_pr_open_waits_for_the_hand_over_and_nothing_reaches_the_forge() {
    let memory = Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    });
    let harness = Scripted::new(&[]);
    let (stop, text, _) = delivered("seed", &memory, &harness);
    assert_eq!(stop, Stop::NoProgress, "{text}");
    assert!(text.contains("pr.open (blocked)"), "{text}");
    assert_eq!(validator_steps(&text, "pr.open"), 0, "{text}");
    assert_eq!(validator_steps(&text, "ci.read"), 0, "{text}");
    assert!(
        memory.calls.borrow().is_empty(),
        "{:?}",
        memory.calls.borrow()
    );
}

/// The hand-over is a step that changes nothing: takt records it as one `handover` record with
/// no result, by its own producer, at the revision, and `change.handed_over` then holds.
#[test]
fn a_hand_over_is_recorded_with_no_result_at_the_revision() {
    let memory = Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    });
    let harness = Scripted::new(&[Move::Edit("work.txt"), Move::HandOver]);
    let (stop, text, case_dir) = delivered("recorded", &memory, &harness);
    assert_eq!(
        stop,
        Stop::ApprovalRequired(vec!["repository.merge".to_owned()]),
        "{text}"
    );
    let handovers = records(&case_dir, "handover");
    assert_eq!(handovers.len(), 1, "{handovers:?}\n{text}");
    assert_eq!(handovers[0]["result"], serde_json::Value::Null, "{text}");
    assert_eq!(
        handovers[0]["producer"],
        takt::run::CHOICE_PRODUCER,
        "{text}"
    );
    assert_eq!(handovers[0]["subject"], "implementation", "{text}");
    let ci = records(&case_dir, "ci_result");
    assert_eq!(
        handovers[0]["subject_revision"], ci[0]["subject_revision"],
        "{text}"
    );
}

/// A hand-over that also writes a file is refused, the file is discarded, nothing is recorded
/// and nothing is pushed.
#[test]
fn a_hand_over_with_changes_is_refused_and_nothing_is_pushed() {
    let memory = Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    });
    let harness = Scripted::new(&[Move::Edit("work.txt"), Move::HandOverEditing("more.txt")]);
    let (stop, text, case_dir) = delivered("hand-over-editing", &memory, &harness);
    assert!(
        matches!(&stop, Stop::StepRefused(why) if why == "change.handover records a choice and changed 1 path(s)"),
        "{stop:?}\n{text}"
    );
    assert!(records(&case_dir, "handover").is_empty(), "{text}");
    assert_eq!(validator_steps(&text, "pr.open"), 0, "{text}");
    assert!(
        memory.calls.borrow().is_empty(),
        "{:?}",
        memory.calls.borrow()
    );
}

/// Continuous integration fails; the harness edits again, which returns `change.handed_over`
/// to unknown, and the new revision is pushed only once the harness hands it over again: one
/// push per hand-over, one pull request. Where the hand-over is recorded it is not offered.
#[test]
fn after_a_failed_ci_the_next_change_is_pushed_once_it_is_handed_over_again() {
    let memory = Rc::new(Memory {
        checks: Checks::Fail,
        ..Memory::default()
    });
    let harness = Scripted::new(&[
        Move::Edit("work.txt"),
        Move::HandOver,
        Move::Edit("work.txt"),
        Move::Edit("work.txt"),
        Move::HandOver,
    ]);
    let (stop, text, case_dir) = delivered("ci-failed", &memory, &harness);
    assert_eq!(stop, Stop::NoProgress, "{text}");
    let calls = memory.calls.borrow().clone();
    assert_eq!(
        calls.iter().filter(|c| c.starts_with("push ")).count(),
        2,
        "{calls:?}\n{text}"
    );
    assert_eq!(
        calls.iter().filter(|c| c.starts_with("open ")).count(),
        1,
        "{calls:?}"
    );
    assert_eq!(validator_steps(&text, "pr.open"), 2, "{text}");
    assert_eq!(validator_steps(&text, "ci.read"), 2, "{text}");
    assert_eq!(records(&case_dir, "handover").len(), 2, "{text}");
    assert_eq!(
        *harness.handover_offered.borrow(),
        [true, true, false, true, true, false],
        "{text}"
    );
}

/// The list the choice rule reads (`takt::validator::BUILT_IN_ACTIONS`) names exactly the actions
/// takt's built-in validators perform: a validator added without its action there would let its
/// evidence kind pass for a choice the step choice records.
#[test]
fn the_built_in_actions_are_the_actions_of_the_built_in_validators() {
    let forge: Rc<dyn Forge> = Rc::new(Memory::default());
    let built_in: Vec<Box<dyn Validator>> = vec![
        Box::new(takt::validator::TestsRun::new("true").expect("a command")),
        Box::new(takt::validator::GateRun::new(".engineering/gates.yaml")),
        Box::new(PrOpen::new(forge.clone())),
        Box::new(CiRead::new(forge.clone())),
        Box::new(takt::validator::PrMerged::new(forge)),
        Box::new(ChangeRead),
    ];
    let mut actions: Vec<&str> = built_in.iter().map(|v| v.action()).collect();
    actions.sort_unstable();
    let mut listed = takt::validator::BUILT_IN_ACTIONS.to_vec();
    listed.sort_unstable();
    assert_eq!(actions, listed);
}
