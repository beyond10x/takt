//! Adversary, wave 44 U2b pass 2: story:docs-edits-the-tests-read-are-attempts, the matching of
//! "names it" after "Decision (wave 44, review)": a test file names a documentation path when its
//! content holds that path or one of its parent directories, where the match ends at a path
//! boundary (the end of the text, `/`, or a character that cannot be part of a path).

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
        .join(format!("adversary-w44-u2b-{}-{name}", std::process::id()));
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
        actions: harness.actions.borrow().clone(),
    }
}

fn edits(actions: &[String]) -> usize {
    actions
        .iter()
        .filter(|a| *a == "implementation.edit")
        .count()
}

fn merge_requested() -> Result<Stop, String> {
    Ok(Stop::ApprovalRequired(vec!["repository.merge".to_owned()]))
}

/// Attempts in `case` that each leave the test red, under a tests file whose text is `tests`:
/// the stop.
fn attempts_under(name: &str, tests: &'static str, case: &'static str) -> Outcome {
    let harness = Developer::new(TESTS, tests, &[Edit::Attempt(case)]);
    delivered(name, &[case], &harness, case, 30)
}

/// A code edit, an edit of `docs_edit`, then the fix, under a tests file whose text is `tests`:
/// the stop. The merge is requested when the documentation edit is not counted, and
/// `RepeatedTestFailure` (2) stops the run when it is.
fn code_docs_fix(name: &str, tests: &'static str, docs_edit: &'static str) -> Outcome {
    let harness = Developer::new(
        TESTS,
        tests,
        &[
            Edit::Attempt(IMPLEMENTATION),
            Edit::Attempt(docs_edit),
            Edit::Fix(IMPLEMENTATION),
        ],
    );
    let docs: &[&str] = if docs_edit.starts_with("docs/") {
        &[docs_edit]
    } else {
        &[]
    };
    delivered(name, docs, &harness, IMPLEMENTATION, 30)
}

fn counted(outcome: &Outcome) -> bool {
    matches!(outcome.stop, Ok(Stop::RepeatedTestFailure { .. }))
}

/// Red. The review decision: a parent directory names its files where the match ends at a path
/// boundary, and a quote is one. `"docs/fixtures"` in a Rust string names
/// `docs/fixtures/case.txt`; the implementation needs a trailing `/`.
#[test]
fn a_directory_named_without_its_trailing_slash_names_its_files() {
    const CASE: &str = "docs/fixtures/case.txt";
    let outcome = attempts_under(
        "dir-no-slash",
        "let dir = Path::new(\"docs/fixtures\");\nremove::t1\n",
        CASE,
    );
    assert!(
        counted(&outcome),
        "stop {:?} after {} attempts in {CASE}, whose parent \"docs/fixtures\" the tests file names: {:?}",
        outcome.stop,
        edits(&outcome.actions),
        outcome.actions
    );
}

/// Red. The decision's own example, in the form `cases_under_dot_takt.rs` writes it:
/// `concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/runs/<run>/case")`, a parent directory
/// reached from the crate, then a file under it read. It names every file under it.
#[test]
fn the_decisions_example_form_names_the_files_under_it() {
    const CASE: &str = "docs/runs/turn-1/case/case.yaml";
    let outcome = attempts_under(
        "decision-example",
        "const CASE: &str = concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/../../docs/runs/turn-1/case\");\nremove::t1\n",
        CASE,
    );
    assert!(
        counted(&outcome),
        "stop {:?} after {} attempts in {CASE}, under the directory the tests file names as cases_under_dot_takt.rs does: {:?}",
        outcome.stop,
        edits(&outcome.actions),
        outcome.actions
    );
}

/// Red. The same directory with its trailing slash, reached through `../../`: a full file path
/// is matched anywhere in the text, a directory only where the word starts with it.
#[test]
fn a_directory_with_its_slash_after_a_relative_prefix_names_its_files() {
    const CASE: &str = "docs/fixtures/case.txt";
    let outcome = attempts_under(
        "dir-prefixed",
        "let dir = concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/../../docs/fixtures/\");\nremove::t1\n",
        CASE,
    );
    assert!(
        counted(&outcome),
        "stop {:?} after {} attempts in {CASE}, whose parent \"/../../docs/fixtures/\" the tests file names: {:?}",
        outcome.stop,
        edits(&outcome.actions),
        outcome.actions
    );
}

/// Red. "a character that cannot be part of a path" is a boundary; `~` can be part of a path
/// (an editor's backup), so `docs/guide.md~` does not name `docs/guide.md`.
#[test]
fn a_tilde_that_continues_a_path_is_no_boundary() {
    let outcome = code_docs_fix(
        "tilde",
        "# reads docs/guide.md~\nremove::t1\n",
        "docs/guide.md",
    );
    assert_eq!(
        outcome.stop,
        merge_requested(),
        "docs/guide.md~ named docs/guide.md: {:?}",
        outcome.actions
    );
}

/// Green probe. A Rust escape right after the path: `\` cannot be part of the path, so
/// `"docs/fixture.txt\n"` names `docs/fixture.txt`.
#[test]
fn a_rust_escape_after_the_path_is_a_boundary() {
    const CASE: &str = "docs/fixture.txt";
    let outcome = attempts_under(
        "escape",
        "let s = \"docs/fixture.txt\\n\";\nremove::t1\n",
        CASE,
    );
    assert!(counted(&outcome), "{:?}", outcome.stop);
}
