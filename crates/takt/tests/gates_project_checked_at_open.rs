//! story:gates-project-checked-at-open: a gates project file that the gate would refuse refuses
//! the case when it opens, before the first model call, naming the file and the reason. The
//! base revision's gates document and project file, the two the case pins at open, are prepared
//! as `gate.run` prepares them, running none of their commands: a project file with
//! `timeout_seconds: 1800` refuses the open with no case left, the harness never asked; a valid
//! one opens as before, and its gate runs.

use std::cell::Cell;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::anyhow;
use takt::harness::{Harness, StepContext, StepReport};
use takt::validator::{GateRun, Validator};
use takt::{Run, RunOptions, Stop};

/// The protocol the case opens on: `gate.run` and nothing a harness decides.
const PROTOCOL: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/gate-only/1.yaml"
);

/// The project file, relative to the tree.
const PROJECT: &str = ".engineering/assertions-project.yaml";

/// A scratch directory under this crate's build directory, removed when dropped.
struct Scratch(PathBuf);

impl std::ops::Deref for Scratch {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            let message = format!("remove the scratch tree {}: {error}", self.0.display());
            if std::thread::panicking() {
                eprintln!("{message}");
            } else {
                panic!("{message}");
            }
        }
    }
}

fn git(tree: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(args)
        .current_dir(tree)
        .output()
        .expect("git runs");
    assert!(output.status.success(), "git {args:?}");
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// A committed repository whose gates document holds one gate, `probe`, true when the command
/// `probe` (`<program> -c 'exit 0'`) exits 0, and whose project file binds it with `timeout_seconds`.
fn repository(name: &str, timeout_seconds: u64, program: &str) -> Scratch {
    let tree = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "gates-project-checked-at-open-{}-{name}",
        std::process::id()
    ));
    if tree.exists() {
        fs::remove_dir_all(&tree).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(tree.join(".engineering")).expect("its directory");
    fs::write(
        tree.join(".engineering/gates.yaml"),
        "format: engineering-gates/1\nlanguage: canon-expr/1\ngates:\n  \
         - id: probe\n    text: The probe command exits 0.\n    assertions:\n      \
         - 'process.succeeded(\"probe\")'\n",
    )
    .expect("a gates document");
    fs::write(
        tree.join(PROJECT),
        format!(
            "format: engineering-assertions-project/1\ntimeout_seconds: {timeout_seconds}\n\
             commands:\n  probe:\n    executable: {program}\n    args: ['-c', 'exit 0']\n"
        ),
    )
    .expect("a project file");
    git(&tree, &["init", "-q", "-b", "main"]);
    git(&tree, &["add", "-A"]);
    git(&tree, &["commit", "-q", "-m", "start"]);
    Scratch(tree)
}

/// A harness that counts the times it is asked for a step, and takes none.
#[derive(Default)]
struct Counting {
    calls: Cell<usize>,
}

impl Harness for Counting {
    fn name(&self) -> &str {
        "counting"
    }

    fn step(&self, context: &StepContext<'_>) -> Result<StepReport, anyhow::Error> {
        self.calls.set(self.calls.get() + 1);
        Err(anyhow!(
            "the counting harness takes no step among {}",
            context.offered.join(", ")
        ))
    }
}

fn options(id: &str) -> RunOptions {
    RunOptions {
        protocol: PROTOCOL.to_owned(),
        intent: "run the gate".to_owned(),
        inputs: Vec::new(),
        iterations: Some(4),
        hours: None,
        tokens: None,
        case_id: Some(id.to_owned()),
        gate: None,
    }
}

/// The case directories under the tree's cases directory.
fn cases(tree: &Path) -> Vec<String> {
    match fs::read_dir(tree.join(takt::case::CASES_DIR)) {
        Ok(entries) => entries
            .map(|entry| entry.expect("an entry"))
            .filter(|entry| entry.path().join("case.yaml").is_file())
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect(),
        Err(_) => Vec::new(),
    }
}

/// Scenario `gates-project/timeout-out-of-range-refused-at-open`: `timeout_seconds: 1800`, which
/// the gate's preparation refuses (`1..300`), refuses the case when it opens: an error naming
/// the project file and `timeout_seconds`, the harness asked 0 times, no step taken, no case
/// left open, and the tree back on the branch it was on.
#[test]
fn a_project_file_the_gate_would_refuse_refuses_the_case_at_open() {
    let tree = repository("timeout-1800", 1800, "sh");
    let harness = Counting::default();
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(GateRun::new(".engineering/gates.yaml"))];
    let error = match Run::open(&tree, options("timeout-1800"), &harness, &validators) {
        Ok(_) => panic!("the case opened over a project file the gate refuses"),
        Err(error) => format!("{error:#}"),
    };
    println!("refused at open: {error}");
    assert!(error.contains(PROJECT), "names the project file: {error}");
    assert!(
        error.contains("timeout_seconds"),
        "names timeout_seconds: {error}"
    );
    assert_eq!(harness.calls.get(), 0, "the harness was asked for a step");
    assert!(
        cases(&tree).is_empty(),
        "a case was left: {:?}",
        cases(&tree)
    );
    assert_eq!(git(&tree, &["branch", "--show-current"]), "main");
}

/// Scenario `gates-project/valid-file-opens`: a valid project file (`timeout_seconds: 60`)
/// opens the case as before; its gate runs and the case completes, the harness never asked.
#[test]
fn a_valid_project_file_opens_the_case_as_before() {
    let tree = repository("timeout-60", 60, "sh");
    let harness = Counting::default();
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(GateRun::new(".engineering/gates.yaml"))];
    let mut run =
        Run::open(&tree, options("timeout-60"), &harness, &validators).expect("the case opens");
    assert_eq!(cases(&tree), ["timeout-60"]);
    let mut out = Vec::new();
    let stop = run.run(&mut out).expect("the run stops without an error");
    let text = String::from_utf8(out).expect("utf-8");
    assert_eq!(stop, Stop::Completed("accepted".to_owned()), "{text}");
    assert_eq!(harness.calls.get(), 0, "{text}");
}

/// Scenario `gates-project/program-found-nowhere-refused-at-open`: a command whose bare program
/// name is on no directory of the path the gate searches refuses the case when it opens, naming
/// the project file and the program, the harness asked 0 times and no case left open.
#[test]
fn a_program_found_nowhere_refuses_the_case_at_open() {
    let tree = repository("nowhere", 60, "takt-no-such-program");
    let harness = Counting::default();
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(GateRun::new(".engineering/gates.yaml"))];
    let error = match Run::open(&tree, options("nowhere"), &harness, &validators) {
        Ok(_) => panic!("the case opened over a program the gate finds nowhere"),
        Err(error) => format!("{error:#}"),
    };
    assert!(error.contains(PROJECT), "names the project file: {error}");
    assert!(
        error.contains("takt-no-such-program"),
        "names the program: {error}"
    );
    assert_eq!(harness.calls.get(), 0, "the harness was asked for a step");
    assert!(
        cases(&tree).is_empty(),
        "a case was left: {:?}",
        cases(&tree)
    );
}
