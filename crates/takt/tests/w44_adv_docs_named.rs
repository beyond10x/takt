//! Adversary, wave 44 U2: story:docs-edits-the-tests-read-are-attempts.
//!
//! The rule held to (the story's "Decision (wave 44)"): a path under `docs/`, `README.md` or
//! `CHANGELOG.md` is documentation unless a test file at the revision names it; a test file is a
//! path the case's `tests.write` commits changed. The Decision's own example of a test that names
//! a documentation path is `crates/takt-cli/tests/cases_under_dot_takt.rs`, which names the
//! directory `docs/runs/…/case` and reads the files under it. § Outcome: "A revision counts as a
//! new attempt when it changes a path the story's tests can read", and "A revision that changes
//! only documentation no test reads still does not count."

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

fn scratch(name: &str, docs: &[&str]) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("adversary-w44-u2-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(&dir).expect("scratch dir");
    fs::write(dir.join("README.md"), "a story\n").expect("a file");
    for path in docs {
        let file = dir.join(path);
        fs::create_dir_all(file.parent().expect("a parent")).expect("a dir");
        fs::write(file, "broken\n").expect("a file");
    }
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

/// `tests.run` fails at a revision holding the tests file while `decider` is not [`FIXED`],
/// naming each line of the tests file not starting with `#` as a failing test; every other
/// action passes.
struct Scripted {
    action: &'static str,
    kind: &'static str,
    tests: &'static str,
    decider: &'static str,
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
        let tests = at(tree, revision, self.tests);
        let fails = self.action == "tests.run"
            && tests.is_some()
            && at(tree, revision, self.decider).as_deref() != Some(FIXED);
        let (result, summary) = if fails {
            let names: Vec<String> = tests
                .unwrap_or_default()
                .lines()
                .filter(|line| !line.starts_with('#'))
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

/// One `implementation.edit`: a new line in `path`, or `path` set to [`FIXED`].
#[derive(Clone, Copy, Debug)]
enum Edit {
    Attempt(&'static str),
    Fix(&'static str),
}

/// Writes `tests_text` to `tests` on the first `tests.write`, then makes `edits` in order as
/// `implementation.edit`s; once they run out, repeats the last.
struct Developer {
    tests: &'static str,
    tests_text: &'static str,
    edits: Vec<Edit>,
    actions: RefCell<Vec<String>>,
}

impl Developer {
    fn new(tests: &'static str, tests_text: &'static str, edits: &[Edit]) -> Self {
        Self {
            tests,
            tests_text,
            edits: edits.to_vec(),
            actions: RefCell::new(Vec::new()),
        }
    }

    fn edit_count(&self) -> usize {
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
        if !context.tree.join(self.tests).exists() && offered("tests.write") {
            return write(self.tests, self.tests_text.to_owned(), "tests.write");
        }
        let fixed = self.edit_count() >= self.edits.len()
            && matches!(self.edits.last(), Some(Edit::Fix(_)));
        if !fixed && offered("implementation.edit") {
            let n = self.edit_count();
            let edit = self.edits[n.min(self.edits.len() - 1)];
            return match edit {
                Edit::Attempt(path) => {
                    let mut text = fs::read_to_string(context.tree.join(path)).unwrap_or_default();
                    text.push_str(&format!("attempt {}\n", n + 1));
                    write(path, text, "implementation.edit")
                }
                Edit::Fix(path) => write(path, FIXED.to_owned(), "implementation.edit"),
            };
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
        }
        .to_owned();
        self.actions.borrow_mut().push(action.clone());
        Ok(StepReport {
            action,
            summary: "nothing to change".to_owned(),
            tokens: Some(100),
        })
    }
}

fn validators(
    forge: &Rc<dyn Forge>,
    tests: &'static str,
    decider: &'static str,
) -> Vec<Box<dyn Validator>> {
    vec![
        Box::new(Scripted {
            action: "tests.run",
            kind: "test_result",
            tests,
            decider,
        }),
        Box::new(Scripted {
            action: "gate.run",
            kind: "gate_result",
            tests,
            decider,
        }),
        Box::new(PrOpen::new(forge.clone())),
        Box::new(CiRead::new(forge.clone())),
        Box::new(ChangeRead),
    ]
}

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

struct Outcome {
    stop: Result<Stop, String>,
    text: String,
    actions: Vec<String>,
}

fn delivered(
    name: &str,
    docs: &[&str],
    harness: &Developer,
    decider: &'static str,
    iterations: u64,
) -> Outcome {
    let tree = scratch(name, docs);
    let forge: Rc<dyn Forge> = Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    });
    let validators = validators(&forge, harness.tests, decider);
    let mut run =
        Run::open(&tree, options(name, iterations), harness, &validators).expect("the case opens");
    let mut out = Vec::new();
    let stop = run.run(&mut out).map_err(|error| format!("{error:#}"));
    Outcome {
        stop,
        text: String::from_utf8(out).expect("utf-8"),
        actions: harness.actions.borrow().clone(),
    }
}

fn edits(actions: &[String]) -> usize {
    actions
        .iter()
        .filter(|a| *a == "implementation.edit")
        .count()
}

/// The Decision's own example names a directory (`docs/runs/…/case`, as
/// `cases_under_dot_takt.rs` does) and reads a file under it. Here the tests file names
/// `docs/fixtures/` and is decided by `docs/fixtures/case.txt`; every attempt there leaves the
/// test red. Two such attempts should stop `RepeatedTestFailure`, as two attempts in a file the
/// tests name in full do (`w42_adv_repeat_count.rs`).
#[test]
fn attempts_in_a_docs_file_under_a_directory_the_tests_name_are_counted() {
    const CASE: &str = "docs/fixtures/case.txt";
    let harness = Developer::new(
        TESTS,
        "# reads every file under docs/fixtures/\nremove::t1\n",
        &[Edit::Attempt(CASE)],
    );
    let outcome = delivered("dir-named", &[CASE], &harness, CASE, 30);
    assert!(
        matches!(outcome.stop, Ok(Stop::RepeatedTestFailure { .. })),
        "stop {:?} after {} attempts in {CASE}, under the directory the tests file names, each leaving the test red: {:?}\n{}",
        outcome.stop,
        edits(&outcome.actions),
        outcome.actions,
        outcome.text
    );
}

/// "A revision that changes only documentation no test reads still does not count." The tests
/// file names `docs/guide.md.tpl` only; an edit of `docs/guide.md` between a code edit and the
/// fix is one no test reads, and should leave the row alone so the fix is reached.
#[test]
fn a_docs_path_that_is_only_a_prefix_of_a_named_path_is_not_counted() {
    const GUIDE: &str = "docs/guide.md";
    let harness = Developer::new(
        TESTS,
        "# reads docs/guide.md.tpl\nremove::t1\n",
        &[
            Edit::Attempt(IMPLEMENTATION),
            Edit::Attempt(GUIDE),
            Edit::Fix(IMPLEMENTATION),
        ],
    );
    let outcome = delivered("prefix", &[GUIDE], &harness, IMPLEMENTATION, 30);
    assert_eq!(
        outcome.stop,
        Ok(Stop::ApprovalRequired(vec!["repository.merge".to_owned()])),
        "an edit of {GUIDE}, which no test names, was counted: {:?}\n{}",
        outcome.actions,
        outcome.text
    );
}

/// Green probe: the tests file names `docs/fixture.txt`, and an edit of `docs/notes.md` (named
/// by nothing) between a code edit and the fix is not counted.
#[test]
fn a_docs_edit_beside_a_named_one_is_still_not_counted() {
    let harness = Developer::new(
        TESTS,
        "# reads docs/fixture.txt\nremove::t1\n",
        &[
            Edit::Attempt(IMPLEMENTATION),
            Edit::Attempt("docs/notes.md"),
            Edit::Attempt("README.md"),
            Edit::Fix(IMPLEMENTATION),
        ],
    );
    let outcome = delivered(
        "beside",
        &["docs/fixture.txt", "docs/notes.md"],
        &harness,
        IMPLEMENTATION,
        30,
    );
    assert_eq!(
        outcome.stop,
        Ok(Stop::ApprovalRequired(vec!["repository.merge".to_owned()])),
        "{:?}\n{}",
        outcome.actions,
        outcome.text
    );
}

/// Green probe: a tests file that itself lies under `docs/` and names the fixture: attempts in
/// the fixture are counted.
#[test]
fn a_tests_file_under_docs_that_names_the_fixture_counts_its_attempts() {
    const FIXTURE: &str = "docs/fixture.txt";
    let harness = Developer::new(
        "docs/tests/remove_test.txt",
        "# reads docs/fixture.txt\nremove::t1\n",
        &[Edit::Attempt(FIXTURE)],
    );
    let outcome = delivered("tests-under-docs", &[FIXTURE], &harness, FIXTURE, 30);
    assert!(
        matches!(outcome.stop, Ok(Stop::RepeatedTestFailure { .. })),
        "stop {:?} after {} attempts: {:?}\n{}",
        outcome.stop,
        edits(&outcome.actions),
        outcome.actions,
        outcome.text
    );
}

/// Green probe: the tests file names `README.md`; attempts in `README.md` are counted.
#[test]
fn attempts_in_a_readme_the_tests_name_are_counted() {
    let harness = Developer::new(
        TESTS,
        "# reads README.md\nremove::t1\n",
        &[Edit::Attempt("README.md")],
    );
    let outcome = delivered("readme", &[], &harness, "README.md", 30);
    assert!(
        matches!(outcome.stop, Ok(Stop::RepeatedTestFailure { .. })),
        "stop {:?} after {} attempts: {:?}\n{}",
        outcome.stop,
        edits(&outcome.actions),
        outcome.actions,
        outcome.text
    );
}
