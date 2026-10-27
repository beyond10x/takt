//! Adversarial cases for story:python-modules-are-kept-only-when-a-python-answers. A case that
//! opened with no Python keeps no `python-modules.json`, so a later resume asks again and a step
//! or an observation finds the file absent rather than present. Each case here drives one of the
//! consequences: the file a resume writes and a replay of an earlier step, the file a step or an
//! observation plants where none was, a fail-closed answer at open, a resume whose test command
//! now runs a Python, and the reference page's listing of a `--test-cmd true` case.

use std::cell::{Cell, RefCell};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use takt::harness::{Harness, StepContext, StepReport};
use takt::run::{MODULES_FILE, Modules};
use takt::validator::{TestsRun, Validator};
use takt::{Run, RunOptions, Stop};

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

const CODE: &str = "def answer():\n    return 0\n";

/// The test script: fails until `answer.txt` is in the tree; names no Python.
const CHECK: &str = "test -f answer.txt\n";

/// A test script that names no Python and copies a data file the tree holds, `plant.json`, into
/// every case directory: what a test suite running code a step wrote can do.
const CHECK_PLANTS: &str = "if [ -f plant.json ]; then for d in .takt/cases/*/; do cp plant.json \"${d}python-modules.json\"; done; fi\ntest -f answer.txt\n";

/// A module file naming no standard-library module: what a step would plant to weaken the guard.
const WEAK: &str = "{\"from\":[\"planted\"],\"strict\":[],\"names\":[]}";

/// An absolute Python outside the tree that is not there: the interpreter cannot be established.
const MISSING_PYTHON: &str = "/nonexistent-takt-adv-python-modules/bin/python3";

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

fn repository(name: &str, files: &[(&str, &str)]) -> Scratch {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("python-modules-adv1-{}-{name}", std::process::id()));
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

/// A judgement step that writes its payload the first time it is offered; a path starting with
/// `CASE/` is written under every case directory of the tree. It keeps what it was offered and
/// the files the case directories held when it was asked.
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
                if let Some(rest) = path.strip_prefix("CASE/") {
                    for case in case_dirs(context.tree) {
                        fs::write(case.join(rest), text).expect("a planted file");
                    }
                } else {
                    let at = context.tree.join(path);
                    fs::create_dir_all(at.parent().expect("a parent")).expect("a directory");
                    fs::write(&at, text).expect("a file");
                }
            }
        }
        Ok(StepReport {
            action: "implementation.edit".to_owned(),
            summary: "made the tests pass".to_owned(),
            tokens: Some(1),
        })
    }
}

fn case_dirs(tree: &Path) -> Vec<PathBuf> {
    let Ok(cases) = fs::read_dir(tree.join(".takt/cases")) else {
        return Vec::new();
    };
    cases
        .map(|case| case.expect("an entry").path())
        .filter(|path| path.is_dir())
        .collect()
}

fn case_files(tree: &Path) -> Vec<String> {
    let mut names = Vec::new();
    for case in case_dirs(tree) {
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

struct Ran {
    stop: anyhow::Result<Stop>,
    text: String,
    case_dir: PathBuf,
}

impl Ran {
    fn refusal(&self) -> &str {
        match &self.stop {
            Ok(Stop::StepRefused(why)) => why,
            other => panic!("the run is refused, not {other:?}\n{}", self.text),
        }
    }

    fn id(&self) -> String {
        self.case_dir
            .file_name()
            .expect("a case id")
            .to_string_lossy()
            .into_owned()
    }
}

fn drive(run: &mut Run<'_>) -> Ran {
    run.continue_past_red_base();
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    Ran {
        stop,
        text: String::from_utf8_lossy(&out).into_owned(),
        case_dir: run.case.dir.clone(),
    }
}

fn validators(command: &str) -> Vec<Box<dyn Validator>> {
    vec![Box::new(TestsRun::new(command).expect("a command"))]
}

fn run_writing(tree: &Scratch, command: &str, files: &[(&str, &str)]) -> Ran {
    let validators = validators(command);
    let harness = Writes::with(files);
    let protocol = tree.root.join("protocol.yaml").display().to_string();
    let mut run =
        Run::open(tree, options(protocol), &harness, &validators).expect("the case opens");
    drive(&mut run)
}

fn resume_writing(tree: &Scratch, id: &str, command: &str, files: &[(&str, &str)]) -> Ran {
    let validators = validators(command);
    let harness = Writes::with(files);
    let protocol = tree.root.join("protocol.yaml").display().to_string();
    let mut run =
        Run::resume(tree, options(protocol), id, &harness, &validators).expect("the case resumes");
    drive(&mut run)
}

fn first_harness_step(case_dir: &Path) -> u64 {
    let events = fs::read_to_string(case_dir.join("events.jsonl")).expect("the run record");
    events
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("a record line"))
        .find(|value| value["event"] == "step" && value["by"] == "harness")
        .map(|value| value["step"].as_u64().expect("a step number"))
        .expect("the record holds a harness step")
}

/// A replay rebuilds the case directory "as the step found it" (`replay.rs`, `rebuild_case`).
/// The step of the first run found no module file: the case opened with `sh check.sh`, which
/// names no Python. A resume whose test command names a Python it cannot establish (the
/// operator's `--test-cmd` for that run) writes the file then, fail-closed. Replaying the first
/// run's step afterwards must rebuild the case without that file, which the step never saw;
/// before this change the file was written at open and never rewritten, so the replay copied
/// the bytes the step had found.
#[test]
fn replay_before_a_resume_that_wrote_the_file_rebuilds_without_it() {
    let tree = repository(
        "replay-after-resume",
        &[("check.sh", CHECK), ("todo.py", CODE)],
    );
    let opened = validators("sh check.sh");
    let harness = Writes::with(&[("answer.txt", "yes\n")]);
    let protocol = tree.root.join("protocol.yaml").display().to_string();
    let (case_dir, ran) = {
        let mut run =
            Run::open(&tree, options(protocol.clone()), &harness, &opened).expect("the case opens");
        let ran = drive(&mut run);
        (run.case.dir.clone(), ran)
    };
    assert!(
        !case_dir.join(MODULES_FILE).exists(),
        "the case opened without a module file\n{}",
        ran.text
    );
    let saw = harness
        .case_files
        .borrow()
        .first()
        .cloned()
        .expect("a call");
    assert!(
        !saw.iter().any(|name| name == MODULES_FILE),
        "the first run's step found no module file: {saw:?}"
    );
    let step = first_harness_step(&case_dir);

    let resumed_with = validators(&format!("{MISSING_PYTHON} -c pass"));
    let idle = Writes::with(&[]);
    {
        let _run = Run::resume(&tree, options(protocol), &ran.id(), &idle, &resumed_with)
            .expect("the case resumes");
    }
    assert!(
        !case_dir.join(MODULES_FILE).exists(),
        "story:python-modules-are-kept-only-when-a-python-answers: a resume asks again and writes no module file, so the file exists only as the case opened with it"
    );

    let start =
        takt::replay::starting_state(&case_dir, step, Some(1)).expect("the step's starting state");
    let replaying = Writes::with(&[]);
    let dir = tree.root.join("replay-1");
    takt::replay::replay_once(&start, &tree, &replaying, None, &dir, None)
        .expect("the replay runs");
    let files = replaying.case_files.borrow();
    assert_eq!(files.len(), 1, "one call");
    assert!(
        !files[0].iter().any(|name| name == MODULES_FILE),
        "the replay of run 1's step rebuilt the case with a module file that step never found (written by the later resume): {:?}",
        files[0]
    );
}

/// The reference page's listing of the case a `--test-cmd true` run leaves
/// (`website/docs/reference/case-files.md`, "A case after one run") says what takt writes for a
/// test command that runs no Python. It must agree with what a case opened with `true` keeps.
#[test]
fn reference_listing_of_a_true_case_matches_what_takt_keeps() {
    let page =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../website/docs/reference/case-files.md");
    let text = fs::read_to_string(&page).expect("the reference page");
    let section = text
        .split("## A case after one run")
        .nth(1)
        .expect("the section");
    assert!(
        section.contains("--test-cmd true"),
        "the section's run uses --test-cmd true"
    );
    let listing = section
        .split("```console")
        .nth(1)
        .and_then(|block| block.split("```").next())
        .expect("the listing");
    let listed = listing
        .lines()
        .any(|line| line.trim() == ".takt/cases/<case>/python-modules.json");

    let tree = repository("reference", &[("todo.py", CODE)]);
    let opened = validators("true");
    let harness = Writes::with(&[]);
    let protocol = tree.root.join("protocol.yaml").display().to_string();
    let run = Run::open(&tree, options(protocol), &harness, &opened).expect("the case opens");
    let kept = run.case.dir.join(MODULES_FILE).exists();
    assert_eq!(
        listed, kept,
        "case-files.md lists python-modules.json for a --test-cmd true case: {listed}; takt keeps it: {kept}"
    );
}

/// A step that writes a module file into its case directory, where none is now, is refused and
/// the file removed; the resume then reads the built-in lists and refuses `json.py` as before.
#[test]
fn a_step_planting_the_absent_file_is_refused_and_removed() {
    let tree = repository("step-plants", &[("check.sh", CHECK), ("todo.py", CODE)]);
    let ran = run_writing(&tree, "sh check.sh", &[("CASE/python-modules.json", WEAK)]);
    let why = ran.refusal();
    assert!(why.contains(MODULES_FILE), "the refusal names it: {why}");
    assert!(
        !ran.case_dir.join(MODULES_FILE).exists(),
        "the planted file is removed\n{}",
        ran.text
    );
    let resumed = resume_writing(
        &tree,
        &ran.id(),
        "sh check.sh",
        &[("json.py", "VALUE = 1\n")],
    );
    let why = resumed.refusal();
    assert!(why.contains("json.py"), "json.py is refused: {why}");
    assert!(!resumed.case_dir.join(MODULES_FILE).exists());
    assert_eq!(Modules::read(&resumed.case_dir), Modules::default());
}

/// An observation whose test command copies a file a step wrote into the case directory, where no
/// module file is, is refused and the file removed; the resume reads the built-in lists.
#[test]
fn an_observation_planting_the_absent_file_is_refused_and_removed() {
    let tree = repository(
        "observation-plants",
        &[("check.sh", CHECK_PLANTS), ("todo.py", CODE)],
    );
    let ran = run_writing(
        &tree,
        "sh check.sh",
        &[("plant.json", WEAK), ("answer.txt", "yes\n")],
    );
    assert!(
        !ran.case_dir.join(MODULES_FILE).exists(),
        "the planted file is not left: {:?}\n{}",
        ran.stop,
        ran.text
    );
    assert!(
        !matches!(ran.stop, Ok(Stop::Completed(_))),
        "a run whose observation wrote into the case is not accepted: {:?}\n{}",
        ran.stop,
        ran.text
    );
    let resumed = resume_writing(
        &tree,
        &ran.id(),
        "sh check.sh",
        &[("json.py", "VALUE = 1\n")],
    );
    assert!(!resumed.case_dir.join(MODULES_FILE).exists());
    assert_eq!(Modules::read(&resumed.case_dir), Modules::default());
}

/// A case whose test command names a Python that cannot be established: no Python answered, and
/// the answer fails closed. That answer is not the built-in lists, so it is kept at open and the
/// resume reads it fail-closed. (A check on "a Python answered", the story's wording, would drop
/// it.)
#[test]
fn a_fail_closed_answer_at_open_is_kept() {
    let tree = repository("fail-closed", &[("todo.py", CODE)]);
    let opened = validators(&format!("{MISSING_PYTHON} -c pass"));
    let harness = Writes::with(&[]);
    let protocol = tree.root.join("protocol.yaml").display().to_string();
    let run = Run::open(&tree, options(protocol), &harness, &opened).expect("the case opens");
    let text = fs::read_to_string(run.case.dir.join(MODULES_FILE))
        .expect("the case keeps the fail-closed answer");
    let kept: Modules = serde_json::from_str(&text).expect("the module file reads");
    assert!(kept.fail_closed.is_some(), "kept fail-closed: {kept:?}");
    assert_eq!(kept.from, Modules::default().from, "no Python answered");
    assert_eq!(Modules::read(&run.case.dir), kept);
}

/// A case opened with no Python, resumed with a test command whose Python cannot be established:
/// the resume asks again and fails closed, so a Python file a step adds at the tree's root, which
/// the built-in lists would let through, is refused.
#[test]
fn resume_asking_again_fails_closed_on_a_new_python() {
    let tree = repository(
        "resume-fails-closed",
        &[("check.sh", CHECK), ("todo.py", CODE)],
    );
    let first = run_writing(&tree, "sh check.sh", &[("json.py", "VALUE = 1\n")]);
    assert!(first.refusal().contains("json.py"));
    assert!(!first.case_dir.join(MODULES_FILE).exists());
    let resumed = resume_writing(
        &tree,
        &first.id(),
        &format!("{MISSING_PYTHON} -c pass"),
        &[("helper.py", "VALUE = 1\n")],
    );
    let why = resumed.refusal();
    assert!(why.contains("helper.py"), "helper.py is refused: {why}");
    assert!(
        !resumed.case_dir.join(MODULES_FILE).exists(),
        "story:python-modules-are-kept-only-when-a-python-answers: the resume guards with its fail-closed answer and keeps no file"
    );
}
