//! story:story-delivery-tests-first: under the default `gate_on: after_handover` of
//! `story.delivery/3`, the repository's gate runs once, at the handed-over revision. At every
//! revision of the case before it, the gate's precondition does not hold, and takt says so
//! instead of running it: it prints `gate.run: deferred (precondition ... not true at <rev>)`
//! and records one `gate.deferred` line in `events.jsonl`, which is never evidence and never a
//! pass. Scenario 1 with a fake harness: the tests first, a failing `tests.run` at their
//! revision, the implementation, a green `tests.run`, the hand-over, the gate, `pr.open` with
//! one push.

#[path = "support/own_home.rs"]
mod own_home;

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

/// The story's test file and its implementation file.
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

/// A scratch git tree with one commit, under this crate's build directory, removed when dropped.
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
        .join(format!("gate-deferral-{}-{name}", std::process::id()));
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

/// Whether `path` is in the tree of `revision`.
fn at(tree: &Path, revision: &str, path: &str) -> bool {
    Command::new("git")
        .args(["cat-file", "-e", &format!("{revision}:{path}")])
        .current_dir(tree)
        .status()
        .expect("git runs")
        .success()
}

/// A validator of `action` that observes `pass` at every revision, but `fail` for `tests.run`
/// when `red` and the revision holds the story's tests and not its implementation.
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

/// A developer that writes the story's tests first, the implementation once
/// `implementation.edit` is offered, then hands the change over; it requests the merge when
/// that is offered.
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

/// The lines of the case's `events.jsonl` whose `event` is `event`.
fn events(case_dir: &Path, event: &str) -> Vec<serde_json::Value> {
    fs::read_to_string(case_dir.join("events.jsonl"))
        .expect("events.jsonl")
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("json"))
        .filter(|line| line["event"] == event)
        .collect()
}

/// The commit of the case's branch that added `path`.
fn added(tree: &Path, path: &str) -> String {
    git(
        tree,
        &["log", "--format=%H", "--diff-filter=A", "HEAD", "--", path],
    )
}

/// Scenario 1 under the default `gate_on: after_handover`: one `gate_result`, at the handed-over
/// revision; every revision before it prints its deferral and records one `gate.deferred`
/// line, and no deferral is a pass.
#[test]
fn a_red_revision_runs_no_gate() {
    let tree = scratch("scenario-1");
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
    assert!(
        settings
            .pairs()
            .contains(&("gate_on".to_owned(), "after_handover".to_owned())),
        "{:?}",
        settings.pairs()
    );
    let memory = Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    });
    let forge: Rc<dyn Forge> = memory.clone();
    let validators: Vec<Box<dyn Validator>> = vec![
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
    ];
    let options = RunOptions {
        protocol: settings.spec.clone(),
        intent: "remove a todo by its number".to_owned(),
        inputs: settings.pairs(),
        iterations: Some(40),
        hours: None,
        tokens: None,
        case_id: Some("scenario-1".to_owned()),
        gate: Some("repository.merge".to_owned()),
    };
    let harness = TestsFirst;
    let mut run = Run::open(&tree, options, &harness, &validators).expect("the case opens");
    let mut out = Vec::new();
    let stop = run.run(&mut out).map_err(|error| format!("{error:#}"));
    let case_dir = run.case.dir.clone();
    drop(run);
    let text = String::from_utf8(out).expect("utf-8");
    let calls = memory.calls.borrow().clone();
    assert_eq!(
        stop,
        Ok(Stop::ApprovalRequired(vec!["repository.merge".to_owned()])),
        "{text}"
    );

    // The order: the tests' revision, a failing tests.run there, then the implementation's.
    let tests_revision = added(&tree, TESTS);
    let implementation_revision = added(&tree, IMPLEMENTATION);
    assert!(!tests_revision.is_empty(), "{text}");
    assert!(!implementation_revision.is_empty(), "{text}");
    assert_eq!(
        git(
            &tree,
            &["rev-parse", &format!("{implementation_revision}^")]
        ),
        tests_revision,
        "the implementation is not the commit after the tests: {text}"
    );
    let test_results = records(&case_dir, "test_result");
    assert!(
        test_results
            .iter()
            .any(|r| r["subject_revision"] == tests_revision.as_str() && r["result"] == "fail"),
        "no failing tests.run at the tests' revision: {test_results:?}\n{text}"
    );

    let handovers = records(&case_dir, "handover");
    assert_eq!(handovers.len(), 1, "{handovers:?}\n{text}");
    let handed_over = handovers[0]["subject_revision"]
        .as_str()
        .expect("a revision")
        .to_owned();
    assert_eq!(handed_over, implementation_revision, "{text}");

    let gates = records(&case_dir, "gate_result");
    assert_eq!(
        gates.len(),
        1,
        "the gate ran more than once: {gates:?}\n{text}"
    );
    assert_eq!(
        gates[0]["subject_revision"],
        handed_over.as_str(),
        "the gate did not run at the handed-over revision: {gates:?}\n{text}"
    );

    // Every revision the case observed before the handed-over one: a deferral, printed and
    // recorded once, carrying no result, and no gate_result there.
    let mut earlier: Vec<String> = test_results
        .iter()
        .filter_map(|r| r["subject_revision"].as_str().map(str::to_owned))
        .filter(|revision| *revision != handed_over)
        .collect();
    earlier.sort();
    earlier.dedup();
    assert!(
        earlier.contains(&tests_revision),
        "the red revision is not among the earlier ones: {earlier:?}\n{text}"
    );
    let deferred = events(&case_dir, "gate.deferred");
    // The revision the gate ran at is never said to be one it did not run at.
    assert!(
        !deferred
            .iter()
            .any(|line| line["revision"] == handed_over.as_str()),
        "a deferral at the handed-over revision: {deferred:?}\n{text}"
    );
    for revision in &earlier {
        let lines: Vec<&serde_json::Value> = deferred
            .iter()
            .filter(|line| line["revision"] == revision.as_str())
            .collect();
        assert_eq!(
            lines.len(),
            1,
            "gate.deferred lines at {revision}: {deferred:?}\n{text}"
        );
        assert_eq!(lines[0]["action"], "gate.run", "{lines:?}");
        assert!(
            lines[0].get("result").is_none(),
            "a deferral carries no result: {lines:?}"
        );
        assert!(
            lines[0]["unmet"]
                .as_array()
                .is_some_and(|unmet| !unmet.is_empty()),
            "a deferral names what its precondition lacks: {lines:?}"
        );
        // Printed as the loop prints a revision, its first eight characters.
        let short = &revision[..8];
        let printed = text
            .lines()
            .filter(|line| {
                line.contains("gate.run: deferred (precondition ")
                    && line.ends_with(&format!(" not true at {short})"))
            })
            .count();
        assert_eq!(printed, 1, "deferral lines printed at {revision}:\n{text}");
        assert!(
            !gates
                .iter()
                .any(|g| g["subject_revision"] == revision.as_str()),
            "a deferred revision has a gate_result: {gates:?}"
        );
    }
    // A deferral is never evidence: no record under the case names it.
    assert!(
        fs::read_dir(case_dir.join("evidence"))
            .expect("evidence")
            .all(|entry| {
                !fs::read_to_string(entry.expect("an entry").path())
                    .expect("a file")
                    .contains("deferred")
            }),
        "a deferral was written as evidence: {text}"
    );

    // One pull request, one push.
    assert_eq!(records(&case_dir, "pull_request").len(), 1, "{text}");
    assert_eq!(
        calls.iter().filter(|c| c.starts_with("push ")).count(),
        1,
        "{calls:?}\n{text}"
    );
    assert_eq!(
        calls.iter().filter(|c| c.starts_with("open ")).count(),
        1,
        "{calls:?}\n{text}"
    );
}
