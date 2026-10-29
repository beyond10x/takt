//! Adversary, wave 42: the count of `REPEATED_FAILURES` after U6 (the red run after
//! `tests.write` is not counted) and U9 (a revision that changes documentation only is not
//! counted).
//!
//! - U6's commit: "alternating writes and edits cannot loop forever". A harness that, after each
//!   implementation edit that leaves the tests red, writes one more test and edits again never
//!   meets two counted results of one set; before U6 the run stopped after its first edit.
//! - U9's rule: `is_documentation_path` is "documentation the tests cannot depend on". A test
//!   that reads a file under `docs/` (as `crates/takt-cli/tests/cases_under_dot_takt.rs` reads
//!   `docs/runs/…`) is decided by edits there, and attempts made there are never counted.

#[path = "support/own_home.rs"]
mod own_home;

use std::cell::RefCell;
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
/// A file under `docs/` the story's tests read: what they decide is in it.
const DOCS_FIXTURE: &str = "docs/fixture.txt";
const FIXED: &str = "fixed\n";

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
        "w42-adv-repeat-count-{}-{name}",
        std::process::id()
    ));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(dir.join("docs")).expect("scratch dir");
    fs::write(dir.join("README.md"), "a story\n").expect("a file");
    fs::write(dir.join(DOCS_FIXTURE), "broken\n").expect("a file");
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "start"]);
    Scratch(dir)
}

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

/// Where the tests' outcome is decided: the implementation file, or a file under `docs/`.
#[derive(Clone, Copy)]
enum Decided {
    Implementation,
    Docs,
}

/// `tests.run` fails at a revision holding the tests file while what decides them is not
/// [`FIXED`], naming each line of the tests file as a failing test; every other action passes.
struct Scripted {
    action: &'static str,
    kind: &'static str,
    decided: Decided,
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
        let decider = match self.decided {
            Decided::Implementation => IMPLEMENTATION,
            Decided::Docs => DOCS_FIXTURE,
        };
        let tests = at(tree, revision, TESTS);
        let fails = self.action == "tests.run"
            && tests.is_some()
            && at(tree, revision, decider).as_deref() != Some(FIXED);
        let (result, summary) = if fails {
            let names: Vec<String> = tests
                .unwrap_or_default()
                .lines()
                .map(str::to_owned)
                .collect();
            (
                "fail",
                format!(
                    "the test command exited 101: fail; 0 passed, {} failed, 0 ignored over {} result lines; to rerun pass `--test remove`; failed: {}",
                    names.len(),
                    names.len(),
                    names.join(", ")
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

/// How a [`Developer`] works after its first test is written.
#[derive(Clone, Copy)]
enum Style {
    /// After each implementation edit, writes one more test, then edits again.
    AddsATestAfterEachEdit,
    /// Each implementation edit is a new attempt in [`DOCS_FIXTURE`], never the fix.
    EditsTheDocsFixture,
}

struct Developer {
    style: Style,
    actions: RefCell<Vec<String>>,
}

impl Developer {
    fn new(style: Style) -> Self {
        Self {
            style,
            actions: RefCell::new(Vec::new()),
        }
    }

    fn edits(&self) -> usize {
        self.actions
            .borrow()
            .iter()
            .filter(|action| *action == "implementation.edit")
            .count()
    }
}

impl Harness for Developer {
    fn name(&self) -> &str {
        "developer"
    }

    fn step(&self, context: &StepContext<'_>) -> anyhow::Result<StepReport> {
        let offered = |action: &str| context.offered.iter().any(|offer| offer == action);
        let last = self.actions.borrow().last().cloned();
        let write = |path: &str, text: String, action: &str| -> anyhow::Result<StepReport> {
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
        let tests = fs::read_to_string(context.tree.join(TESTS)).ok();
        if tests.is_none() && offered("tests.write") {
            return write(TESTS, "remove::t1\n".to_owned(), "tests.write");
        }
        let n = self.edits() + 1;
        match self.style {
            Style::AddsATestAfterEachEdit => {
                if last.as_deref() == Some("implementation.edit") && offered("tests.write") {
                    let mut text = tests.unwrap_or_default();
                    text.push_str(&format!("remove::t{}\n", text.lines().count() + 1));
                    return write(TESTS, text, "tests.write");
                }
                if offered("implementation.edit") {
                    return write(
                        IMPLEMENTATION,
                        format!("attempt {n}\n"),
                        "implementation.edit",
                    );
                }
            }
            Style::EditsTheDocsFixture => {
                if offered("implementation.edit") {
                    return write(
                        DOCS_FIXTURE,
                        format!("attempt {n}\n"),
                        "implementation.edit",
                    );
                }
            }
        }
        let action = context
            .offered
            .first()
            .map_or("repository.inspect", String::as_str)
            .to_owned();
        self.actions.borrow_mut().push(action.clone());
        Ok(StepReport {
            action,
            summary: "nothing to change".to_owned(),
            tokens: Some(100),
        })
    }
}

fn validators(forge: &Rc<dyn Forge>, decided: Decided) -> Vec<Box<dyn Validator>> {
    vec![
        Box::new(Scripted {
            action: "tests.run",
            kind: "test_result",
            decided,
        }),
        Box::new(Scripted {
            action: "gate.run",
            kind: "gate_result",
            decided,
        }),
        Box::new(PrOpen::new(forge.clone())),
        Box::new(CiRead::new(forge.clone())),
        Box::new(ChangeRead),
    ]
}

/// A delivery run on the default protocol for a story (`story.delivery/2`, tests first).
fn options(name: &str, iterations: u64) -> RunOptions {
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
    RunOptions {
        protocol: settings.spec.clone(),
        intent: "remove a todo by its number".to_owned(),
        inputs: settings.pairs(),
        iterations: Some(iterations),
        hours: None,
        tokens: None,
        case_id: Some(name.to_owned()),
        gate: Some("repository.merge".to_owned()),
    }
}

fn delivered(
    name: &str,
    style: Style,
    decided: Decided,
) -> (Result<Stop, String>, String, Vec<String>) {
    let tree = scratch(name);
    let forge: Rc<dyn Forge> = Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    });
    let validators = validators(&forge, decided);
    let harness = Developer::new(style);
    let mut run =
        Run::open(&tree, options(name, 30), &harness, &validators).expect("the case opens");
    let mut out = Vec::new();
    let stop = run.run(&mut out).map_err(|error| format!("{error:#}"));
    let actions = harness.actions.borrow().clone();
    (stop, String::from_utf8(out).expect("utf-8"), actions)
}

/// U6's claim: alternating `tests.write` and `implementation.edit` cannot loop forever. Here
/// every implementation edit leaves every test red and the harness answers each with one more
/// test; the run should stop `RepeatedTestFailure` within its 30 steps, as it did before U6
/// after the first edit. It spends the whole step budget instead.
#[test]
fn alternating_a_new_test_and_an_edit_that_fixes_nothing_stops_the_run() {
    let (stop, text, actions) = delivered(
        "adds-a-test",
        Style::AddsATestAfterEachEdit,
        Decided::Implementation,
    );
    assert!(
        matches!(stop, Ok(Stop::RepeatedTestFailure { .. })),
        "stop {stop:?} after {} implementation edits, none of which fixed anything: {actions:?}\n{text}",
        actions
            .iter()
            .filter(|a| *a == "implementation.edit")
            .count()
    );
}

/// U9: the story's tests are decided by a file under `docs/`. Each implementation edit is a new
/// attempt there, and each leaves the same test failing: two of them in a row are what
/// `REPEATED_FAILURES` stops, as two code edits are. None is counted, and the run spends its
/// whole step budget.
#[test]
#[ignore = "story:docs-edits-the-tests-read-are-attempts: edits under docs/ that the tests read are never counted as attempts"]
fn attempts_in_a_docs_file_the_tests_read_are_counted() {
    let (stop, text, actions) =
        delivered("docs-fixture", Style::EditsTheDocsFixture, Decided::Docs);
    assert!(
        matches!(stop, Ok(Stop::RepeatedTestFailure { .. })),
        "stop {stop:?} after {} attempts in {DOCS_FIXTURE}, each leaving the test red: {actions:?}\n{text}",
        actions
            .iter()
            .filter(|a| *a == "implementation.edit")
            .count()
    );
}
