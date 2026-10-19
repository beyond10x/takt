//! story:story-delivery-tests-first: on `story.delivery/2` a story case writes its tests before
//! its implementation and opens its pull request once, on the complete change. The order is the
//! protocol's, held by preconditions: `implementation.edit` needs `tests.red`, a `test_result`
//! `fail` at a revision whose `change_record` is `tests` (the change since the base is to test
//! paths only), and `pr.open` needs `change.complete` (a test path and an implementation path
//! both changed) with the tests and the gate green at the same revision. A harness that edits the
//! implementation first is refused by the frontier, which never offered the action.

use std::cell::RefCell;
use std::collections::{BTreeSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

use takt::case::CASES_DIR;
use takt::evidence::Record;
use takt::forge::{Checks, Forge, Memory};
use takt::harness::{Harness, StepContext, StepReport};
use takt::validator::{ChangeRead, ChangeVocabulary, CiRead, Observation, PrOpen, Validator};
use takt::{CaseDir, Decision, LoadedProtocol, Run, RunOptions, Stop};

/// The protocol under test, and the revision it leaves as it is.
const PROTOCOL: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../protocols/story-delivery/2.yaml"
);
const PROTOCOL_ONE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../protocols/story-delivery/1.yaml"
);

/// The story's test path and implementation path: one under `tests/`, one not.
const TEST_PATH: &str = "tests/remove_test.txt";
const IMPLEMENTATION_PATH: &str = "src/remove.txt";

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

/// A scratch repository at its seed: one commit, no tests of the story, no implementation.
fn scratch(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("tests-first-{}-{name}", std::process::id()));
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

/// A record of `kind` about `subject` at `revision`; an empty `result` writes none (a
/// hand-over has no result).
fn record_of(
    kind: &str,
    result: &str,
    subject: &str,
    revision: &str,
    producer: &str,
    seq: usize,
) -> Record {
    Record::new(
        format!("{kind}-{seq}"),
        kind,
        (!result.is_empty()).then(|| result.to_owned()),
        subject,
        revision,
        producer,
    )
}

/// The gate of a repository whose work keeps it green: `pass` at every revision.
struct Green;

impl Validator for Green {
    fn name(&self) -> &str {
        "gate"
    }

    fn action(&self) -> &str {
        "gate.run"
    }

    fn kind(&self) -> &str {
        "gate_result"
    }

    fn subject(&self) -> &str {
        "implementation"
    }

    fn observe(&self, _: &Path, revision: &str, seq: usize) -> anyhow::Result<Observation> {
        Ok(Observation {
            summary: "pass".to_owned(),
            artifact: None,
            records: vec![record_of(
                self.kind(),
                "pass",
                self.subject(),
                revision,
                self.name(),
                seq,
            )],
        })
    }
}

/// The story's tests, as a test command runs them from the tree at the revision: `fail` while
/// the story's test file is there and the implementation it tests is not done, `pass` otherwise
/// (the seed's own tests pass, and so does a done implementation).
struct Tests;

impl Validator for Tests {
    fn name(&self) -> &str {
        "tests"
    }

    fn action(&self) -> &str {
        "tests.run"
    }

    fn kind(&self) -> &str {
        "test_result"
    }

    fn subject(&self) -> &str {
        "implementation"
    }

    fn observe(&self, tree: &Path, revision: &str, seq: usize) -> anyhow::Result<Observation> {
        let tested = tree.join(TEST_PATH).exists();
        let done = fs::read_to_string(tree.join(IMPLEMENTATION_PATH))
            .is_ok_and(|text| text.contains("done"));
        let result = if tested && !done { "fail" } else { "pass" };
        Ok(Observation {
            summary: result.to_owned(),
            artifact: None,
            records: vec![record_of(
                self.kind(),
                result,
                self.subject(),
                revision,
                self.name(),
                seq,
            )],
        })
    }
}

/// One step of the scripted developer.
#[derive(Clone, Copy)]
enum Move {
    /// Writes the story's test file and reports `tests.write`.
    TestsWrite,
    /// Writes the implementation file with this content and reports `implementation.edit`.
    Implement(&'static str),
    /// Reports `change.handover`, changing nothing.
    HandOver,
}

/// Plays its moves in order and keeps what each step was offered; asked while the merge needs
/// its approval it requests the merge; out of moves it hands over when that is offered, else
/// reports a `tests.write` that changed nothing.
struct Developer {
    moves: RefCell<VecDeque<Move>>,
    offers: RefCell<Vec<Vec<String>>>,
}

impl Developer {
    fn new(moves: &[Move]) -> Self {
        Self {
            moves: RefCell::new(moves.iter().copied().collect()),
            offers: RefCell::new(Vec::new()),
        }
    }
}

impl Harness for Developer {
    fn name(&self) -> &str {
        "developer"
    }

    fn step(&self, context: &StepContext<'_>) -> anyhow::Result<StepReport> {
        self.offers.borrow_mut().push(context.offered.to_vec());
        if context.approvals.contains(&"repository.merge".to_owned()) {
            return Ok(StepReport {
                action: "repository.merge".to_owned(),
                summary: "the change is complete".to_owned(),
                tokens: Some(100),
            });
        }
        let next = self.moves.borrow_mut().pop_front();
        let (action, written) = match next {
            Some(Move::TestsWrite) => ("tests.write", Some((TEST_PATH, "the test\n"))),
            Some(Move::Implement(content)) => {
                ("implementation.edit", Some((IMPLEMENTATION_PATH, content)))
            }
            Some(Move::HandOver) => ("change.handover", None),
            None if context.offered.contains(&"change.handover".to_owned()) => {
                ("change.handover", None)
            }
            None => ("tests.write", None),
        };
        if let Some((path, content)) = written {
            let at = context.tree.join(path);
            fs::create_dir_all(at.parent().expect("a parent"))?;
            fs::write(&at, content)?;
        }
        Ok(StepReport {
            action: action.to_owned(),
            summary: format!("played {action}"),
            tokens: Some(100),
        })
    }
}

/// A harness never asked: for cases opened and read without a run.
struct Idle;

impl Harness for Idle {
    fn name(&self) -> &str {
        "idle"
    }

    fn step(&self, _: &StepContext<'_>) -> anyhow::Result<StepReport> {
        Ok(StepReport {
            action: "tests.write".to_owned(),
            summary: "idle".to_owned(),
            tokens: None,
        })
    }
}

/// The records of `kind` under the case, in the order they were written (by the number their
/// ids end in), each as its JSON.
fn records(case_dir: &Path, kind: &str) -> Vec<serde_json::Value> {
    let Ok(entries) = fs::read_dir(case_dir.join("evidence")) else {
        return Vec::new();
    };
    let mut found: Vec<serde_json::Value> = entries
        .map(|entry| {
            serde_json::from_str(
                &fs::read_to_string(entry.expect("an entry").path()).expect("a file"),
            )
            .expect("json")
        })
        .filter(|record: &serde_json::Value| record["kind"] == kind)
        .collect();
    let number = |record: &serde_json::Value| -> usize {
        record["id"]
            .as_str()
            .and_then(|id| id.rsplit('-').next())
            .and_then(|n| n.parse().ok())
            .expect("a numbered id")
    };
    found.sort_by_key(number);
    found
}

/// Every decision the run wrote, in order, parsed and raw.
fn decisions(case_dir: &Path) -> Vec<(Decision, serde_json::Value)> {
    let mut names: Vec<PathBuf> = fs::read_dir(case_dir.join("decisions"))
        .expect("decisions")
        .map(|entry| entry.expect("an entry").path())
        .collect();
    names.sort();
    names
        .iter()
        .map(|path| {
            let text = fs::read_to_string(path).expect("a decision");
            (
                Decision::from_json(&text).expect("a decision document"),
                serde_json::from_str(&text).expect("json"),
            )
        })
        .collect()
}

/// Whether a blocked action's reasons in the raw decision name `claim`.
fn names_claim(raw: &serde_json::Value, action: &str, claim: &str) -> bool {
    raw["actions"][action]["reasons"]
        .as_array()
        .is_some_and(|reasons| reasons.iter().any(|reason| reason["claim"] == claim))
}

/// The validator steps of `action` the run printed.
fn validator_steps(text: &str, action: &str) -> usize {
    text.lines()
        .filter(|line| line.starts_with("step ") && line.ends_with(&format!(": {action} {{}}")))
        .count()
}

/// A delivery run on `story.delivery/2` with the story's tests, a green gate, `pr.open` and
/// `ci.read` over `memory` and `change.read`: the stop, what was printed, a copy of the case
/// directory and the implementation revision the run ended at.
fn delivered(
    name: &str,
    memory: &Rc<Memory>,
    harness: &dyn Harness,
) -> (Stop, String, PathBuf, String) {
    let tree = scratch(name);
    let forge: Rc<dyn Forge> = memory.clone();
    let validators: Vec<Box<dyn Validator>> = vec![
        Box::new(Tests),
        Box::new(Green),
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
    let revision = run
        .case
        .revision("implementation")
        .expect("implementation has a revision");
    let dir = run.case.dir.clone();
    drop(run);
    // The scratch tree goes when this returns; what the assertions read is copied out first.
    let kept = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("tests-first-{}-{name}-case", std::process::id()));
    let _ = fs::remove_dir_all(&kept);
    copy(&dir, &kept);
    (stop, String::from_utf8(out).expect("utf-8"), kept, revision)
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

/// Scenario 1 with a developer that plays the order the protocol asks for: `tests.write`, then
/// `implementation.edit`, then the hand-over. The `tests.run` between the two edits records
/// `fail` at the tests-only revision, the implementation step is admitted by `tests.red` and
/// blocked by it before, the pull request opens once and continuous integration runs once.
#[test]
fn a_story_case_writes_its_tests_first_and_edits_the_implementation_under_a_red_test() {
    let memory = Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    });
    let harness = Developer::new(&[Move::TestsWrite, Move::Implement("done\n"), Move::HandOver]);
    let (stop, text, case_dir, revision) = delivered("in-order", &memory, &harness);
    assert_eq!(
        stop,
        Stop::ApprovalRequired(vec!["repository.merge".to_owned()]),
        "{text}"
    );

    // The order, as the run printed it.
    let tests_write = text
        .find(": tests.write (developer)")
        .unwrap_or_else(|| panic!("no tests.write step: {text}"));
    let implementation_edit = text
        .find(": implementation.edit (developer)")
        .unwrap_or_else(|| panic!("no implementation.edit step: {text}"));
    assert!(
        tests_write < implementation_edit,
        "the implementation was edited before the tests were written: {text}"
    );
    // The frontier offered tests.write and not implementation.edit at the first step, and
    // implementation.edit at the second.
    let offers = harness.offers.borrow();
    assert!(
        offers[0].contains(&"tests.write".to_owned())
            && !offers[0].contains(&"implementation.edit".to_owned()),
        "{offers:?}\n{text}"
    );
    assert!(
        offers[1].contains(&"implementation.edit".to_owned()),
        "{offers:?}\n{text}"
    );

    // The test run between them failed, at the revision whose change record is `tests`.
    let fails: Vec<serde_json::Value> = records(&case_dir, "test_result")
        .into_iter()
        .filter(|record| record["result"] == "fail")
        .collect();
    assert_eq!(fails.len(), 1, "{fails:?}\n{text}");
    let tests_only: Vec<serde_json::Value> = records(&case_dir, "change_record")
        .into_iter()
        .filter(|record| record["result"] == "tests")
        .collect();
    assert_eq!(tests_only.len(), 1, "{tests_only:?}\n{text}");
    assert_eq!(
        fails[0]["subject_revision"], tests_only[0]["subject_revision"],
        "the failing run and the tests-only change record are not at one revision: {text}"
    );
    assert_ne!(
        fails[0]["subject_revision"],
        serde_json::Value::String(revision.clone()),
        "the failing run is at the final revision: {text}"
    );

    // The implementation step's decision cites that failure: the first decision that admits
    // implementation.edit holds tests.red, and every decision before it blocks the action
    // naming tests.red.
    let decisions = decisions(&case_dir);
    let first = decisions
        .iter()
        .position(|(decision, _)| {
            decision
                .actions
                .get("implementation.edit")
                .map(String::as_str)
                == Some("admissible")
        })
        .unwrap_or_else(|| panic!("implementation.edit was never admissible: {text}"));
    assert_eq!(
        decisions[first]
            .0
            .claims
            .get("tests.red")
            .map(String::as_str),
        Some("true"),
        "{}",
        decisions[first].0.json
    );
    assert!(first > 0, "{text}");
    for (decision, raw) in &decisions[..first] {
        assert_eq!(
            decision
                .actions
                .get("implementation.edit")
                .map(String::as_str),
            Some("blocked"),
            "{}",
            decision.json
        );
        assert!(
            names_claim(raw, "implementation.edit", "tests.red"),
            "implementation.edit is blocked without naming tests.red: {}",
            decision.json
        );
    }

    // One push, one pull request, one CI read, for the final revision.
    let calls = memory.calls.borrow().clone();
    assert_eq!(
        calls.iter().filter(|c| c.starts_with("push ")).count(),
        1,
        "{calls:?}\n{text}"
    );
    assert_eq!(
        calls.iter().filter(|c| c.starts_with("open ")).count(),
        1,
        "{calls:?}"
    );
    assert_eq!(
        calls.iter().filter(|c| c.starts_with("checks ")).count(),
        1,
        "{calls:?}"
    );
    assert_eq!(validator_steps(&text, "pr.open"), 1, "{text}");
    let pull_requests = records(&case_dir, "pull_request");
    assert_eq!(pull_requests.len(), 1, "{text}");
    assert_eq!(
        pull_requests[0]["subject_revision"],
        revision.as_str(),
        "{text}"
    );
    assert_eq!(records(&case_dir, "ci_result").len(), 1, "{text}");
}

/// `pr.open` is not offered while only the tests changed, nor while both changed but the tests
/// are still red, whatever the harness hands over; it is taken once both path kinds changed and
/// the tests and the gate are green at that one revision, and the forge sees one push.
#[test]
fn pr_open_waits_for_both_path_kinds_green_at_one_revision_and_pushes_once() {
    let memory = Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    });
    let harness = Developer::new(&[
        Move::TestsWrite,
        Move::HandOver,
        Move::Implement("wip\n"),
        Move::Implement("done\n"),
        Move::HandOver,
    ]);
    let (stop, text, case_dir, revision) = delivered("waits", &memory, &harness);
    assert_eq!(
        stop,
        Stop::ApprovalRequired(vec!["repository.merge".to_owned()]),
        "{text}"
    );
    let shapes: Vec<String> = records(&case_dir, "change_record")
        .iter()
        .map(|record| record["result"].as_str().unwrap_or("-").to_owned())
        .collect();
    assert_eq!(shapes, ["none", "tests", "complete", "complete"], "{text}");
    assert_eq!(records(&case_dir, "handover").len(), 2, "{text}");

    let decisions = decisions(&case_dir);
    let value = |decision: &Decision, claim: &str| -> String {
        decision.claims.get(claim).cloned().unwrap_or_default()
    };
    let status = |decision: &Decision, action: &str| -> String {
        decision.actions.get(action).cloned().unwrap_or_default()
    };
    // Handed over at the tests-only revision: red, and no pull request.
    assert!(
        decisions.iter().any(|(decision, _)| {
            value(decision, "change.handed_over") == "true"
                && value(decision, "tests.red") == "true"
                && status(decision, "pr.open") == "blocked"
        }),
        "{text}"
    );
    // Both kinds changed but the tests red: still no pull request, and the implementation may
    // be edited again.
    assert!(
        decisions.iter().any(|(decision, raw)| {
            value(decision, "change.complete") == "true"
                && value(decision, "tests.pass") == "false"
                && status(decision, "pr.open") == "blocked"
                && names_claim(raw, "pr.open", "tests.pass")
                && status(decision, "implementation.edit") == "admissible"
        }),
        "{text}"
    );

    assert_eq!(validator_steps(&text, "pr.open"), 1, "{text}");
    let calls = memory.calls.borrow().clone();
    assert_eq!(
        calls.iter().filter(|c| c.starts_with("push ")).count(),
        1,
        "one push for the one complete change: {calls:?}\n{text}"
    );
    assert_eq!(
        calls.iter().filter(|c| c.starts_with("open ")).count(),
        1,
        "{calls:?}"
    );
    let pull_requests = records(&case_dir, "pull_request");
    assert_eq!(pull_requests.len(), 1, "{text}");
    assert_eq!(
        pull_requests[0]["subject_revision"],
        revision.as_str(),
        "{text}"
    );
    let passes: Vec<serde_json::Value> = records(&case_dir, "test_result")
        .into_iter()
        .filter(|record| {
            record["result"] == "pass" && record["subject_revision"] == revision.as_str()
        })
        .collect();
    assert_eq!(passes.len(), 1, "{text}");
}

/// A harness that edits the implementation first is refused by the protocol: the frontier never
/// offered `implementation.edit` (its precondition names `tests.red`), the step is discarded
/// whole, nothing is committed, recorded for it or pushed.
#[test]
fn a_harness_that_edits_the_implementation_first_is_refused_by_the_frontier() {
    let memory = Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    });
    let harness = Developer::new(&[Move::Implement("done\n"), Move::TestsWrite]);
    let (stop, text, case_dir, _) = delivered("implementation-first", &memory, &harness);
    assert_eq!(
        stop,
        Stop::StepRefused("implementation.edit was not offered".to_owned()),
        "{text}"
    );
    let offers = harness.offers.borrow();
    assert_eq!(offers.len(), 1, "{offers:?}\n{text}");
    assert!(
        offers[0].contains(&"tests.write".to_owned())
            && !offers[0].contains(&"implementation.edit".to_owned()),
        "{offers:?}\n{text}"
    );
    let (first, raw) = &decisions(&case_dir)[0];
    assert_eq!(
        first.actions.get("implementation.edit").map(String::as_str),
        Some("blocked"),
        "{}",
        first.json
    );
    assert!(
        names_claim(raw, "implementation.edit", "tests.red"),
        "{}",
        first.json
    );
    assert!(!text.contains("effect: committed"), "{text}");
    assert!(
        records(&case_dir, "change_record")
            .iter()
            .all(|record| record["result"] == "none"),
        "{text}"
    );
    assert!(
        memory.calls.borrow().is_empty(),
        "{:?}",
        memory.calls.borrow()
    );
}

/// `change.read` answers in the vocabulary the case's own protocol reads: on `/2` the shape of
/// the paths the merge would change (`none`, `tests`, `complete`), on `/1`, over the same
/// commits, `none` and `ahead`; one record, one result, either way.
#[test]
fn change_read_answers_in_the_vocabulary_the_cases_protocol_reads() {
    for (protocol, expected) in [
        (PROTOCOL, ["none", "tests", "complete"]),
        (PROTOCOL_ONE, ["none", "ahead", "ahead"]),
    ] {
        let name = if protocol == PROTOCOL {
            "shape"
        } else {
            "published"
        };
        let tree = scratch(&format!("vocabulary-{name}"));
        let validators: Vec<Box<dyn Validator>> = Vec::new();
        let options = RunOptions {
            protocol: protocol.to_owned(),
            intent: "deliver".to_owned(),
            inputs: Vec::new(),
            iterations: Some(1),
            hours: None,
            tokens: None,
            case_id: Some(name.to_owned()),
            gate: None,
        };
        let mut run = Run::open(&tree, options, &Idle, &validators).expect("the case opens");
        let mut results = Vec::new();
        for (seq, path) in [None, Some(TEST_PATH), Some(IMPLEMENTATION_PATH)]
            .into_iter()
            .enumerate()
        {
            if let Some(path) = path {
                let at = tree.join(path);
                fs::create_dir_all(at.parent().expect("a parent")).expect("a directory");
                fs::write(&at, "x\n").expect("a file");
                git(&tree, &["add", "-A"]);
                git(&tree, &["commit", "-q", "-m", path]);
            }
            let revision = git(&tree, &["rev-parse", "HEAD"]);
            run.case
                .set_revision("implementation", &revision)
                .expect("the revision moves");
            let observed = ChangeRead.observe(&tree, &revision, seq).expect("observes");
            assert_eq!(observed.records.len(), 1, "{}", observed.summary);
            results.push(observed.records[0].result.clone().unwrap_or_default());
        }
        assert_eq!(results, expected, "{name}");
    }
}

/// The vocabulary is decided by the results a protocol reads: `ahead` or nothing is the
/// published one, a shape result the shape, and one asking for both is refused, at
/// `Run::open` before a case directory exists.
#[test]
fn a_protocol_that_reads_change_record_as_ahead_and_as_a_shape_is_refused_at_open() {
    let set = |results: &[&str]| -> BTreeSet<String> {
        results.iter().map(|r| (*r).to_owned()).collect()
    };
    assert_eq!(
        ChangeRead::vocabulary(&set(&["ahead"])).expect("published"),
        ChangeVocabulary::Published
    );
    assert_eq!(
        ChangeRead::vocabulary(&set(&[])).expect("published"),
        ChangeVocabulary::Published
    );
    assert_eq!(
        ChangeRead::vocabulary(&set(&["none"])).expect("published"),
        ChangeVocabulary::Published
    );
    assert_eq!(
        ChangeRead::vocabulary(&set(&["complete", "tests"])).expect("shape"),
        ChangeVocabulary::Shape
    );
    let mixed = ChangeRead::vocabulary(&set(&["ahead", "complete"]))
        .expect_err("one record carries one result");
    assert!(
        mixed.to_string().contains("one record carries one result"),
        "{mixed}"
    );

    let tree = scratch("mixed");
    let protocol = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("tests-first-{}-mixed.yaml", std::process::id()));
    fs::write(
        &protocol,
        "format: protocol/1\nprotocol:\n  id: mixed.change\n  revision: 1\n\
         artifacts:\n  implementation:\n    description: the code\n\
         evidence_kinds:\n  change_record:\n    description: the change\n\
         claims:\n  change.made:\n    description: made\n    true_when:\n      evidence:\n        kind: change_record\n        result: ahead\n        subject: implementation\n\
         \x20 change.complete:\n    description: complete\n    true_when:\n      evidence:\n        kind: change_record\n        result: complete\n        subject: implementation\n\
         actions:\n  implementation.edit:\n    description: change the code\n    effect: write\n\
         \x20 change.read:\n    description: read the change\n    effect: read\n    may_produce:\n      - evidence: change_record\n\
         outcomes:\n  abandoned:\n    description: given up\n    requires:\n      decision: explicitly_abandoned\n",
    )
    .expect("a protocol");
    let options = RunOptions {
        protocol: protocol.display().to_string(),
        intent: "deliver".to_owned(),
        inputs: Vec::new(),
        iterations: Some(1),
        hours: None,
        tokens: None,
        case_id: Some("mixed".to_owned()),
        gate: None,
    };
    let validators: Vec<Box<dyn Validator>> = Vec::new();
    let error = Run::open(&tree, options, &Idle, &validators)
        .err()
        .map(|error| format!("{error:#}"))
        .expect("the protocol is refused");
    let _ = fs::remove_file(&protocol);
    assert!(
        error.contains("mixed.change/1") && error.contains("one record carries one result"),
        "{error}"
    );
    assert!(
        !tree.join(CASES_DIR).join("mixed").exists(),
        "a case was written for a refused protocol"
    );
}

/// The protocol as data, record by record: what each claim needs and which action each admits,
/// read by Canon from `story.delivery/2` over a case with no run.
#[test]
fn the_second_revision_orders_the_steps_by_its_claims() {
    let tree = scratch("ladder");
    let protocol = LoadedProtocol::from_file(Path::new(PROTOCOL))
        .expect("the protocol loads, validates and compiles");
    assert_eq!(protocol.id, "story.delivery/2");
    let mut case =
        CaseDir::open(&tree, protocol, "ladder", "deliver the story", &[]).expect("the case opens");
    let seed = case
        .revision("implementation")
        .expect("implementation has a revision");
    let status = |decision: &Decision, action: &str| -> String {
        decision.actions.get(action).cloned().unwrap_or_default()
    };
    let value = |decision: &Decision, claim: &str| -> String {
        decision.claims.get(claim).cloned().unwrap_or_default()
    };
    let mut seq = 0;
    let mut add = |case: &mut CaseDir, kind: &str, result: &str, revision: &str| {
        seq += 1;
        case.add_evidence(&record_of(
            kind,
            result,
            "implementation",
            revision,
            "ladder",
            seq,
        ))
        .expect("the record is written");
    };

    // At the seed, with nothing recorded: the tests may be written, the implementation not.
    let decision = case.evaluate(1).expect("evaluates");
    assert_eq!(
        status(&decision, "tests.write"),
        "admissible",
        "{}",
        decision.json
    );
    assert_eq!(
        status(&decision, "change.read"),
        "admissible",
        "{}",
        decision.json
    );
    assert_eq!(
        status(&decision, "implementation.edit"),
        "blocked",
        "{}",
        decision.json
    );
    assert_eq!(status(&decision, "pr.open"), "blocked", "{}", decision.json);

    // A failing run at a revision whose change is to test paths only: tests.red, and the
    // implementation may be edited; a pull request may not be opened.
    add(&mut case, "change_record", "tests", &seed);
    add(&mut case, "test_result", "fail", &seed);
    let decision = case.evaluate(2).expect("evaluates");
    assert_eq!(value(&decision, "tests.red"), "true", "{}", decision.json);
    assert_eq!(
        value(&decision, "change.complete"),
        "false",
        "{}",
        decision.json
    );
    assert_eq!(
        status(&decision, "implementation.edit"),
        "admissible",
        "{}",
        decision.json
    );
    assert_eq!(status(&decision, "pr.open"), "blocked", "{}", decision.json);

    // A failing run at a revision with both kinds changed: not red, but the implementation
    // may still be edited (it was started), and the pull request waits for green.
    let second = "1111111111111111111111111111111111111111";
    case.set_revision("implementation", second)
        .expect("the revision moves");
    add(&mut case, "change_record", "complete", second);
    add(&mut case, "test_result", "fail", second);
    add(&mut case, "gate_result", "pass", second);
    add(&mut case, "handover", "", second);
    let decision = case.evaluate(3).expect("evaluates");
    assert_eq!(value(&decision, "tests.red"), "false", "{}", decision.json);
    assert_eq!(
        value(&decision, "change.complete"),
        "true",
        "{}",
        decision.json
    );
    assert_eq!(
        value(&decision, "change.handed_over"),
        "true",
        "{}",
        decision.json
    );
    assert_eq!(
        status(&decision, "implementation.edit"),
        "admissible",
        "{}",
        decision.json
    );
    assert_eq!(status(&decision, "pr.open"), "blocked", "{}", decision.json);

    // Green with both kinds changed and handed over, at one revision: the pull request opens;
    // then CI, the merge under its grant, and the outcome, as on /1.
    let third = "2222222222222222222222222222222222222222";
    case.set_revision("implementation", third)
        .expect("the revision moves");
    add(&mut case, "change_record", "complete", third);
    add(&mut case, "test_result", "pass", third);
    add(&mut case, "gate_result", "pass", third);
    let decision = case.evaluate(4).expect("evaluates");
    assert_eq!(status(&decision, "pr.open"), "blocked", "{}", decision.json);
    add(&mut case, "handover", "", third);
    let decision = case.evaluate(5).expect("evaluates");
    assert_eq!(
        status(&decision, "pr.open"),
        "admissible",
        "{}",
        decision.json
    );
    add(&mut case, "pull_request", "open", third);
    add(&mut case, "ci_result", "pass", third);
    let decision = case.evaluate(6).expect("evaluates");
    assert_eq!(
        status(&decision, "repository.merge"),
        "approval-required",
        "{}",
        decision.json
    );
    add(&mut case, "merge_observation", "merged", third);
    let decision = case.evaluate(7).expect("evaluates");
    assert_eq!(
        decision.outcomes.get("accepted").map(String::as_str),
        Some("legitimate"),
        "{}",
        decision.json
    );

    // Only the implementation changed (a test path removed): the implementation may be
    // edited, the change is not complete.
    let fourth = "3333333333333333333333333333333333333333";
    case.set_revision("implementation", fourth)
        .expect("the revision moves");
    add(&mut case, "change_record", "implementation", fourth);
    add(&mut case, "test_result", "pass", fourth);
    let decision = case.evaluate(8).expect("evaluates");
    assert_eq!(
        value(&decision, "implementation.changed"),
        "true",
        "{}",
        decision.json
    );
    assert_eq!(
        value(&decision, "change.complete"),
        "false",
        "{}",
        decision.json
    );
    assert_eq!(
        status(&decision, "implementation.edit"),
        "admissible",
        "{}",
        decision.json
    );
    assert_eq!(status(&decision, "pr.open"), "blocked", "{}", decision.json);
}

/// A test command that passes at every revision, whatever the tests say.
struct AlwaysPass;

impl Validator for AlwaysPass {
    fn name(&self) -> &str {
        "always-pass"
    }

    fn action(&self) -> &str {
        "tests.run"
    }

    fn kind(&self) -> &str {
        "test_result"
    }

    fn subject(&self) -> &str {
        "implementation"
    }

    fn observe(&self, _: &Path, revision: &str, seq: usize) -> anyhow::Result<Observation> {
        Ok(Observation {
            summary: "pass".to_owned(),
            artifact: None,
            records: vec![record_of(
                self.kind(),
                "pass",
                self.subject(),
                revision,
                self.name(),
                seq,
            )],
        })
    }
}

/// A developer that only ever writes tests: offered `tests.write`, it appends one more line to
/// the story's test file, so every step commits a change; offered nothing else it could take, it
/// hands over. One that `hands_over` takes `change.handover` whenever it is offered, a step
/// that changes no file between two of its test writes.
struct TestWriter {
    hands_over: bool,
}

impl Harness for TestWriter {
    fn name(&self) -> &str {
        "test-writer"
    }

    fn step(&self, context: &StepContext<'_>) -> anyhow::Result<StepReport> {
        let handover = context.offered.contains(&"change.handover".to_owned());
        let action = if self.hands_over && handover {
            "change.handover"
        } else if context.offered.contains(&"tests.write".to_owned()) {
            let at = context.tree.join(TEST_PATH);
            fs::create_dir_all(at.parent().expect("a parent"))?;
            let mut text = fs::read_to_string(&at).unwrap_or_default();
            text.push_str("one more test\n");
            fs::write(&at, text)?;
            "tests.write"
        } else {
            "change.handover"
        };
        Ok(StepReport {
            action: action.to_owned(),
            summary: format!("played {action}"),
            tokens: Some(100),
        })
    }
}

/// On `story.delivery/2` a `tests.write` step after which the tests still pass is no progress
/// toward `tests.red`, which the implementation step needs: after two such steps in a row the
/// run stops `NoProgress`, naming `tests.red` and the test command's result, instead of being
/// offered `tests.write` again without end. A hand-over between them, which changes no file,
/// moves nothing toward `tests.red` either and does not start the count again. The step budget
/// here is only a guard: a run that does not stop on its own ends on it, and fails, rather than
/// hanging the suite.
#[test]
fn tests_that_still_pass_after_two_tests_write_steps_stop_the_run_naming_tests_red() {
    for hands_over in [false, true] {
        let name = if hands_over {
            "always-pass-handover"
        } else {
            "always-pass"
        };
        let tree = scratch(name);
        let validators: Vec<Box<dyn Validator>> =
            vec![Box::new(AlwaysPass), Box::new(Green), Box::new(ChangeRead)];
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
        let harness = TestWriter { hands_over };
        let mut run = Run::open(&tree, options, &harness, &validators).expect("the case opens");
        let mut out = Vec::new();
        let stop = run.run(&mut out).expect("the run stops without an error");
        let text = String::from_utf8(out).expect("utf-8");
        let written = text
            .lines()
            .filter(|line| {
                line.starts_with("step ") && line.ends_with(": tests.write (test-writer)")
            })
            .count();
        let stopped = stop.to_string();
        assert!(
            stopped.starts_with("NoProgress (") && stopped.contains("tests.red"),
            "hands over {hands_over}: the run did not stop NoProgress naming tests.red: {stopped}\n{text}"
        );
        assert!(
            stopped.contains("pass"),
            "the stop does not name the test command's result: {stopped}"
        );
        assert_eq!(
            written, 2,
            "hands over {hands_over}: tests.write steps before the stop: {text}"
        );
        let events = fs::read_to_string(run.case.dir.join("events.jsonl")).expect("events");
        let last: serde_json::Value =
            serde_json::from_str(events.lines().last().expect("a line")).expect("json");
        assert_eq!(last["event"], "stop", "{events}");
        assert_eq!(last["stop"], stopped.as_str(), "{events}");
    }
}
