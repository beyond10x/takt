//! story:a-revision-without-code-is-no-new-attempt: a failing `tests.run` counts toward
//! `REPEATED_FAILURES` only when the revision it ran at changed, since the last failing result
//! the row counted, a path the tests can depend on: one outside `docs/**`, `README.md` and
//! `CHANGELOG.md`. A revision that changes documentation only is no new attempt and leaves the
//! row as it was. Two code edits that leave the same tests failing still stop the run
//! `RepeatedTestFailure`, and a case stopped on it and resumed gets at least one implementation
//! edit before the count can stop it again.

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
        "repeated-failure-counts-code-{}-{name}",
        std::process::id()
    ));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(dir.join("docs")).expect("scratch dir");
    fs::write(dir.join("README.md"), "a story\n").expect("a file");
    fs::write(dir.join("CHANGELOG.md"), "# Changelog\n").expect("a file");
    fs::write(dir.join("docs/notes.md"), "notes\n").expect("a file");
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

/// What one `implementation.edit` of a [`Developer`] writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Edit {
    /// A new attempt at the implementation that leaves the tests failing.
    Code,
    /// A line in `docs/notes.md`, nothing else.
    Docs,
    /// A line in `README.md`, nothing else.
    Readme,
    /// A line in `CHANGELOG.md`, nothing else.
    Changelog,
    /// A new attempt at the implementation, and a line in `docs/notes.md` beside it.
    CodeAndDocs,
    /// The implementation that makes the tests pass.
    Fix,
}

/// A developer that writes the story's tests first, then takes `implementation.edit` each time it
/// is offered while the implementation is not [`FIXED`], writing what `plan` names for that edit
/// ([`Edit::Code`] past its end). With the implementation fixed it hands the change over and
/// requests the merge when that is offered; a step with nothing of that offered takes the first
/// action offered, changing nothing. `edits` holds the edits it made, in order.
struct Developer {
    plan: Vec<Edit>,
    edits: RefCell<Vec<Edit>>,
}

impl Developer {
    fn new(plan: &[Edit]) -> Self {
        Developer {
            plan: plan.to_vec(),
            edits: RefCell::new(Vec::new()),
        }
    }

    fn edits(&self) -> Vec<Edit> {
        self.edits.borrow().clone()
    }
}

/// Appends `line` to the file `path` under `tree`.
fn append(tree: &Path, path: &str, line: &str) -> anyhow::Result<()> {
    let file = tree.join(path);
    fs::create_dir_all(file.parent().expect("a parent"))?;
    let mut text = fs::read_to_string(&file).unwrap_or_default();
    text.push_str(line);
    text.push('\n');
    fs::write(&file, text)?;
    Ok(())
}

impl Harness for Developer {
    fn name(&self) -> &str {
        "developer"
    }

    fn step(&self, context: &StepContext<'_>) -> anyhow::Result<StepReport> {
        let offered = |action: &str| context.offered.iter().any(|offer| offer == action);
        let report = |action: &str, summary: String| StepReport {
            action: action.to_owned(),
            summary,
            tokens: Some(100),
        };
        if !context.tree.join(TESTS).exists() && offered("tests.write") {
            append(context.tree, TESTS, "the story's test")?;
            return Ok(report("tests.write", format!("wrote {TESTS}")));
        }
        let fixed = fs::read_to_string(context.tree.join(IMPLEMENTATION))
            .ok()
            .as_deref()
            == Some(FIXED);
        if !fixed && offered("implementation.edit") {
            let n = self.edits.borrow().len();
            let edit = self.plan.get(n).copied().unwrap_or(Edit::Code);
            self.edits.borrow_mut().push(edit);
            let attempt = format!("attempt {}", n + 1);
            match edit {
                Edit::Code => {
                    fs::create_dir_all(context.tree.join("src"))?;
                    fs::write(context.tree.join(IMPLEMENTATION), format!("{attempt}\n"))?;
                }
                Edit::Docs => append(context.tree, "docs/notes.md", &attempt)?,
                Edit::Readme => append(context.tree, "README.md", &attempt)?,
                Edit::Changelog => append(context.tree, "CHANGELOG.md", &attempt)?,
                Edit::CodeAndDocs => {
                    fs::create_dir_all(context.tree.join("src"))?;
                    fs::write(context.tree.join(IMPLEMENTATION), format!("{attempt}\n"))?;
                    append(context.tree, "docs/notes.md", &attempt)?;
                }
                Edit::Fix => {
                    fs::create_dir_all(context.tree.join("src"))?;
                    fs::write(context.tree.join(IMPLEMENTATION), FIXED)?;
                }
            }
            return Ok(report("implementation.edit", format!("{edit:?}")));
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
        Ok(report(action, "the change is complete".to_owned()))
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

/// The options of a delivery run of the story on the default protocol for a story with its
/// default inputs (`story.delivery/2`, `tests_first: required`), as `takt run` with no
/// `--protocol` and no `--input` opens it, with `iterations` steps.
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

/// A run's stop and what it printed.
fn run(run: &mut Run<'_>) -> (Result<Stop, String>, String) {
    let mut out = Vec::new();
    let stop = run.run(&mut out).map_err(|error| format!("{error:#}"));
    (stop, String::from_utf8(out).expect("utf-8"))
}

/// A run of the story in `tree` under the case `name` with `harness`: its stop, what it printed
/// and the case's id.
fn delivered(
    tree: &Path,
    name: &str,
    harness: &Developer,
) -> (Result<Stop, String>, String, String) {
    let forge = forge();
    let validators = validators(&forge);
    let mut opened =
        Run::open(tree, options(name, 40), harness, &validators).expect("the case opens");
    let (stop, text) = run(&mut opened);
    let id = opened.case.id.clone();
    (stop, text, id)
}

/// The case `id` in `tree` resumed with `harness`: its stop and what it printed.
fn resumed(
    tree: &Path,
    name: &str,
    id: &str,
    harness: &Developer,
) -> (Result<Stop, String>, String) {
    let forge = forge();
    let validators = validators(&forge);
    let mut resumed =
        Run::resume(tree, options(name, 40), id, harness, &validators).expect("the case resumes");
    run(&mut resumed)
}

/// The stop `RepeatedTestFailure` the story's one test gives after `count` results in a row.
fn repeated(count: u64) -> Stop {
    Stop::RepeatedTestFailure {
        tests: vec![NEW_TEST.to_owned()],
        count,
    }
}

/// The stop of a run that reached the merge.
fn merge_requested() -> Stop {
    Stop::ApprovalRequired(vec!["repository.merge".to_owned()])
}

/// Acceptance 1, the story's check: after a code edit leaves the test red, an edit of
/// `docs/notes.md` alone, then one of `README.md` and one of `CHANGELOG.md`, each leave it red
/// at a new revision and none of them stops the run `RepeatedTestFailure`:
/// `implementation.edit` is offered again after each, and the fix carries the run to the merge.
#[test]
fn a_doc_only_revision_is_no_new_attempt() {
    let tree = scratch("doc-only");
    let harness = Developer::new(&[
        Edit::Code,
        Edit::Docs,
        Edit::Readme,
        Edit::Changelog,
        Edit::Fix,
    ]);
    let (stop, text, _) = delivered(&tree, "doc-only", &harness);
    let edits = harness.edits();
    assert_ne!(stop, Ok(repeated(2)), "{edits:?}\n{text}");
    assert_eq!(
        edits,
        vec![
            Edit::Code,
            Edit::Docs,
            Edit::Readme,
            Edit::Changelog,
            Edit::Fix
        ],
        "implementation.edit was not offered again after a documentation edit: {stop:?}\n{text}"
    );
    assert_eq!(stop, Ok(merge_requested()), "{edits:?}\n{text}");
}

/// Acceptance 2: two code edits in a row that leave the same test failing stop the run
/// `RepeatedTestFailure` (2) after the second, with no third edit.
#[test]
fn two_code_edits_that_leave_the_same_tests_failing_stop_the_run() {
    let tree = scratch("two-code");
    let harness = Developer::new(&[Edit::Code, Edit::Code]);
    let (stop, text, _) = delivered(&tree, "two-code", &harness);
    let edits = harness.edits();
    assert_eq!(stop, Ok(repeated(2)), "{edits:?}\n{text}");
    assert_eq!(edits, vec![Edit::Code, Edit::Code], "{text}");
}

/// A documentation edit between two code edits leaves the row as it was: it neither counts nor
/// starts the row over, so the second code edit stops the run `RepeatedTestFailure` (2), and an
/// edit that changes code and documentation together counts as a code edit.
#[test]
fn a_doc_only_revision_between_code_edits_leaves_the_row() {
    let tree = scratch("between");
    let harness = Developer::new(&[Edit::Code, Edit::Docs, Edit::CodeAndDocs]);
    let (stop, text, _) = delivered(&tree, "between", &harness);
    let edits = harness.edits();
    assert_eq!(stop, Ok(repeated(2)), "{edits:?}\n{text}");
    assert_eq!(
        edits,
        vec![Edit::Code, Edit::Docs, Edit::CodeAndDocs],
        "{text}"
    );
}

/// Acceptance 3: a case stopped `RepeatedTestFailure` and resumed runs one implementation edit
/// before it stops on the same reason again; the resumed run's check again at the revision it
/// stopped at does not stop it at once.
#[test]
fn a_resumed_case_gets_an_implementation_edit_before_it_stops_again() {
    let tree = scratch("resume-stops");
    let first = Developer::new(&[Edit::Code, Edit::Code]);
    let (stop, text, id) = delivered(&tree, "resume-stops", &first);
    assert_eq!(stop, Ok(repeated(2)), "{:?}\n{text}", first.edits());
    let again = Developer::new(&[Edit::Code]);
    let (stop, text) = resumed(&tree, "resume-stops", &id, &again);
    let edits = again.edits();
    assert_eq!(
        edits,
        vec![Edit::Code],
        "the resumed run made no implementation edit before it stopped: {stop:?}\n{text}"
    );
    assert_eq!(stop, Ok(repeated(2)), "{edits:?}\n{text}");
}

/// Acceptance 3, the case recovers: resumed after `RepeatedTestFailure`, its first edit fixes
/// the implementation and the run goes on to the merge.
#[test]
fn a_resumed_case_can_recover() {
    let tree = scratch("resume-recovers");
    let first = Developer::new(&[Edit::Code, Edit::Code]);
    let (stop, text, id) = delivered(&tree, "resume-recovers", &first);
    assert_eq!(stop, Ok(repeated(2)), "{:?}\n{text}", first.edits());
    let again = Developer::new(&[Edit::Fix]);
    let (stop, text) = resumed(&tree, "resume-recovers", &id, &again);
    let edits = again.edits();
    assert_eq!(edits, vec![Edit::Fix], "{stop:?}\n{text}");
    assert_eq!(stop, Ok(merge_requested()), "{edits:?}\n{text}");
}
