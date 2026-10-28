//! Adversary cases for story:story-delivery-2-evidence-writers: the `tests_change` record of a
//! `tests.write` commit and the `paths_changed` record of `change.read`, on `story.delivery/2`,
//! driven against what the protocol says the two kinds mean. `tests_change` is "the record that
//! the story's tests were written"; `change.complete` holds on "a complete change, with test
//! paths and implementation paths both changed", the story's tests written and the
//! implementation done.

use std::cell::Cell;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

use takt::evidence::Record;
use takt::forge::{Checks, Forge, Memory};
use takt::harness::{Harness, StepContext, StepReport};
use takt::validator::{ChangeRead, CiRead, Observation, PrOpen, Validator};
use takt::{Run, RunOptions, Stop};

/// A test the base branch already holds, and the story's implementation file.
const OLD_TESTS: &str = "tests/old_test.txt";
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

/// A scratch git tree under this crate's build directory, removed when dropped.
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

/// A tree whose one commit on `main` holds a README and the test [`OLD_TESTS`].
fn scratch(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "story-delivery-2-writers-adv1-{}-{name}",
        std::process::id()
    ));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(dir.join("tests")).expect("scratch dir");
    fs::write(dir.join("README.md"), "a story\n").expect("a file");
    fs::write(dir.join(OLD_TESTS), "an existing test\n").expect("a test");
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

/// A validator of `action` that observes `pass`, but `fail` for `tests.run` at a revision
/// that holds neither the implementation nor [`OLD_TESTS`]: the tests left once that test is
/// gone fail until the story is implemented.
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
        let fails =
            self.red && !at(tree, revision, IMPLEMENTATION) && !at(tree, revision, OLD_TESTS);
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

/// What the developer does with the steps it is offered.
#[derive(Clone, Copy)]
enum Plan {
    /// `tests.write` deletes [`OLD_TESTS`] and writes nothing; then `implementation.edit`
    /// writes the implementation.
    DeleteTestThenImplement,
    /// `implementation.edit` deletes [`OLD_TESTS`] and writes the implementation, in one step;
    /// `tests.write` is never taken.
    ImplementDeletingTest,
    /// `implementation.edit` writes the implementation only; `tests.write` is never taken.
    ImplementOnly,
    /// `tests.write` is taken while offered, changing nothing.
    EmptyTestsWrite,
}

struct Developer {
    plan: Plan,
    calls: Cell<usize>,
}

impl Harness for Developer {
    fn name(&self) -> &str {
        "adversary-developer"
    }

    fn step(&self, context: &StepContext<'_>) -> anyhow::Result<StepReport> {
        self.calls.set(self.calls.get() + 1);
        let offered = |action: &str| context.offered.iter().any(|offer| offer == action);
        let report = |action: &str, summary: &str| StepReport {
            action: action.to_owned(),
            summary: summary.to_owned(),
            tokens: Some(100),
        };
        let old = context.tree.join(OLD_TESTS);
        let implementation = context.tree.join(IMPLEMENTATION);
        let implement = || -> anyhow::Result<()> {
            fs::create_dir_all(implementation.parent().expect("a parent"))?;
            fs::write(&implementation, "implementation.edit\n")?;
            Ok(())
        };
        match self.plan {
            Plan::DeleteTestThenImplement => {
                if old.exists() && offered("tests.write") {
                    fs::remove_file(&old)?;
                    return Ok(report("tests.write", "deleted the failing test"));
                }
                if !implementation.exists() && offered("implementation.edit") {
                    implement()?;
                    return Ok(report("implementation.edit", "wrote the implementation"));
                }
            }
            Plan::ImplementDeletingTest => {
                if !implementation.exists() && offered("implementation.edit") {
                    if old.exists() {
                        fs::remove_file(&old)?;
                    }
                    implement()?;
                    return Ok(report(
                        "implementation.edit",
                        "wrote the implementation and deleted the failing test",
                    ));
                }
            }
            Plan::ImplementOnly => {
                if !implementation.exists() && offered("implementation.edit") {
                    implement()?;
                    return Ok(report("implementation.edit", "wrote the implementation"));
                }
            }
            Plan::EmptyTestsWrite => {
                if offered("tests.write") {
                    return Ok(report("tests.write", "the tests are as they should be"));
                }
            }
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
        Ok(report(action, "the change is complete"))
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

struct Delivered {
    stop: Result<Stop, String>,
    text: String,
    calls: Vec<String>,
    case_dir: PathBuf,
    tree: Scratch,
}

/// The commit of the case's branch that deleted `path`, empty when none did.
fn deleted(tree: &Path, path: &str) -> String {
    git(
        tree,
        &["log", "--format=%H", "--diff-filter=D", "HEAD", "--", path],
    )
}

/// A run of the story on `story.delivery/2` with `tests_first` as given and the gate after the
/// handover: the tests red until the implementation is there, the gate green, `pr.open` and
/// `ci.read` on a fake forge whose checks pass, and `change.read`.
fn delivered(name: &str, tests_first: &str, plan: Plan) -> Delivered {
    let tree = scratch(name);
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
        protocol: "story.delivery/2".to_owned(),
        intent: "remove a todo by its number".to_owned(),
        inputs: vec![
            ("tests_first".to_owned(), tests_first.to_owned()),
            ("gate_on".to_owned(), "after_handover".to_owned()),
        ],
        iterations: Some(40),
        hours: None,
        tokens: None,
        case_id: Some(name.to_owned()),
        gate: Some("repository.merge".to_owned()),
    };
    let harness = Developer {
        plan,
        calls: Cell::new(0),
    };
    let mut run = Run::open(&tree, options, &harness, &validators).expect("the case opens");
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

/// A `tests.write` step whose only change deletes an existing test writes no test: the
/// protocol's `tests_change` is "the record that the story's tests were written", and under
/// `tests_first: required` it is what admits `implementation.edit`.
#[test]
fn a_tests_write_that_only_deletes_a_test_records_no_tests_change() {
    let run = delivered("delete-only", "required", Plan::DeleteTestThenImplement);
    let text = &run.text;
    assert!(
        !deleted(&run.tree, OLD_TESTS).is_empty() && text.contains(": tests.write"),
        "the scenario is not reached: no tests.write deleted {OLD_TESTS}\n{text}"
    );
    let changes = records(&run.case_dir, "tests_change");
    let pulls = records(&run.case_dir, "pull_request");
    assert!(
        changes.is_empty(),
        "story:story-delivery-2-evidence-writers: a tests.write that only deleted {OLD_TESTS} \
         was recorded as the story's tests written ({} tests_change, {} pull request(s), stop \
         {:?}): {changes:?}\n{text}",
        changes.len(),
        pulls.len(),
        run.stop
    );
}

/// A revision whose only change to a test path deletes an existing test is no complete change:
/// the story's tests are not written, so `pr.open` stays blocked.
#[test]
fn a_change_whose_only_test_change_deletes_a_test_opens_no_pull_request() {
    let run = delivered("delete-in-edit", "optional", Plan::ImplementDeletingTest);
    let text = &run.text;
    assert!(
        !deleted(&run.tree, OLD_TESTS).is_empty()
            && records(&run.case_dir, "handover").len() == 1
            && records(&run.case_dir, "change_record").len() == 1,
        "the scenario is not reached: no handed-over revision that deleted {OLD_TESTS} was \
         read, stop {:?}\n{text}",
        run.stop
    );
    let paths = records(&run.case_dir, "paths_changed");
    let pulls = records(&run.case_dir, "pull_request");
    assert!(
        pulls.is_empty() && paths.iter().all(|record| record["result"] != "both"),
        "story:story-delivery-2-evidence-writers: a revision that deleted {OLD_TESTS} and wrote \
         no test is paths_changed {:?} and opened {} pull request(s) ({:?}), stop {:?}\n{text}",
        paths
            .iter()
            .map(|record| record["result"].clone())
            .collect::<Vec<_>>(),
        pulls.len(),
        run.calls,
        run.stop
    );
}

/// A change of the implementation only is `implementation_only`, and `pr.open` stays blocked:
/// the arm of `paths_changed` no case of the unit reaches.
#[test]
fn a_change_of_the_implementation_only_is_implementation_only() {
    let run = delivered("implementation-only", "optional", Plan::ImplementOnly);
    let text = &run.text;
    assert!(run.stop.is_ok(), "{:?}\n{text}", run.stop);
    let paths = records(&run.case_dir, "paths_changed");
    assert_eq!(
        paths.len(),
        1,
        "story:story-delivery-2-evidence-writers: paths_changed records: {paths:?}, stop {:?}\n{text}",
        run.stop
    );
    assert_eq!(
        paths[0]["result"], "implementation_only",
        "story:story-delivery-2-evidence-writers: {paths:?}\n{text}"
    );
    assert!(
        records(&run.case_dir, "tests_change").is_empty(),
        "story:story-delivery-2-evidence-writers: no tests.write was taken: {text}"
    );
    assert!(
        records(&run.case_dir, "pull_request").is_empty()
            && !run
                .calls
                .iter()
                .any(|c| c.starts_with("open ") || c.starts_with("push ")),
        "story:story-delivery-2-evidence-writers: a pull request for the implementation only: \
         {:?}\n{text}",
        run.calls
    );
}

/// A `tests.write` step that changes no file commits nothing, so takt records no
/// `tests_change`, and `implementation.edit` stays blocked under `tests_first: required`.
#[test]
fn a_tests_write_that_changes_nothing_records_no_tests_change() {
    let run = delivered("empty-tests-write", "required", Plan::EmptyTestsWrite);
    let text = &run.text;
    assert!(
        text.contains(": tests.write") && text.contains("effect: no file changed"),
        "the scenario is not reached: no tests.write was taken with no file changed\n{text}"
    );
    let changes = records(&run.case_dir, "tests_change");
    assert!(
        changes.is_empty(),
        "story:story-delivery-2-evidence-writers: a tests.write with no file changed was \
         recorded: {changes:?}\n{text}"
    );
    assert!(
        records(&run.case_dir, "pull_request").is_empty(),
        "story:story-delivery-2-evidence-writers: {text}"
    );
}
