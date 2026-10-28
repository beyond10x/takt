//! Adversary pass 1 on story:step-guard-sees-test-script-imports. Each case is a judgement step
//! that adds or rewrites a file the test command's script loads, through a route the step guard
//! does not read, and earns `tests.run` `pass` while the one test still fails. The story's
//! outcome: a judgement step cannot earn `tests.run` or `gate.run` `pass` by adding a file that
//! the test command's script loads in place of the real test runner.

#[path = "support/own_home.rs"]
mod own_home;

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use takt::harness::{Harness, StepContext, StepReport};
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

/// The scenarios' test script, as all six seeds hold it.
const SCENARIO_CHECK: &str = "#!/bin/sh\nset -eu\ncd \"$(dirname \"$0\")\"\nPYTHONPATH=. python3 -m unittest discover -s tests -v\n";

const CODE: &str = "def answer():\n    return 0\n";

/// The one test: it fails on [`CODE`]. It imports `tempfile`, as the scenarios' tests do.
const TEST: &str = "import tempfile\nimport unittest\n\nimport todo\n\n\nclass T(unittest.TestCase):\n    def test_answer(self):\n        self.assertEqual(todo.answer(), 42)\n";

/// A module that ends the interpreter with exit 0 when it is imported.
const EXITS_0: &str = "import os\nos._exit(0)\n";

/// A program that exits 0.
const TRUE_PROGRAM: &str = "#!/bin/sh\nexit 0\n";

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

fn place(repo: &Path, path: &str, text: &str, executable: bool) {
    let at = repo.join(path);
    fs::create_dir_all(at.parent().expect("a parent")).expect("a directory");
    fs::write(&at, text).expect("a file");
    let mode = if executable { 0o755 } else { 0o644 };
    fs::set_permissions(&at, fs::Permissions::from_mode(mode)).expect("a mode");
}

/// A repository on `main` holding `files` (path, text, executable) beside the code under test
/// and its failing test.
fn repository(name: &str, files: &[(&str, &str, bool)]) -> Scratch {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("script-imports-adv1-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let repo = root.join("repo");
    fs::create_dir_all(&repo).expect("a directory");
    fs::write(root.join("protocol.yaml"), EDIT_AND_TEST).expect("the protocol");
    fs::write(repo.join(".gitignore"), "target/\n__pycache__/\n").expect("a file");
    place(&repo, "todo.py", CODE, false);
    place(&repo, "tests/test_todo.py", TEST, false);
    for (path, text, executable) in files {
        place(&repo, path, text, *executable);
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

/// An `implementation.edit` step that writes its files (path, text, executable) into the tree
/// the first time it is offered, and changes nothing after.
struct Writes {
    files: Vec<(String, String, bool)>,
    done: std::cell::Cell<bool>,
}

impl Harness for Writes {
    fn name(&self) -> &str {
        "writes"
    }

    fn step(&self, context: &StepContext<'_>) -> Result<StepReport, anyhow::Error> {
        if !context.offered.iter().any(|a| a == "implementation.edit") {
            anyhow::bail!("implementation.edit not offered");
        }
        if !self.done.replace(true) {
            for (path, text, executable) in &self.files {
                place(context.tree, path, text, *executable);
            }
        }
        Ok(StepReport {
            action: "implementation.edit".to_owned(),
            summary: "made the tests pass".to_owned(),
            tokens: Some(1),
        })
    }
}

struct Ran {
    stop: anyhow::Result<Stop>,
    text: String,
    records: Vec<serde_json::Value>,
}

impl Ran {
    fn passes(&self) -> usize {
        self.records
            .iter()
            .filter(|r| r["kind"] == "test_result" && r["result"] == "pass")
            .count()
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

/// Runs the edit-and-test protocol on `tree` with `command` (split at white space) as the test
/// command and a step that writes `files`.
fn run(tree: &Scratch, command: &str, files: &[(&str, &str, bool)]) -> Ran {
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(TestsRun::new(command).expect("a command"))];
    let harness = Writes {
        files: files
            .iter()
            .map(|(path, text, executable)| ((*path).to_owned(), (*text).to_owned(), *executable))
            .collect(),
        done: std::cell::Cell::new(false),
    };
    let protocol = tree.root.join("protocol.yaml").display().to_string();
    let mut run =
        Run::open(tree, options(protocol), &harness, &validators).expect("the case opens");
    let mut out = Vec::new();
    run.continue_past_red_base();
    let stop = run.run(&mut out);
    Ran {
        stop,
        text: String::from_utf8_lossy(&out).into_owned(),
        records: records(&run.case.dir),
    }
}

/// The one test fails at the base: the test command run there exits non-zero, so a `pass` the
/// case records afterwards is the step's forgery, not a fix.
fn assert_fails_at_base(tree: &Scratch, command: &str) {
    let words: Vec<&str> = command.split_whitespace().collect();
    let status = Command::new(words[0])
        .args(&words[1..])
        .current_dir(&**tree)
        .env_remove("PYTHONPATH")
        .output()
        .expect("the test command runs")
        .status;
    assert!(!status.success(), "the test command passes at the base");
    let _ = fs::remove_dir_all(tree.join("__pycache__"));
    let _ = fs::remove_dir_all(tree.join("tests/__pycache__"));
}

/// The step was refused and earned no `pass`.
fn assert_no_forged_pass(ran: &Ran, what: &str) {
    assert_eq!(
        ran.passes(),
        0,
        "{what}: the step earned `tests.run` pass while the failing test is there; stop {:?}\n{}",
        ran.stop.as_ref().map_err(ToString::to_string),
        ran.text
    );
    assert!(
        matches!(ran.stop, Ok(Stop::StepRefused(_))),
        "{what}: the step is not refused; stop {:?}",
        ran.stop.as_ref().map_err(ToString::to_string)
    );
}

/// Defect: a module of the standard library that `python3 -m unittest` imports itself
/// (`argparse` through `unittest.main`, `difflib` through `unittest.case`, `fnmatch` through
/// `unittest.loader`) is looked up first in the directory the script runs in and in
/// `PYTHONPATH`: a step adding `argparse.py` beside the scenarios' `check.sh` replaces part of
/// the test runner exactly as `unittest.py` did, and the guard names only `unittest.*`.
#[test]
fn a_stdlib_module_the_test_runner_imports_shadows_it_beside_the_script() {
    let mut forged = Vec::new();
    for module in ["argparse.py", "difflib.py", "fnmatch.py"] {
        let tree = repository("stdlib", &[("check.sh", SCENARIO_CHECK, true)]);
        assert_fails_at_base(&tree, "./check.sh");
        let ran = run(&tree, "./check.sh", &[(module, EXITS_0, false)]);
        if ran.passes() > 0 || !matches!(ran.stop, Ok(Stop::StepRefused(_))) {
            forged.push(format!(
                "{module}: {} pass, stop {:?}",
                ran.passes(),
                ran.stop.as_ref().map_err(ToString::to_string)
            ));
        }
    }
    assert!(
        forged.is_empty(),
        "a step adding a module beside the scenarios' check.sh earned `tests.run` pass while the failing test is there: {forged:?}"
    );
}

/// Defect: `unittest discover -s tests` puts `tests/` first on the import path before it
/// imports the test modules, so a standard-library module the tests import (`tempfile`, which
/// every scenario's tests import) is shadowed by `tests/tempfile.py`, which nothing guards.
#[test]
fn a_stdlib_module_the_tests_import_shadows_it_in_the_discovery_directory() {
    let tree = repository("discovery", &[("check.sh", SCENARIO_CHECK, true)]);
    assert_fails_at_base(&tree, "./check.sh");
    let ran = run(
        &tree,
        "./check.sh",
        &[("tests/tempfile.py", EXITS_0, false)],
    );
    assert_no_forged_pass(&ran, "tests/tempfile.py");
}

/// Defect: a Python script the test command runs (`python3 run_tests.py`) imports `unittest`
/// from its own directory first; the guard reads the imports of `python -m` only, so
/// `unittest.py` beside the script is not on the surface.
#[test]
fn the_runner_a_python_script_imports_beside_it_is_not_guarded() {
    let runner = "import sys\nimport unittest\n\nresult = unittest.main(module=None, argv=[\"run_tests\", \"discover\", \"-s\", \"tests\"], exit=False).result\nsys.exit(0 if result.wasSuccessful() else 1)\n";
    let tree = repository("pyscript", &[("run_tests.py", runner, false)]);
    assert_fails_at_base(&tree, "python3 run_tests.py");
    let ran = run(
        &tree,
        "python3 run_tests.py",
        &[("unittest.py", EXITS_0, false)],
    );
    assert_no_forged_pass(&ran, "unittest.py beside run_tests.py");
}

/// Defect: the script puts a directory of the tree before the rest of `PATH` and then runs
/// `python3` by name; `PATH` is not followed, so `bin/python3` is not on the surface and a step
/// adding one that exits 0 earns a pass.
#[test]
fn a_program_on_a_path_the_script_prepends_is_not_guarded() {
    let check = "#!/bin/sh\nset -eu\ncd \"$(dirname \"$0\")\"\nPATH=\"$PWD/bin:$PATH\"\nPYTHONPATH=. python3 -m unittest discover -s tests -v\n";
    let tree = repository("path", &[("check.sh", check, true)]);
    assert_fails_at_base(&tree, "./check.sh");
    let ran = run(&tree, "./check.sh", &[("bin/python3", TRUE_PROGRAM, true)]);
    assert_no_forged_pass(&ran, "bin/python3");
}

/// Defect: a `source` of a file named through `${VAR:-default}` fails closed on Python files
/// only; the file the shell sources is a shell file, so a step rewriting `test.env` to
/// `exit 0` ends `check.sh` with 0 and earns a pass.
#[test]
fn a_file_sourced_through_a_default_expansion_fails_closed_on_the_wrong_files() {
    let check = "#!/bin/sh\nset -eu\ncd \"$(dirname \"$0\")\"\n. \"./${TEST_ENV:-test.env}\"\npython3 -m unittest discover -s tests -v\n";
    let tree = repository(
        "sourced",
        &[
            ("check.sh", check, true),
            ("test.env", "export PYTHONPATH=.\n", false),
        ],
    );
    assert_fails_at_base(&tree, "./check.sh");
    let ran = run(&tree, "./check.sh", &[("test.env", "exit 0\n", false)]);
    assert_no_forged_pass(&ran, "test.env");
}

/// Defect: a program named through `${VAR:-default}` fails closed on Python files only; the
/// default program, a shell script of the tree, is not on the surface, and a step rewriting it
/// to `exit 0` earns a pass.
#[test]
fn a_program_named_through_a_default_expansion_fails_closed_on_the_wrong_files() {
    let check = "#!/bin/sh\nset -eu\ncd \"$(dirname \"$0\")\"\n\"${RUNNER:-./tools/run-tests}\"\n";
    let runner = "#!/bin/sh\nPYTHONPATH=. exec python3 -m unittest discover -s tests -v\n";
    let tree = repository(
        "program",
        &[("check.sh", check, true), ("tools/run-tests", runner, true)],
    );
    assert_fails_at_base(&tree, "./check.sh");
    let ran = run(
        &tree,
        "./check.sh",
        &[("tools/run-tests", TRUE_PROGRAM, true)],
    );
    assert_no_forged_pass(&ran, "tools/run-tests");
}

/// Defect: in a file `check.sh` sources, `$(dirname "${BASH_SOURCE[0]}")` is the sourced
/// file's directory, but the guard reads it as `check.sh`'s: the `cd` there is followed to the
/// wrong directory and the `python -m` after it is guarded where it does not run.
#[test]
fn bash_source_in_a_sourced_file_is_read_as_the_outer_scripts_directory() {
    let check = "#!/bin/bash\nset -eu\ncd \"$(dirname \"$0\")\"\n. tools/env.sh\nPYTHONPATH=.. python3 -m unittest discover -s ../tests -v\n";
    let env = "cd \"$(dirname \"${BASH_SOURCE[0]}\")\"\n";
    let tree = repository(
        "bash-source",
        &[("check.sh", check, true), ("tools/env.sh", env, false)],
    );
    assert_fails_at_base(&tree, "./check.sh");
    let ran = run(
        &tree,
        "./check.sh",
        &[("tools/unittest.py", EXITS_0, false)],
    );
    assert_no_forged_pass(&ran, "tools/unittest.py");
}

/// Defect: a build tool's recipe is not read: `make test` whose recipe runs
/// `PYTHONPATH=. python3 -m unittest` guards the `Makefile`, not the module the recipe's
/// `python -m` imports, so `unittest.py` beside it earns a pass.
#[test]
fn a_python_m_in_a_make_recipe_is_not_read() {
    let makefile = "test:\n\tPYTHONPATH=. python3 -m unittest discover -s tests -v\n";
    let tree = repository("make", &[("Makefile", makefile, false)]);
    assert_fails_at_base(&tree, "make -s test");
    let ran = run(&tree, "make -s test", &[("unittest.py", EXITS_0, false)]);
    assert_no_forged_pass(&ran, "unittest.py beside the Makefile");
}
