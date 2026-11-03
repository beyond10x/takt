//! Adversary pass 1 on story:story-delivery-tests-first (the gate deferred at a red revision).
//! Scenario 1 with a fake harness, as `gate_deferral.rs` builds it, driven further: a case
//! resumed one step at a time, `gate_on: each_revision`, a record line that does not read at
//! the loop's start, the printed line against the recorded one, and the line against the ESS
//! shape of `takt.work.RunGateDeferred`.

#[path = "support/own_home.rs"]
mod own_home;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

use takt::evidence::Record;
use takt::forge::{Checks, Forge, Memory};
use takt::harness::{Harness, StepContext, StepReport};
use takt::protocol::{SettingsRequest, default_protocol, resolve_settings};
use takt::run_config::ProtocolSettings;
use takt::validator::{ChangeRead, CiRead, Observation, PrOpen, Validator};
use takt::{Run, RunOptions, Stop};

const TESTS: &str = "tests/remove_test.txt";
const IMPLEMENTATION: &str = "src/remove.txt";

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
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "w49-adv-gate-deferral-{}-{name}",
        std::process::id()
    ));
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

fn at(tree: &Path, revision: &str, path: &str) -> bool {
    Command::new("git")
        .args(["cat-file", "-e", &format!("{revision}:{path}")])
        .current_dir(tree)
        .status()
        .expect("git runs")
        .success()
}

struct Scripted {
    action: &'static str,
    kind: &'static str,
    red: bool,
}

impl Validator for Scripted {
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

    fn observe(&self, tree: &Path, revision: &str, seq: usize) -> anyhow::Result<Observation> {
        let fails = self.red && at(tree, revision, TESTS) && !at(tree, revision, IMPLEMENTATION);
        let result = if fails { "fail" } else { "pass" };
        Ok(Observation {
            summary: result.to_owned(),
            artifact: None,
            records: vec![Record::new(
                format!("{}-{seq}", self.kind),
                self.kind,
                Some(result.to_owned()),
                self.subject(),
                revision,
                self.name(),
            )],
        })
    }
}

/// The developer of `gate_deferral.rs`: the tests first, then the implementation, then the
/// hand-over. It reads only the tree, so a resumed run gets the same steps.
struct TestsFirst;

impl Harness for TestsFirst {
    fn name(&self) -> &str {
        "tests-first"
    }

    fn step(&self, context: &StepContext<'_>) -> anyhow::Result<StepReport> {
        let offered = |action: &str| context.offered.iter().any(|offer| offer == action);
        let write = |path: &str, action: &str| -> anyhow::Result<StepReport> {
            let file = context.tree.join(path);
            fs::create_dir_all(file.parent().expect("a parent"))?;
            fs::write(&file, format!("{action}\n"))?;
            Ok(StepReport {
                action: action.to_owned(),
                summary: format!("wrote {path}"),
                tokens: Some(100),
            })
        };
        if !context.tree.join(TESTS).exists() && offered("tests.write") {
            return write(TESTS, "tests.write");
        }
        if !context.tree.join(IMPLEMENTATION).exists() && offered("implementation.edit") {
            return write(IMPLEMENTATION, "implementation.edit");
        }
        let action = if context.approvals.iter().any(|a| a == "repository.merge") {
            "repository.merge"
        } else if offered("change.handover") {
            "change.handover"
        } else {
            context
                .offered
                .first()
                .map_or("repository.inspect", String::as_str)
        };
        Ok(StepReport {
            action: action.to_owned(),
            summary: "the change is complete".to_owned(),
            tokens: Some(100),
        })
    }
}

fn validators(forge: Rc<dyn Forge>) -> Vec<Box<dyn Validator>> {
    vec![
        Box::new(Scripted {
            action: "tests.run",
            kind: "test_result",
            red: true,
        }),
        Box::new(Scripted {
            action: "gate.run",
            kind: "gate_result",
            red: false,
        }),
        Box::new(PrOpen::new(forge.clone())),
        Box::new(CiRead::new(forge)),
        Box::new(ChangeRead),
    ]
}

/// The options of scenario 1 on takt's defaults (`story.delivery/3`, `gate_on: after_handover`),
/// with `gate_on` set to `gate_on` when given, and `iterations` steps.
fn options(id: &str, gate_on: Option<&str>, iterations: u64) -> RunOptions {
    let settings = resolve_settings(&SettingsRequest {
        file: &ProtocolSettings::default(),
        flag_protocol: None,
        flag_inputs: &[],
        tags: &[],
        default_protocol: default_protocol(true),
        file_protocol_applies: true,
    })
    .expect("the settings resolve");
    assert_eq!(settings.protocol.id, "story.delivery/3");
    let mut inputs = settings.pairs();
    if let Some(value) = gate_on {
        for (key, set) in &mut inputs {
            if key == "gate_on" {
                *set = value.to_owned();
            }
        }
    }
    RunOptions {
        protocol: settings.spec.clone(),
        intent: "remove a todo by its number".to_owned(),
        inputs,
        iterations: Some(iterations),
        hours: None,
        tokens: None,
        case_id: Some(id.to_owned()),
        gate: Some("repository.merge".to_owned()),
    }
}

/// One whole run of scenario 1: its stop, what it printed, the case directory.
fn whole(tree: &Path, id: &str, gate_on: Option<&str>) -> (Result<Stop, String>, String, PathBuf) {
    let memory = Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    });
    let forge: Rc<dyn Forge> = memory;
    let validators = validators(forge);
    let harness = TestsFirst;
    let mut run =
        Run::open(tree, options(id, gate_on, 40), &harness, &validators).expect("the case opens");
    let mut out = Vec::new();
    let stop = run.run(&mut out).map_err(|error| format!("{error:#}"));
    let case_dir = run.case.dir.clone();
    drop(run);
    (stop, String::from_utf8(out).expect("utf-8"), case_dir)
}

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

/// The readable lines of the case's `events.jsonl` whose `event` is `event`; a line that does
/// not read is passed over.
fn events(case_dir: &Path, event: &str) -> Vec<serde_json::Value> {
    fs::read_to_string(case_dir.join("events.jsonl"))
        .expect("events.jsonl")
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|line| line["event"] == event)
        .collect()
}

/// How many `gate.deferred` lines name each revision.
fn deferred_by_revision(case_dir: &Path) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for line in events(case_dir, "gate.deferred") {
        let revision = line["revision"].as_str().expect("a revision").to_owned();
        *counts.entry(revision).or_insert(0) += 1;
    }
    counts
}

/// How many printed deferral lines end on each eight-character revision.
fn printed_by_short(text: &str) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("gate.run: deferred (precondition ")
            && let Some((_, short)) = rest.rsplit_once(" not true at ")
        {
            let short = short.trim_end_matches(')').to_owned();
            *counts.entry(short).or_insert(0) += 1;
        }
    }
    counts
}

/// A case resumed one step at a time (a second, third, ... `takt run` on it) says each deferral
/// once over all its runs: one `gate.deferred` line per revision, one printed line per revision,
/// at every revision the case observed before the handed-over one, and one `gate_result`.
#[test]
fn adv_a_a_case_resumed_step_by_step_says_each_deferral_once() {
    let tree = scratch("resumed");
    let id = "resumed";
    let memory = Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    });
    let forge: Rc<dyn Forge> = memory;
    let validators = validators(forge);
    let harness = TestsFirst;
    let mut text = String::new();
    let mut run =
        Run::open(&tree, options(id, None, 1), &harness, &validators).expect("the case opens");
    let case_dir = run.case.dir.clone();
    let mut stop = None;
    for attempt in 0..60 {
        let mut out = Vec::new();
        let this = run.run(&mut out).map_err(|error| format!("{error:#}"));
        text.push_str(&String::from_utf8_lossy(&out));
        drop(run);
        if !matches!(this, Ok(Stop::Iterations(_))) {
            stop = Some(this);
            break;
        }
        assert!(attempt < 59, "the case never stopped: {text}");
        run = Run::resume(&tree, options(id, None, 1), id, &harness, &validators)
            .expect("the case resumes");
    }
    assert_eq!(
        stop,
        Some(Ok(Stop::ApprovalRequired(vec![
            "repository.merge".to_owned()
        ]))),
        "{text}"
    );
    let gates = records(&case_dir, "gate_result");
    assert_eq!(
        gates.len(),
        1,
        "story:story-delivery-tests-first: {gates:?}\n{text}"
    );
    let handed_over = gates[0]["subject_revision"].as_str().expect("a revision");
    let earlier: BTreeSet<String> = records(&case_dir, "test_result")
        .iter()
        .filter_map(|r| r["subject_revision"].as_str().map(str::to_owned))
        .filter(|revision| revision != handed_over)
        .collect();
    assert!(!earlier.is_empty(), "{text}");
    let recorded = deferred_by_revision(&case_dir);
    let printed = printed_by_short(&text);
    for revision in &earlier {
        assert_eq!(
            recorded.get(revision),
            Some(&1),
            "story:story-delivery-tests-first: gate.deferred lines at {revision} over the resumed runs: {recorded:?}\n{text}"
        );
        assert_eq!(
            printed.get(&revision[..8]),
            Some(&1),
            "story:story-delivery-tests-first: printed deferrals at {revision} over the resumed runs: {printed:?}\n{text}"
        );
    }
    for (revision, n) in &recorded {
        assert_eq!(
            *n, 1,
            "story:story-delivery-tests-first: {revision}: {recorded:?}\n{text}"
        );
    }
}

/// Under `gate_on: each_revision` the gate runs at every revision the case's tests ran at, the
/// red one included, and no deferral is recorded or printed.
#[test]
fn adv_a_each_revision_runs_the_gate_everywhere_and_defers_nothing() {
    let tree = scratch("each-revision");
    let (stop, text, case_dir) = whole(&tree, "each-revision", Some("each_revision"));
    let yaml = fs::read_to_string(case_dir.join("case.yaml")).expect("case.yaml");
    assert!(yaml.contains("gate_on: each_revision"), "{yaml}");
    assert!(stop.is_ok(), "{stop:?}\n{text}");
    let deferred = events(&case_dir, "gate.deferred");
    assert!(
        deferred.is_empty(),
        "story:story-delivery-tests-first: a deferral under each_revision: {deferred:?}\n{text}"
    );
    assert!(!text.contains("deferred"), "{text}");
    let gated: BTreeSet<String> = records(&case_dir, "gate_result")
        .iter()
        .filter_map(|r| r["subject_revision"].as_str().map(str::to_owned))
        .collect();
    let tested: BTreeSet<String> = records(&case_dir, "test_result")
        .iter()
        .filter_map(|r| r["subject_revision"].as_str().map(str::to_owned))
        .collect();
    assert!(tested.len() >= 2, "{tested:?}\n{text}");
    for revision in &tested {
        assert!(
            gated.contains(revision),
            "story:story-delivery-tests-first: under each_revision no gate ran at {revision}: {gated:?}\n{text}"
        );
    }
}

/// The printed line and the recorded line say the same: one printed
/// `gate.run: deferred (precondition <claims> not true at <rev>)` per `gate.deferred` line,
/// `<claims>` its `unmet` joined by `, `, `<rev>` its revision's first eight characters. And the
/// line is the ESS shape of `takt.work.RunGateDeferred` on `takt.work.RunEventLine`: exactly
/// `at`, `step`, `event`, `action`, `revision`, `unmet`; `action` `gate.run`; `revision` a full
/// commit of the case's branch; every `unmet` a claim of `story.delivery/3` and none twice.
#[test]
fn adv_a_the_printed_deferral_is_the_recorded_one_and_its_ess_shape() {
    let tree = scratch("shape");
    let (stop, text, case_dir) = whole(&tree, "shape", None);
    assert_eq!(
        stop,
        Ok(Stop::ApprovalRequired(vec!["repository.merge".to_owned()])),
        "{text}"
    );
    let protocol = include_str!("../../../protocols/story-delivery/3.yaml");
    let model = b10x_canon::model::parse(protocol).expect("story.delivery/3 parses");
    let claims: BTreeSet<String> = model
        .claims
        .iter()
        .map(|(id, _)| id.as_str().to_owned())
        .collect();
    let deferred = events(&case_dir, "gate.deferred");
    assert!(!deferred.is_empty(), "{text}");
    let printed: Vec<&str> = text
        .lines()
        .filter(|line| line.starts_with("gate.run: deferred"))
        .collect();
    assert_eq!(printed.len(), deferred.len(), "{printed:?}\n{deferred:?}");
    for (line, said) in deferred.iter().zip(&printed) {
        let keys: BTreeSet<&str> = line
            .as_object()
            .expect("an object")
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(
            keys,
            BTreeSet::from(["at", "step", "event", "action", "revision", "unmet"]),
            "story:story-delivery-tests-first: {line}"
        );
        assert_eq!(line["action"], "gate.run", "{line}");
        let revision = line["revision"].as_str().expect("a revision");
        assert_eq!(revision.len(), 40, "{line}");
        assert_eq!(
            git(&tree, &["cat-file", "-t", revision]),
            "commit",
            "{line}"
        );
        let unmet: Vec<&str> = line["unmet"]
            .as_array()
            .expect("unmet is a list")
            .iter()
            .map(|claim| claim.as_str().expect("a claim id"))
            .collect();
        assert!(!unmet.is_empty(), "{line}");
        assert_eq!(
            unmet.iter().collect::<BTreeSet<_>>().len(),
            unmet.len(),
            "{line}"
        );
        for claim in &unmet {
            assert!(claims.contains(*claim), "{claim} is no claim: {line}");
        }
        let expected = format!(
            "gate.run: deferred (precondition {} not true at {})",
            unmet.join(", "),
            &revision[..8]
        );
        assert_eq!(*said, expected, "story:story-delivery-tests-first\n{text}");
    }
}

/// A `gate.deferred` line says "the repository's gate did not run at `revision`"
/// (`crates/takt/src/events.rs`, the ESS `takt.work.RunGateDeferred`, the events reference):
/// so no revision the gate ran at carries one. Scenario 1 hands over at the implementation's
/// revision, which the loop reaches with the hand-over still to come.
#[test]
fn adv_a_no_revision_the_gate_ran_at_is_recorded_as_deferred() {
    let tree = scratch("ran-there");
    let (stop, text, case_dir) = whole(&tree, "ran-there", None);
    assert_eq!(
        stop,
        Ok(Stop::ApprovalRequired(vec!["repository.merge".to_owned()])),
        "{text}"
    );
    let gates = records(&case_dir, "gate_result");
    assert_eq!(gates.len(), 1, "{gates:?}\n{text}");
    let ran = gates[0]["subject_revision"].as_str().expect("a revision");
    let recorded = deferred_by_revision(&case_dir);
    assert_eq!(
        recorded.get(ran),
        None,
        "story:story-delivery-tests-first: the gate ran at {ran} and the record says it did not run there: {recorded:?}\n{text}"
    );
}

/// A record line that does not read at the loop's start (not a cut-short last line, which the
/// record repairs and skips) leaves the resumed run blind to the deferrals already recorded:
/// it says the same revision's deferral again. The ESS says once per revision, "an earlier
/// run's line counting".
#[test]
fn adv_a_an_unreadable_record_line_does_not_say_a_deferral_twice() {
    let tree = scratch("unreadable");
    let id = "unreadable";
    let memory = Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    });
    let forge: Rc<dyn Forge> = memory;
    let validators = validators(forge);
    let harness = TestsFirst;
    let mut run =
        Run::open(&tree, options(id, None, 1), &harness, &validators).expect("the case opens");
    let case_dir = run.case.dir.clone();
    let mut out = Vec::new();
    let first = run.run(&mut out).map_err(|error| format!("{error:#}"));
    drop(run);
    let mut text = String::from_utf8_lossy(&out).into_owned();
    assert!(
        matches!(first, Ok(Stop::Iterations(_))),
        "{first:?}\n{text}"
    );
    let before = deferred_by_revision(&case_dir);
    assert!(!before.is_empty(), "no deferral in the first run: {text}");

    let path = case_dir.join("events.jsonl");
    let record = fs::read_to_string(&path).expect("events.jsonl");
    let (head, tail) = record.split_once('\n').expect("two lines");
    fs::write(&path, format!("{head}\n{{\"event\": \"op\n{tail}")).expect("rewritten");

    let mut run = Run::resume(&tree, options(id, None, 1), id, &harness, &validators)
        .expect("the case resumes");
    let mut out = Vec::new();
    let second = run.run(&mut out).map_err(|error| format!("{error:#}"));
    drop(run);
    text.push_str(&String::from_utf8_lossy(&out));
    let after = deferred_by_revision(&case_dir);
    for (revision, n) in &after {
        assert_eq!(
            *n, 1,
            "story:story-delivery-tests-first: {revision} deferred {n} times across a resume over an unreadable line ({second:?}): {after:?}\n{text}"
        );
    }
}

/// The trace of scenario 1 (`takt trace`, read from the case) holds one gate operation, the one
/// `gate.run` step, `pass`; no deferral becomes a gate operation or a pass, and every other op
/// naming `gate.run` is that step.
#[test]
fn adv_a_the_trace_holds_one_gate_op_and_no_deferral_as_a_pass() {
    let tree = scratch("trace");
    let (stop, text, case_dir) = whole(&tree, "trace", None);
    assert!(stop.is_ok(), "{stop:?}\n{text}");
    assert!(!events(&case_dir, "gate.deferred").is_empty(), "{text}");
    let trace = takt::trace::Trace::from_case(&case_dir, None, None).expect("the trace reads");
    let gates: Vec<&takt::trace::Op> = trace
        .ops
        .iter()
        .filter(|op| op.kind.name() == "gate" || op.action.as_deref() == Some("gate.run"))
        .collect();
    assert_eq!(
        gates.len(),
        1,
        "story:story-delivery-tests-first: {gates:?}\n{text}"
    );
    assert_eq!(gates[0].kind.name(), "gate", "{gates:?}");
    assert_eq!(gates[0].outcome.as_deref(), Some("pass"), "{gates:?}");
}
