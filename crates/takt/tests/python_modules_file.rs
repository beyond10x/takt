//! story:python-modules-are-kept-only-when-a-python-answers: a case keeps `python-modules.json`
//! only when a Python answered at case open. With no Python answering, the case keeps no such
//! file, and the step guard uses the built-in lists at open, at resume and in a replay, exactly
//! as it does when the file holds them.

use std::cell::{Cell, RefCell};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use takt::harness::{Harness, StepContext, StepReport};
use takt::run::{MODULES_FILE, Modules};
use takt::validator::{TestsRun, Validator};
use takt::{Run, RunOptions, Stop};

/// The edit-and-test protocol the step guard's tests run.
const EDIT_AND_TEST: &str = "format: protocol/1

protocol:
  id: edit.and.test
  revision: 1
  description: >-
    Edit the implementation and run the tests against it; accepted once they pass.

artifacts:
  implementation:
    description: a revision of the code

evidence_kinds:
  test_result:
    description: the result of running tests against an implementation revision

claims:
  tests.pass:
    description: the tests pass for the current implementation revision
    true_when:
      evidence:
        kind: test_result
        result: pass
        subject: implementation

obligations:
  tests.required:
    description: the tests pass for the current implementation revision
    discharged_when:
      claim: tests.pass

actions:
  implementation.edit:
    description: change the implementation
    effect: write
  tests.run:
    description: run the tests against an implementation revision
    effect: read
    may_produce:
      - evidence: test_result

outcomes:
  accepted:
    description: the tests pass for the current implementation revision
    requires:
      claim: tests.pass
";

/// The code under test, wrong.
const CODE: &str = "def answer():\n    return 0\n";

/// The test script: it fails until `answer.txt` is in the tree, and names no Python. A test
/// command that passed at the base would accept the case before any judgement step ran.
const CHECK: &str = "test -f answer.txt\n";

/// A module named as the standard library's `json` is, written in the tree's root: a directory
/// the built-in lists cover, since the test command runs there.
const JSON_MODULE: &str = "VALUE = 1\n";

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
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// A scratch directory under this crate's build directory: the protocol file and the step's
/// payload at its root and the repository in `repo/`. Dropping it removes both.
struct Scratch {
    root: PathBuf,
    repo: PathBuf,
}

impl std::ops::Deref for Scratch {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.repo
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// A repository on `main` holding `files` (path, text) in its first commit, beside a
/// `.gitignore` that ignores `target/` and Python's byte-code caches.
fn repository(name: &str, files: &[(&str, &str)]) -> Scratch {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("python-modules-file-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let repo = root.join("repo");
    fs::create_dir_all(&repo).expect("a directory");
    fs::write(root.join("protocol.yaml"), EDIT_AND_TEST).expect("the protocol");
    fs::write(repo.join(".gitignore"), "target/\n__pycache__/\n").expect("a file");
    for (path, text) in files {
        let at = repo.join(path);
        fs::create_dir_all(at.parent().expect("a parent")).expect("a directory");
        fs::write(&at, text).expect("a file");
    }
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "start"]);
    Scratch { root, repo }
}

fn options(protocol: String) -> RunOptions {
    RunOptions {
        protocol,
        intent: "make the tests pass".to_owned(),
        inputs: Vec::new(),
        iterations: Some(4),
        hours: None,
        tokens: None,
        case_id: None,
        gate: None,
    }
}

/// A judgement step that writes its payload (each path and its text) into the tree the first
/// time it is offered, and changes nothing after. It keeps what each call was offered and the
/// case files the case directory of its tree held when it was asked.
struct Writes {
    payload: Vec<(String, String)>,
    done: Cell<bool>,
    offered: RefCell<Vec<Vec<String>>>,
    case_files: RefCell<Vec<Vec<String>>>,
}

impl Writes {
    fn with(payload: &[(&str, &str)]) -> Self {
        Self {
            payload: payload
                .iter()
                .map(|(path, text)| ((*path).to_owned(), (*text).to_owned()))
                .collect(),
            done: Cell::new(false),
            offered: RefCell::new(Vec::new()),
            case_files: RefCell::new(Vec::new()),
        }
    }
}

impl Harness for Writes {
    fn name(&self) -> &str {
        "writes"
    }

    fn step(&self, context: &StepContext<'_>) -> Result<StepReport, anyhow::Error> {
        self.offered.borrow_mut().push(context.offered.to_vec());
        self.case_files.borrow_mut().push(case_files(context.tree));
        if !context.offered.iter().any(|a| a == "implementation.edit") {
            anyhow::bail!("implementation.edit not offered");
        }
        if !self.done.replace(true) {
            for (path, text) in &self.payload {
                write(context.tree, path, text);
            }
        }
        Ok(StepReport {
            action: "implementation.edit".to_owned(),
            summary: "made the tests pass".to_owned(),
            tokens: Some(1),
        })
    }
}

/// What one run left: its stop, what it printed, the case's evidence records and its directory.
struct Ran {
    stop: anyhow::Result<Stop>,
    text: String,
    records: Vec<serde_json::Value>,
    case_dir: PathBuf,
}

impl Ran {
    fn passes(&self) -> usize {
        self.records
            .iter()
            .filter(|r| r["kind"] == "test_result" && r["result"] == "pass")
            .count()
    }

    fn refusal(&self) -> &str {
        match &self.stop {
            Ok(Stop::StepRefused(why)) => why,
            other => panic!(
                "the run is refused, not {other:?}; records {:?}\n{}",
                self.records, self.text
            ),
        }
    }
}

fn records(case_dir: &Path) -> Vec<serde_json::Value> {
    let Ok(entries) = fs::read_dir(case_dir.join("evidence")) else {
        return Vec::new();
    };
    let mut paths: Vec<PathBuf> = entries.map(|e| e.expect("an entry").path()).collect();
    paths.sort();
    paths
        .iter()
        .map(|path| serde_json::from_slice(&fs::read(path).expect("a record")).expect("json"))
        .collect()
}

/// The names of the files in each case directory of `tree`, sorted.
fn case_files(tree: &Path) -> Vec<String> {
    let mut names = Vec::new();
    let Ok(cases) = fs::read_dir(tree.join(".takt/cases")) else {
        return names;
    };
    for case in cases {
        let case = case.expect("an entry").path();
        for file in fs::read_dir(&case).expect("a case directory") {
            names.push(
                file.expect("an entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned(),
            );
        }
    }
    names.sort();
    names
}

fn has_module_file(files: &[String]) -> bool {
    files.iter().any(|name| name == MODULES_FILE)
}

/// Runs `run` past its red base to its stop, and what it left.
fn drive(run: &mut Run<'_>) -> Ran {
    run.continue_past_red_base();
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    Ran {
        stop,
        text: String::from_utf8_lossy(&out).into_owned(),
        records: records(&run.case.dir),
        case_dir: run.case.dir.clone(),
    }
}

/// Opens a case on `tree` with `command` as the test command, and runs it with a step that writes
/// each of `files` (path, text).
fn run_writing(tree: &Scratch, command: &str, files: &[(&str, &str)]) -> Ran {
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(TestsRun::new(command).expect("a command"))];
    let harness = Writes::with(files);
    let protocol = tree.root.join("protocol.yaml").display().to_string();
    let mut run =
        Run::open(tree, options(protocol), &harness, &validators).expect("the case opens");
    drive(&mut run)
}

/// Resumes the case `id` on `tree` with the same test command, and runs it with a step that writes
/// each of `files` (path, text).
fn resume_writing(tree: &Scratch, id: &str, command: &str, files: &[(&str, &str)]) -> Ran {
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(TestsRun::new(command).expect("a command"))];
    let harness = Writes::with(files);
    let protocol = tree.root.join("protocol.yaml").display().to_string();
    let mut run =
        Run::resume(tree, options(protocol), id, &harness, &validators).expect("the case resumes");
    drive(&mut run)
}

/// The number of the first step a harness took in the case's record, as the record numbers it.
fn first_harness_step(case_dir: &Path) -> u64 {
    let events = fs::read_to_string(case_dir.join("events.jsonl")).expect("the run record");
    events
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("a record line"))
        .find(|value| value["event"] == "step" && value["by"] == "harness")
        .map(|value| value["step"].as_u64().expect("a step number"))
        .expect("the record holds a harness step")
}

/// Writes `text` at `path` in `tree`.
fn write(tree: &Path, path: &str, text: &str) {
    let at = tree.join(path);
    fs::create_dir_all(at.parent().expect("a parent")).expect("a directory");
    fs::write(&at, text).expect("a file");
}

/// The step that wrote `path` was refused and put back: the refusal names it, nothing passed, the
/// case's branch holds no commit beyond the base, and nothing is left but the cases and what the
/// test command leaves in ignored caches.
fn assert_refused_and_put_back(tree: &Scratch, ran: &Ran, path: &str) {
    let why = ran.refusal();
    assert!(
        why.contains(path),
        "the refusal names {path}: {why}\n{}",
        ran.text
    );
    assert_eq!(ran.passes(), 0, "{:?}\n{}", ran.records, ran.text);
    assert_eq!(
        git(tree, &["rev-parse", "HEAD"]),
        git(tree, &["rev-list", "--max-parents=0", "HEAD"]),
        "{}",
        ran.text
    );
    let left = git(tree, &["status", "--porcelain", "--", ".", ":!.takt/cases"]);
    assert!(
        left.is_empty(),
        "the step's change is left: {left}\n{}",
        ran.text
    );
}

/// The `python3` the `PATH` names, if one is there.
fn python3_on_path() -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join("python3"))
        .find(|candidate| candidate.is_file())
}

/// Acceptance 1: a case whose test command, `sh check.sh`, names no Python, keeps no
/// `python-modules.json`, and a step that writes a stdlib-named module into the tree's root, an
/// import directory the built-in lists cover, is refused and put back as before.
#[test]
fn no_python_no_file() {
    let tree = repository("no-python", &[("check.sh", CHECK), ("todo.py", CODE)]);
    let ran = run_writing(&tree, "sh check.sh", &[("json.py", JSON_MODULE)]);
    assert_refused_and_put_back(&tree, &ran, "json.py");
    assert!(
        !ran.case_dir.join(MODULES_FILE).exists(),
        "the case keeps no module file\n{}",
        ran.text
    );
    assert_eq!(Modules::read(&ran.case_dir), Modules::default());
}

/// Acceptance 2: with a Python on `PATH` that the test command names, the case opens with the
/// file written, holding the names that Python answered. Skipped, with a note, where the host has
/// no `python3`.
#[test]
fn a_python_writes_the_file() {
    let Some(python) = python3_on_path() else {
        eprintln!("skipped: no python3 on PATH, so no interpreter answers at case open");
        return;
    };
    let tree = repository("a-python", &[("todo.py", CODE)]);
    let validators: Vec<Box<dyn Validator>> = vec![Box::new(
        TestsRun::new("python3 -c pass").expect("a command"),
    )];
    let harness = Writes::with(&[]);
    let protocol = tree.root.join("protocol.yaml").display().to_string();
    let run = Run::open(&tree, options(protocol), &harness, &validators).expect("the case opens");
    let text = fs::read_to_string(run.case.dir.join(MODULES_FILE))
        .expect("the case keeps the module names its Python answered");
    let kept: Modules = serde_json::from_str(&text).expect("the module file reads");
    let answered = python.display().to_string();
    assert!(
        kept.from.iter().any(|from| from.starts_with(&answered)),
        "the file names the Python that answered: {:?}",
        kept.from
    );
    assert!(kept.fail_closed.is_none(), "{:?}", kept.fail_closed);
    assert_eq!(Modules::read(&run.case.dir), kept);
}

/// Acceptance 3: resuming the case of acceptance 1 writes no file, and its step guard refuses the
/// same stdlib-named module.
#[test]
fn resume_without_the_file_writes_none() {
    let tree = repository("resume", &[("check.sh", CHECK), ("todo.py", CODE)]);
    let first = run_writing(&tree, "sh check.sh", &[("json.py", JSON_MODULE)]);
    assert_refused_and_put_back(&tree, &first, "json.py");
    let id = first
        .case_dir
        .file_name()
        .expect("a case id")
        .to_string_lossy()
        .into_owned();
    let resumed = resume_writing(&tree, &id, "sh check.sh", &[("json.py", JSON_MODULE)]);
    assert_refused_and_put_back(&tree, &resumed, "json.py");
    assert!(
        !resumed.case_dir.join(MODULES_FILE).exists(),
        "the resumed case keeps no module file\n{}",
        resumed.text
    );
    assert_eq!(Modules::read(&resumed.case_dir), Modules::default());
}

/// Acceptance 4: a replay of the harness step of a case that names no Python rebuilds its tree
/// without the module file, and is offered what the run was offered, so the step is judged as
/// the run judged it. The step writes `answer.txt`, so the test passes and the case is accepted.
#[test]
fn replay_without_the_file() {
    let tree = repository("replay", &[("check.sh", CHECK), ("todo.py", CODE)]);
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(TestsRun::new("sh check.sh").expect("a command"))];
    let harness = Writes::with(&[("answer.txt", "yes\n")]);
    let protocol = tree.root.join("protocol.yaml").display().to_string();
    let mut run =
        Run::open(&tree, options(protocol), &harness, &validators).expect("the case opens");
    let ran = drive(&mut run);
    let case_dir = run.case.dir.clone();
    assert!(
        !case_dir.join(MODULES_FILE).exists(),
        "the case keeps no module file\n{}",
        ran.text
    );
    let step = first_harness_step(&case_dir);
    let offered_then = harness
        .offered
        .borrow()
        .first()
        .cloned()
        .expect("the run offered the step");

    let start =
        takt::replay::starting_state(&case_dir, step, None).expect("the step's starting state");
    let replaying = Writes::with(&[]);
    let dir = tree.root.join("replay-1");
    let replayed = takt::replay::replay_once(&start, &tree, &replaying, None, &dir, None)
        .expect("the replay runs");
    assert_eq!(
        replayed.result,
        Ok("implementation.edit".to_owned()),
        "the replayed step's action"
    );

    let given = replaying.offered.borrow().clone();
    assert_eq!(
        given,
        vec![offered_then.clone()],
        "the replay is offered what the run was"
    );
    let files = replaying.case_files.borrow();
    assert_eq!(files.len(), 1, "one call");
    assert!(
        files[0].iter().any(|name| name == "intent.md"),
        "the case is rebuilt in the replay's tree: {:?}",
        files[0]
    );
    assert!(
        !has_module_file(&files[0]),
        "the rebuilt case holds the module file: {:?}",
        files[0]
    );
    let record: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(dir.join("replay.json")).expect("the replay's record"),
    )
    .expect("the record reads");
    assert_eq!(record["offered"], serde_json::json!(offered_then));
    assert_eq!(record["action"], "implementation.edit");
}
