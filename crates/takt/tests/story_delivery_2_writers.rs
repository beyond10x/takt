//! story:story-delivery-2-evidence-writers: a story case on the default protocol,
//! `story.delivery/2` with its default inputs (`tests_first: required`), reaches `pr.open` with a
//! harness that writes its tests first. takt writes one `tests_change` record about the
//! implementation at the revision the `tests.write` step committed, and `change.read` writes one
//! `paths_changed` record beside its `change_record`: `both`, `tests_only`, `implementation_only`
//! or `none` by the paths the revision changes outside `.engineering/` since the base. Without
//! the two writers `tests.shown_red` and `change.complete` never hold: `implementation.edit` is
//! never admitted, and `pr.open` never.

#[path = "support/own_home.rs"]
mod own_home;

use std::cell::Cell;
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
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "story-delivery-2-writers-{}-{name}",
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
/// when `red` and the revision holds the story's tests and not its implementation: the tests a
/// developer writes first fail until the implementation is there.
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

/// A developer that writes the story's tests first, then, when `implements`, the implementation
/// once `implementation.edit` is offered, then hands the change over by taking
/// `change.handover`, changing nothing; it requests the merge when that is offered. A step with
/// nothing of that offered takes the first action offered, changing nothing.
struct TestsFirst {
    implements: bool,
    calls: Cell<usize>,
}

impl Harness for TestsFirst {
    fn name(&self) -> &str {
        "tests-first"
    }

    fn step(&self, context: &StepContext<'_>) -> anyhow::Result<StepReport> {
        self.calls.set(self.calls.get() + 1);
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
        if self.implements
            && !context.tree.join(IMPLEMENTATION).exists()
            && offered("implementation.edit")
        {
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

/// A finished run: its stop, what it printed, the forge's calls, the case directory and the
/// tree it ran in, kept until dropped.
struct Delivered {
    stop: Result<Stop, String>,
    text: String,
    calls: Vec<String>,
    case_dir: PathBuf,
    tree: Scratch,
}

/// A delivery run of the story on `protocol`, or on the default protocol for a story with its
/// default inputs when `None`, as `takt run` with no `--protocol` and no `--input` opens it;
/// the tests observed by [`Scripted`] (`red` as given), the gate green, `pr.open` and
/// `ci.read` on a fake forge whose checks pass, and `change.read`.
fn delivered(name: &str, protocol: Option<&str>, red: bool, harness: &dyn Harness) -> Delivered {
    let tree = scratch(name);
    let (protocol, inputs) = match protocol {
        Some(protocol) => (protocol.to_owned(), Vec::new()),
        None => {
            let file = ProtocolSettings::default();
            let settings = resolve_settings(&SettingsRequest {
                file: &file,
                flag_protocol: None,
                flag_inputs: &[],
                tags: &[],
                default_protocol: default_protocol(true),
                file_protocol_applies: true,
            })
            .expect("the settings resolve");
            assert_eq!(settings.protocol.id, "story.delivery/2");
            (settings.spec.clone(), settings.pairs())
        }
    };
    let memory = Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    });
    let forge: Rc<dyn Forge> = memory.clone();
    let validators: Vec<Box<dyn Validator>> = vec![
        Box::new(Scripted {
            action: "tests.run",
            kind: "test_result",
            red,
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
        protocol,
        intent: "remove a todo by its number".to_owned(),
        inputs,
        iterations: Some(40),
        hours: None,
        tokens: None,
        case_id: Some(name.to_owned()),
        gate: Some("repository.merge".to_owned()),
    };
    let mut run = Run::open(&tree, options, harness, &validators).expect("the case opens");
    let mut out = Vec::new();
    let stop = run.run(&mut out).map_err(|error| format!("{error:#}"));
    let case_dir = run.case.dir.clone();
    drop(run);
    let calls = memory.calls.borrow().clone();
    Delivered {
        stop,
        text: String::from_utf8(out).expect("utf-8"),
        calls,
        case_dir,
        tree,
    }
}

/// The commit of the case's branch that added `path`.
fn added(tree: &Path, path: &str) -> String {
    git(
        tree,
        &["log", "--format=%H", "--diff-filter=A", "HEAD", "--", path],
    )
}

/// Acceptance 1: with no `--protocol`, tests first, then the implementation, then the hand-over
/// reaches `pr.open` on the fake forge; the case holds one `tests_change` with no result at
/// the tests revision and one `paths_changed` `both` at the handed-over revision.
#[test]
fn tests_first_on_the_default_protocol_reaches_pr_open() {
    let harness = TestsFirst {
        implements: true,
        calls: Cell::new(0),
    };
    let run = delivered("both", None, true, &harness);
    let text = &run.text;
    let changes = records(&run.case_dir, "tests_change");
    let paths = records(&run.case_dir, "paths_changed");
    assert_eq!(
        changes.len(),
        1,
        "tests_change records: {changes:?}\n{text}"
    );
    assert_eq!(paths.len(), 1, "paths_changed records: {paths:?}\n{text}");
    assert_eq!(
        run.stop,
        Ok(Stop::ApprovalRequired(vec!["repository.merge".to_owned()])),
        "{text}"
    );
    let tests_revision = added(&run.tree, TESTS);
    assert!(
        !tests_revision.is_empty(),
        "no commit added {TESTS}: {text}"
    );
    assert_eq!(changes[0]["subject"], "implementation", "{changes:?}");
    assert_eq!(
        changes[0]["subject_revision"],
        tests_revision.as_str(),
        "the tests_change is not at the revision tests.write committed: {text}"
    );
    assert!(
        changes[0]
            .get("result")
            .is_none_or(serde_json::Value::is_null),
        "a tests_change has no result: {changes:?}"
    );
    assert!(
        text.contains("evidence: tests_change -"),
        "the record is not printed: {text}"
    );
    let handovers = records(&run.case_dir, "handover");
    assert_eq!(handovers.len(), 1, "{handovers:?}\n{text}");
    assert_eq!(paths[0]["result"], "both", "{paths:?}\n{text}");
    assert_eq!(
        paths[0]["subject_revision"], handovers[0]["subject_revision"],
        "the paths_changed is not at the handed-over revision: {text}"
    );
    let pulls = records(&run.case_dir, "pull_request");
    assert_eq!(pulls.len(), 1, "{pulls:?}\n{text}");
    assert_eq!(
        run.calls.iter().filter(|c| c.starts_with("open ")).count(),
        1,
        "{:?}\n{text}",
        run.calls
    );
}

/// Acceptance 2: a change of tests only is `tests_only`, and `pr.open` stays blocked: nothing
/// is pushed or opened.
#[test]
fn a_change_of_tests_only_is_tests_only_and_pr_open_stays_blocked() {
    let harness = TestsFirst {
        implements: false,
        calls: Cell::new(0),
    };
    let run = delivered("tests-only", None, false, &harness);
    let text = &run.text;
    assert!(run.stop.is_ok(), "{:?}\n{text}", run.stop);
    eprintln!("a change of tests only stops {:?}", run.stop);
    let changes = records(&run.case_dir, "tests_change");
    assert_eq!(
        changes.len(),
        1,
        "tests_change records: {changes:?}\n{text}"
    );
    let paths = records(&run.case_dir, "paths_changed");
    assert_eq!(paths.len(), 1, "paths_changed records: {paths:?}\n{text}");
    assert_eq!(paths[0]["result"], "tests_only", "{paths:?}\n{text}");
    assert!(
        records(&run.case_dir, "pull_request").is_empty(),
        "a pull request for tests only: {text}"
    );
    assert!(
        !run.calls
            .iter()
            .any(|c| c.starts_with("open ") || c.starts_with("push ")),
        "{:?}\n{text}",
        run.calls
    );
    assert!(text.contains("pr.open (blocked)"), "{text}");
}

/// A protocol that declares neither kind (`story.delivery/1`) gets neither record: its case's
/// records are what they were before the writers.
#[test]
fn a_protocol_that_declares_neither_kind_gets_neither_record() {
    let harness = TestsFirst {
        implements: true,
        calls: Cell::new(0),
    };
    let run = delivered("delivery-1", Some("story.delivery/1"), true, &harness);
    let text = &run.text;
    assert_eq!(
        run.stop,
        Ok(Stop::ApprovalRequired(vec!["repository.merge".to_owned()])),
        "{text}"
    );
    assert_eq!(records(&run.case_dir, "change_record").len(), 1, "{text}");
    assert!(
        records(&run.case_dir, "tests_change").is_empty(),
        "story.delivery/1 declares no tests_change: {text}"
    );
    assert!(
        records(&run.case_dir, "paths_changed").is_empty(),
        "story.delivery/1 declares no paths_changed: {text}"
    );
}
