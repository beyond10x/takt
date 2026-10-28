//! story:red-run-is-no-repeated-failure: under `tests_first = required` (`story.delivery/2`) a
//! run writes the story's tests, runs them red on purpose, then edits the implementation. That
//! red run, a failing `tests.run` at the revision a `tests.write` step committed (the one its
//! `tests_change` record is about), does not count toward `REPEATED_FAILURES`: the count starts
//! at the first `tests.run` after an implementation edit. Two implementation edits that each
//! leave the same tests failing still stop the run `RepeatedTestFailure`, and a protocol whose
//! `tests.write` produces no `tests_change` (`story.delivery/1`) keeps the count it had.

#[path = "support/own_home.rs"]
mod own_home;

use std::cell::{Cell, RefCell};
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

/// What the implementation holds once it makes the story's test pass.
const FIXED: &str = "fixed\n";

/// The one test the story's tests add, as a failing summary names it.
const NEW_TEST: &str = "remove::a_todo_is_removed_by_its_number";

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
        "repeated-failure-counts-after-red-{}-{name}",
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

/// The text of `path` in the tree of `revision`, none when it is not there.
fn at(tree: &Path, revision: &str, path: &str) -> Option<String> {
    let output = Command::new("git")
        .args(["show", &format!("{revision}:{path}")])
        .current_dir(tree)
        .output()
        .expect("git runs");
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

/// A validator of `action` that observes `pass` at every revision, but for `tests.run` `fail`,
/// naming [`NEW_TEST`] as `cargo test` does through takt's summary, at a revision that holds the
/// story's tests and an implementation that is not [`FIXED`].
struct Scripted {
    action: &'static str,
    kind: &'static str,
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
        let fails = self.action == "tests.run"
            && at(tree, revision, TESTS).is_some()
            && at(tree, revision, IMPLEMENTATION).as_deref() != Some(FIXED);
        let (result, summary) = if fails {
            (
                "fail",
                format!(
                    "the test command exited 101: fail; 0 passed, 1 failed, 0 ignored over 1 result line; to rerun pass `--test remove`; failed: {NEW_TEST}"
                ),
            )
        } else {
            ("pass", "the test command exited 0: pass".to_owned())
        };
        Ok(Observation {
            summary,
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

/// A developer that writes the story's tests first, then edits the implementation each time
/// `implementation.edit` is offered: a new attempt that leaves the tests failing, until its
/// `fixes_on`-th edit, which writes [`FIXED`] (never, when none). With the implementation fixed
/// it hands the change over and requests the merge when that is offered; a step with nothing of
/// that offered takes the first action offered, changing nothing. `edits` counts its
/// implementation edits; `writes_tests` is false for a developer that only edits.
struct Developer {
    writes_tests: bool,
    fixes_on: Option<usize>,
    edits: Cell<usize>,
    actions: RefCell<Vec<String>>,
}

impl Developer {
    fn new(writes_tests: bool, fixes_on: Option<usize>) -> Self {
        Developer {
            writes_tests,
            fixes_on,
            edits: Cell::new(0),
            actions: RefCell::new(Vec::new()),
        }
    }
}

impl Harness for Developer {
    fn name(&self) -> &str {
        "developer"
    }

    fn step(&self, context: &StepContext<'_>) -> anyhow::Result<StepReport> {
        let offered = |action: &str| context.offered.iter().any(|offer| offer == action);
        let write = |path: &str, text: &str, action: &str| -> anyhow::Result<StepReport> {
            let file = context.tree.join(path);
            fs::create_dir_all(file.parent().expect("a parent"))?;
            fs::write(&file, text)?;
            self.actions.borrow_mut().push(action.to_owned());
            Ok(StepReport {
                action: action.to_owned(),
                summary: format!("wrote {path}"),
                tokens: Some(100),
            })
        };
        if self.writes_tests && !context.tree.join(TESTS).exists() && offered("tests.write") {
            return write(TESTS, "the story's test\n", "tests.write");
        }
        let fixed = fs::read_to_string(context.tree.join(IMPLEMENTATION))
            .ok()
            .as_deref()
            == Some(FIXED);
        if !fixed && offered("implementation.edit") {
            let edit = self.edits.get() + 1;
            self.edits.set(edit);
            let text = if self.fixes_on == Some(edit) {
                FIXED.to_owned()
            } else {
                format!("attempt {edit}\n")
            };
            return write(IMPLEMENTATION, &text, "implementation.edit");
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
        self.actions.borrow_mut().push(action.to_owned());
        Ok(StepReport {
            action: action.to_owned(),
            summary: "the change is complete".to_owned(),
            tokens: Some(100),
        })
    }
}

/// The validators of every run here: the tests by [`Scripted`], the gate green, `pr.open` and
/// `ci.read` on `forge`, and `change.read`.
fn validators(forge: &Rc<dyn Forge>) -> Vec<Box<dyn Validator>> {
    vec![
        Box::new(Scripted {
            action: "tests.run",
            kind: "test_result",
        }),
        Box::new(Scripted {
            action: "gate.run",
            kind: "gate_result",
        }),
        Box::new(PrOpen::new(forge.clone())),
        Box::new(CiRead::new(forge.clone())),
        Box::new(ChangeRead),
    ]
}

/// A fake forge whose checks pass.
fn forge() -> Rc<dyn Forge> {
    Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    })
}

/// The options of a delivery run of the story on `protocol`, or on the default protocol for a
/// story with its default inputs when `None` (`story.delivery/2`, `tests_first: required`), as
/// `takt run` with no `--protocol` and no `--input` opens it, with `iterations` steps.
fn options(name: &str, protocol: Option<&str>, iterations: u64) -> RunOptions {
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
            assert!(
                settings
                    .pairs()
                    .iter()
                    .any(|(name, value)| name == "tests_first" && value == "required"),
                "{:?}",
                settings.pairs()
            );
            (settings.spec.clone(), settings.pairs())
        }
    };
    RunOptions {
        protocol,
        intent: "remove a todo by its number".to_owned(),
        inputs,
        iterations: Some(iterations),
        hours: None,
        tokens: None,
        case_id: Some(name.to_owned()),
        gate: Some("repository.merge".to_owned()),
    }
}

/// A run's stop and what it printed.
fn run(run: &mut Run<'_>) -> (Result<Stop, String>, String) {
    let mut out = Vec::new();
    let stop = run.run(&mut out).map_err(|error| format!("{error:#}"));
    (stop, String::from_utf8(out).expect("utf-8"))
}

/// The stop `RepeatedTestFailure` the story's one test gives after `count` results in a row.
fn repeated(count: u64) -> Stop {
    Stop::RepeatedTestFailure {
        tests: vec![NEW_TEST.to_owned()],
        count,
    }
}

/// Acceptance 1 and 4: the tests are written and shown red, the first implementation edit leaves
/// them red, and the run does not stop `RepeatedTestFailure`: it offers `implementation.edit`
/// again, and the second edit, which fixes the implementation, carries the run on to the merge.
#[test]
fn a_first_edit_that_leaves_the_red_test_failing_is_not_a_repeat() {
    let tree = scratch("first-edit");
    let forge = forge();
    let validators = validators(&forge);
    let harness = Developer::new(true, Some(2));
    let mut opened = Run::open(
        &tree,
        options("first-edit", None, 40),
        &harness,
        &validators,
    )
    .expect("the case opens");
    let (stop, text) = run(&mut opened);
    let actions = harness.actions.borrow().clone();
    assert_ne!(stop, Ok(repeated(2)), "{actions:?}\n{text}");
    assert_eq!(
        harness.edits.get(),
        2,
        "implementation.edit was not offered again after the first edit left the test red: {stop:?} {actions:?}\n{text}"
    );
    assert_eq!(
        stop,
        Ok(Stop::ApprovalRequired(vec!["repository.merge".to_owned()])),
        "{actions:?}\n{text}"
    );
}

/// Acceptance 1 across a resume: a run that stopped after the red run, before any edit, is
/// resumed; the first edit of the resumed run leaves the test red and the run offers
/// `implementation.edit` again: the red run the case's record holds counts no more than one the
/// run itself observed.
#[test]
fn a_red_run_before_a_resume_is_not_counted_by_the_resumed_run() {
    let tree = scratch("resumed");
    let forge = forge();
    let validators = validators(&forge);
    let writer = Developer::new(true, None);
    // Steps until the red run: the tests at the base, tests.write, the tests again.
    let mut first = Run::open(&tree, options("resumed", None, 3), &writer, &validators)
        .expect("the case opens");
    let (stop, text) = run(&mut first);
    let case_id = first.case.id.clone();
    drop(first);
    assert_eq!(stop, Ok(Stop::Iterations(3)), "{text}");
    assert_eq!(
        *writer.actions.borrow(),
        vec!["tests.write".to_owned()],
        "the first run edits nothing: {text}"
    );
    assert_eq!(
        text.matches("evidence: test_result fail").count(),
        1,
        "the first run ends on the red run: {text}"
    );
    let harness = Developer::new(true, Some(2));
    let mut resumed = Run::resume(
        &tree,
        options("resumed", None, 40),
        &case_id,
        &harness,
        &validators,
    )
    .expect("the case resumes");
    let (stop, text) = run(&mut resumed);
    let actions = harness.actions.borrow().clone();
    assert_eq!(
        harness.edits.get(),
        2,
        "implementation.edit was not offered again after the resumed run's first edit: {stop:?} {actions:?}\n{text}"
    );
    assert_eq!(
        stop,
        Ok(Stop::ApprovalRequired(vec!["repository.merge".to_owned()])),
        "{actions:?}\n{text}"
    );
}

/// Acceptance 2: two implementation edits in a row that each leave the same test failing stop
/// the run `RepeatedTestFailure` after the second, with no third edit.
#[test]
fn two_edits_that_leave_the_same_test_failing_stop_the_run() {
    let tree = scratch("two-edits");
    let forge = forge();
    let validators = validators(&forge);
    let harness = Developer::new(true, None);
    let mut opened = Run::open(&tree, options("two-edits", None, 40), &harness, &validators)
        .expect("the case opens");
    let (stop, text) = run(&mut opened);
    let actions = harness.actions.borrow().clone();
    assert_eq!(stop, Ok(repeated(2)), "{actions:?}\n{text}");
    assert_eq!(harness.edits.get(), 2, "{actions:?}\n{text}");
}

/// Acceptance 3: on `story.delivery/1`, whose `tests.write` produces no `tests_change`, the
/// failing run after the tests are written counts as it did: the first edit that leaves the same
/// test failing stops the run `RepeatedTestFailure` (2), with no second edit.
#[test]
fn a_protocol_without_tests_first_keeps_the_count() {
    let tree = scratch("delivery-1");
    let forge = forge();
    let validators = validators(&forge);
    let harness = Developer::new(true, None);
    let mut opened = Run::open(
        &tree,
        options("delivery-1", Some("story.delivery/1"), 40),
        &harness,
        &validators,
    )
    .expect("the case opens");
    let (stop, text) = run(&mut opened);
    let actions = harness.actions.borrow().clone();
    assert_eq!(
        actions.first().map(String::as_str),
        Some("tests.write"),
        "{text}"
    );
    assert_eq!(stop, Ok(repeated(2)), "{actions:?}\n{text}");
    assert_eq!(harness.edits.get(), 1, "{actions:?}\n{text}");
}
