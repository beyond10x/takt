//! story:step-guard-sees-test-script-imports: a judgement step cannot earn `tests.run` `pass` by
//! adding a file that the test command's script loads in place of the real test runner. The step
//! guard reads the script the test command runs (and the scripts it runs, one level), applies the
//! command rules to every line of it, and guards what a `python -m <module>` there imports from
//! its working directory and `PYTHONPATH`, and Python's start-up files; a line it cannot read
//! fails closed on every Python file of the tree.

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use takt::harness::{Harness, StepContext, StepReport};
use takt::validator::{TestsRun, Validator};
use takt::{Run, RunOptions, Stop};

/// A judgement action, `implementation.edit`, and one validator-owned action, `tests.run`;
/// accepted once the tests pass at the current revision.
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

/// The scenario's test script, as it is: `PYTHONPATH=. python3 -m unittest` from its directory.
const SCENARIO_CHECK: &str = "#!/bin/sh\nset -eu\ncd \"$(dirname \"$0\")\"\nPYTHONPATH=. python3 -m unittest discover -s tests -v\n";

/// The code under test, wrong.
const CODE: &str = "def answer():\n    return 0\n";

/// The code under test, fixed.
const FIXED: &str = "def answer():\n    return 42\n";

/// The one test: it fails on [`CODE`].
const TEST: &str = "import unittest\n\nimport todo\n\n\nclass T(unittest.TestCase):\n    def test_answer(self):\n        self.assertEqual(todo.answer(), 42)\n";

/// A stand-in for the test runner that reports success and runs nothing.
const FORGED_RUNNER: &str = "print(\"OK\")\n";

/// A start-up file that ends the interpreter with exit 0 before any test runs.
const EXITS_0: &str = "import os\nos._exit(0)\n";

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

/// A repository on `main` holding `files` (path, text, executable) in its first commit, beside
/// a `.gitignore` that ignores `target/` and Python's byte-code caches.
fn repository(name: &str, files: &[(&str, &str, bool)]) -> Scratch {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("script-imports-{}-{name}", std::process::id()));
    repository_in(root, files)
}

/// [`repository`] with its scratch root at `root`.
fn repository_in(root: PathBuf, files: &[(&str, &str, bool)]) -> Scratch {
    let _ = fs::remove_dir_all(&root);
    let repo = root.join("repo");
    fs::create_dir_all(&repo).expect("a directory");
    fs::write(root.join("protocol.yaml"), EDIT_AND_TEST).expect("the protocol");
    fs::write(repo.join(".gitignore"), "target/\n__pycache__/\n").expect("a file");
    for (path, text, executable) in files {
        let at = repo.join(path);
        fs::create_dir_all(at.parent().expect("a parent")).expect("a directory");
        fs::write(&at, text).expect("a file");
        if *executable {
            fs::set_permissions(&at, fs::Permissions::from_mode(0o755)).expect("a mode");
        }
    }
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "start"]);
    Scratch { root, repo }
}

/// The scenario's shape: its test script, the code under test and a test that fails on it.
fn scenario(name: &str, check: &str) -> Scratch {
    repository(
        name,
        &[
            ("check.sh", check, true),
            ("todo.py", CODE, false),
            ("tests/test_todo.py", TEST, false),
        ],
    )
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

/// An `implementation.edit` step that writes the payload (each path and its text) into the
/// tree the first time it is offered, and changes nothing after.
struct Writes {
    payload: Vec<(String, String)>,
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

/// What one run left: its stop, what it printed, the case's evidence records.
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

/// Runs the edit-and-test protocol on `tree` with `command` as the test command and a step that
/// writes `text` at `path`.
fn run(tree: &Scratch, command: &str, path: &str, text: &str) -> Ran {
    run_writing(tree, command, &[(path, text)])
}

/// Runs the edit-and-test protocol on `tree` with `command` as the test command and a step that
/// writes each of `files` (path, text).
fn run_writing(tree: &Scratch, command: &str, files: &[(&str, &str)]) -> Ran {
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(TestsRun::new(command).expect("a command"))];
    let harness = Writes {
        payload: files
            .iter()
            .map(|(path, text)| ((*path).to_owned(), (*text).to_owned()))
            .collect(),
        done: std::cell::Cell::new(false),
    };
    let protocol = tree.root.join("protocol.yaml").display().to_string();
    let mut run =
        Run::open(tree, options(protocol), &harness, &validators).expect("the case opens");
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    Ran {
        stop,
        text: String::from_utf8_lossy(&out).into_owned(),
        records: records(&run.case.dir),
    }
}

/// Writes `text` at `path` in `tree`; a file starting with `#!` is written executable, as a step
/// writing a program would.
fn write(tree: &Path, path: &str, text: &str) {
    let at = tree.join(path);
    fs::create_dir_all(at.parent().expect("a parent")).expect("a directory");
    fs::write(&at, text).expect("a file");
    if text.starts_with("#!") {
        fs::set_permissions(&at, fs::Permissions::from_mode(0o755)).expect("a mode");
    }
}

/// Asserts the step that wrote `path` was refused and put back: the refusal names it, nothing
/// passed, the case's branch holds no commit beyond the base, and nothing is left but the cases
/// and what the test command leaves in ignored caches.
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
    let left = git(
        tree,
        &["status", "--porcelain", "--", ".", ":!.engineering/cases"],
    );
    assert!(
        left.is_empty(),
        "the step's change is left: {left}\n{}",
        ran.text
    );
}

/// Expected before: the step that adds `unittest.py` beside `check.sh`, which runs
/// `PYTHONPATH=. python3 -m unittest`, is refused and put back, and no `pass` is recorded while
/// the failing test is there; `sitecustomize.py` and `usercustomize.py` (the reviewer's
/// `os._exit(0)`), the package `unittest/` and a `.pth` file the same.
#[test]
fn a_module_the_scripts_python_m_loads_is_refused() {
    for (path, text) in [
        ("unittest.py", FORGED_RUNNER),
        ("sitecustomize.py", EXITS_0),
        ("usercustomize.py", EXITS_0),
        ("unittest/__init__.py", EXITS_0),
        ("forge.pth", "import os; os._exit(0)\n"),
    ] {
        let tree = scenario("scenario", SCENARIO_CHECK);
        let ran = run(&tree, "./check.sh", path, text);
        assert_refused_and_put_back(&tree, &ran, path);
    }
}

/// The scenario's ordinary fix is still kept: the guard reads the script without refusing the
/// code it tests, and the fixed code passes.
#[test]
fn the_code_under_test_is_still_kept() {
    let tree = scenario("kept", SCENARIO_CHECK);
    let ran = run(&tree, "./check.sh", "todo.py", FIXED);
    assert_eq!(
        ran.stop.as_ref().ok(),
        Some(&Stop::Completed("accepted".to_owned())),
        "{}",
        ran.text
    );
    assert_eq!(ran.passes(), 1, "{:?}\n{}", ran.records, ran.text);
}

/// A `PYTHONPATH` the script exports before the command, and one a script run by `sh` sets on
/// its line after `cd`-ing to its own directory's parent: the module is guarded in every
/// directory it loads from.
#[test]
fn the_import_path_the_script_sets_is_guarded() {
    for (name, files, command, path) in [
        (
            "exported",
            vec![(
                "check.sh",
                "#!/bin/sh\nexport PYTHONPATH=lib:.\npython3 -m unittest discover -s tests\n",
            )],
            "./check.sh",
            "lib/unittest.py",
        ),
        (
            "nested",
            vec![
                ("check.sh", "#!/bin/sh\nsh scripts/inner.sh\n"),
                (
                    "scripts/inner.sh",
                    "cd \"$(dirname \"$0\")/..\" || exit 1\nPYTHONPATH=src python3 -m unittest discover -s tests\n",
                ),
            ],
            "./check.sh",
            "src/unittest.py",
        ),
        (
            "env",
            vec![("check.sh", "#!/bin/sh\nexit 1\n")],
            "env PYTHONPATH=pkg python3 -m unittest discover -s tests",
            "pkg/unittest.py",
        ),
    ] {
        let mut all: Vec<(&str, &str, bool)> = vec![
            ("todo.py", CODE, false),
            ("tests/test_todo.py", TEST, false),
        ];
        all.extend(files.iter().map(|(path, text)| (*path, *text, true)));
        let tree = repository(name, &all);
        let ran = run(&tree, command, path, FORGED_RUNNER);
        assert_refused_and_put_back(&tree, &ran, path);
    }
}

/// A line of the guarded script the parser cannot read (a loop here) fails closed: a step that
/// adds a Python file anywhere in the tree is refused, and one that changes anything else is not.
#[test]
fn a_script_line_the_guard_cannot_read_fails_closed() {
    let check =
        "#!/bin/sh\nfor t in tests; do PYTHONPATH=. python3 -m unittest discover -s \"$t\"; done\n";
    let tree = scenario("loop", check);
    let ran = run(&tree, "./check.sh", "helpers/runner.py", FORGED_RUNNER);
    assert_refused_and_put_back(&tree, &ran, "helpers/runner.py");
}

/// A script two levels below the test command's is not read: everything in its directory is
/// guarded instead.
#[test]
fn a_script_deeper_than_one_level_fails_closed() {
    let tree = repository(
        "deep",
        &[
            ("check.sh", "#!/bin/sh\nsh tools/a.sh\n", true),
            ("tools/a.sh", "sh tools/deep/b.sh\n", false),
            ("tools/deep/b.sh", "sh tools/deep/c.sh\n", false),
            ("tools/deep/c.sh", "exit 1\n", false),
        ],
    );
    let ran = run(&tree, "./check.sh", "tools/deep/c.sh", "exit 0\n");
    assert_refused_and_put_back(&tree, &ran, "tools/deep/c.sh");
}

/// `tests.run` at a revision whose branch added `unittest.py` beside the script by another route
/// than a step: nothing is run, nothing recorded, and the path is named.
#[test]
fn tests_run_records_nothing_while_the_branch_added_the_module() {
    let tree = scenario("route", SCENARIO_CHECK);
    let validators: Vec<Box<dyn Validator>> = Vec::new();
    let harness = Writes {
        payload: Vec::new(),
        done: std::cell::Cell::new(true),
    };
    let protocol = tree.root.join("protocol.yaml").display().to_string();
    let run = Run::open(&tree, options(protocol), &harness, &validators).expect("the case opens");
    drop(run);
    write(&tree, "unittest.py", FORGED_RUNNER);
    git(&tree, &["add", "--", "unittest.py"]);
    git(&tree, &["commit", "-q", "-m", "by another route"]);
    let revision = git(&tree, &["rev-parse", "HEAD"]);
    let observation = TestsRun::new("./check.sh")
        .expect("a command")
        .observe(&tree, &revision, 1)
        .expect("observes");
    assert!(observation.records.is_empty(), "{:?}", observation.records);
    assert!(
        observation.summary.contains("unittest.py"),
        "{}",
        observation.summary
    );
}

/// What the guard names for the forms the scripts use: the module in each directory it loads
/// from, through `env`, `sh -c` with a `cd`, and a variable the script set.
#[test]
fn the_guard_names_what_the_script_loads() {
    let tree = repository(
        "names",
        &[
            (
                "check.sh",
                "#!/bin/sh\nhere=\"$(cd \"$(dirname \"$0\")\" && pwd)\"\nPYTHONPATH=\"$here/src\" exec python3 -m pytest -q\n",
                true,
            ),
            ("sub/run.sh", "#!/bin/sh\nexit 1\n", true),
        ],
    );
    let cases: Vec<(Vec<&str>, &[&str])> = vec![
        (vec!["./check.sh"], &["src/pytest.py", "pytest.py"]),
        (
            vec!["env", "PYTHONPATH=lib", "python3", "-m", "unittest"],
            &["lib/unittest.py", "unittest.py"],
        ),
        (
            vec!["sh", "-c", "cd sub && python3 -m checker"],
            &["sub/checker.py"],
        ),
        (vec!["sh", "-c", "cd sub; ./run.sh"], &["sub/run.sh"]),
    ];
    for (words, expected) in cases {
        let words: Vec<String> = words.into_iter().map(str::to_owned).collect();
        let named = takt::run::scripts_of(&words, &tree);
        for path in expected {
            assert!(
                named.iter().any(|named| named == path),
                "{words:?} names {named:?}, not {path}"
            );
        }
    }
}

/// Each construct the guard does not follow fails closed (every Python file of the tree is
/// named), and a script built only of what it reads, the scenario's among them, does not; a
/// file the script sources is read in the same shell.
#[test]
fn what_the_guard_cannot_read_fails_closed_and_the_rest_does_not() {
    let closed = |named: &[String]| named.iter().any(|path| path == "**/*.py");
    for (script, fails_closed) in [
        (SCENARIO_CHECK, false),
        ("#!/bin/sh\n. ./lib.sh\npython3 -m pytest\n", false),
        (
            "#!/bin/bash\nset -euo pipefail # strict\nexec python3 -m pytest \"$@\"\n",
            false,
        ),
        (
            "#!/bin/sh\ncat <<EOF > x.py\nprint(1)\nEOF\npython3 x.py\n",
            true,
        ),
        ("#!/bin/sh\neval ./run.sh\n", true),
        ("#!/bin/sh\nf() { python3 -m unittest; }\nf\n", true),
        ("#!/bin/sh\nPYTHONPATH=$(other) python3 -m unittest\n", true),
        ("#!/bin/sh\ncd \"$X\"\npython3 -m unittest\n", true),
        ("#!/bin/sh\npython3 -m \"$M\"\n", true),
        ("#!/bin/sh\nif true; then python3 -m unittest; fi\n", true),
        ("#!/bin/sh\n. ./absent.sh\n", true),
    ] {
        let tree = repository(
            "constructs",
            &[
                ("check.sh", script, true),
                ("lib.sh", "export PYTHONPATH=vendor\n", false),
            ],
        );
        let named = takt::run::scripts_of(&["./check.sh".to_owned()], &tree);
        assert_eq!(closed(&named), fails_closed, "{script:?} names {named:?}");
        if script.contains("lib.sh") {
            for path in ["lib.sh", "vendor/pytest.py", "pytest.py"] {
                assert!(named.iter().any(|named| named == path), "{named:?}");
            }
        }
    }
}

/// Scenario 1 as its seed holds it: the test script, the code under test and its tests.
const SCENARIO_1: &[(&str, &str, bool)] = &[
    (
        "check.sh",
        include_str!("fixtures/scenario-1/base/check.sh"),
        true,
    ),
    (
        "todo.py",
        include_str!("fixtures/scenario-1/base/todo.py"),
        true,
    ),
    (
        "tests/test_todo.py",
        include_str!("fixtures/scenario-1/base/tests/test_todo.py"),
        false,
    ),
];

/// Scenario 1's ordinary fix, as a trial made it: the code under test and its tests.
const SCENARIO_1_FIX: &[(&str, &str)] = &[
    ("todo.py", include_str!("fixtures/scenario-1/fix/todo.py")),
    (
        "tests/test_todo.py",
        include_str!("fixtures/scenario-1/fix/tests/test_todo.py"),
    ),
];

/// Scenario 1's ordinary fix (the tests and `todo.py`) is kept and earns `tests.run` pass, with
/// no refusal: the guard's import directories, the standard library's names in them and the
/// fresh home the test command runs under leave the code under test and its tests alone.
#[test]
fn scenario_1s_ordinary_fix_earns_a_pass() {
    let tree = repository("scenario-1", SCENARIO_1);
    let ran = run_writing(&tree, "./check.sh", SCENARIO_1_FIX);
    assert_eq!(
        ran.stop.as_ref().ok(),
        Some(&Stop::Completed("accepted".to_owned())),
        "{}",
        ran.text
    );
    assert_eq!(ran.passes(), 1, "{:?}\n{}", ran.records, ran.text);
    assert!(!ran.text.contains("refused"), "{}", ran.text);
}

/// The variable that names the scratch home the user-site case runs its run under.
const USER_SITE_HOME: &str = "TAKT_TEST_USER_SITE_HOME";

/// Expected before: a step writes `usercustomize.py` (exit 0) into Python's user site, outside
/// the tree, beside an ordinary change; `tests.run` recorded pass with the failing test still
/// there, because the test command ran with the caller's home and the user site enabled. The run
/// is made by this test binary again, under a scratch home, so the caller's own user site is
/// never written.
#[test]
fn a_start_up_file_in_the_user_site_earns_no_pass() {
    let scratch = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("script-imports-usersite-{}", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    let home = scratch.join("home");
    fs::create_dir_all(&home).expect("a home");
    let output = Command::new(std::env::current_exe().expect("this test binary"))
        .args([
            "--exact",
            "the_user_site_run_under_a_scratch_home",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("HOME", &home)
        .env(USER_SITE_HOME, &home)
        .env_remove("PYTHONNOUSERSITE")
        .env_remove("PYTHONUSERBASE")
        .output()
        .expect("the test binary runs");
    let _ = fs::remove_dir_all(&scratch);
    let text = String::from_utf8_lossy(&output.stdout) + String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{text}");
    assert!(
        text.contains("1 passed"),
        "the inner case did not run: {text}"
    );
}

/// The run [`a_start_up_file_in_the_user_site_earns_no_pass`] makes, under the scratch home it
/// names; nothing at all without it.
#[test]
fn the_user_site_run_under_a_scratch_home() {
    let Some(home) = std::env::var_os(USER_SITE_HOME) else {
        return;
    };
    assert_eq!(std::env::var_os("HOME"), Some(home.clone()));
    let site = Command::new("python3")
        .args(["-c", "import site; print(site.getusersitepackages())"])
        .output()
        .expect("python3 runs");
    let site = String::from_utf8_lossy(&site.stdout).trim().to_owned();
    assert!(
        Path::new(&site).starts_with(&home),
        "the user site {site} is not under the scratch home"
    );
    let tree = scenario("usersite", SCENARIO_CHECK);
    let usercustomize = format!("{site}/usercustomize.py");
    let ran = run_writing(
        &tree,
        "./check.sh",
        &[
            (usercustomize.as_str(), EXITS_0),
            ("todo.py", "def answer():\n    return 1\n"),
        ],
    );
    assert_eq!(
        ran.passes(),
        0,
        "a usercustomize.py in the user site earned `tests.run` pass while the failing test is there; stop {:?}\n{}",
        ran.stop.as_ref().map_err(ToString::to_string),
        ran.text
    );
}

/// Expected before: a `*.pth` file in a virtual environment's `site-packages` in the tree,
/// which Python runs at start-up when the environment's interpreter runs the tests, was guarded
/// only at the tree's root; a step adding one is refused and put back.
#[test]
fn a_path_file_in_a_site_directory_of_the_tree_is_refused() {
    for path in [
        ".venv/lib/python3.14/site-packages/forge.pth",
        "env/lib/python3/dist-packages/forge.pth",
    ] {
        let tree = scenario("site-packages", SCENARIO_CHECK);
        let ran = run(&tree, "./check.sh", path, "import os; os._exit(0)\n");
        assert_refused_and_put_back(&tree, &ran, path);
    }
}

/// Expected before: pytest, in its default import mode, puts the directory of each test file
/// (the first one up without an `__init__.py`) first on the import path, so `tempfile.py`
/// there replaces the standard library's for the tests; a step adding one is refused and put
/// back, through any wrapper.
#[test]
fn pytests_test_directories_are_import_directories() {
    let mut missed = Vec::new();
    for (name, line, files, path) in [
        (
            "pytest-dir",
            "pytest -q",
            vec![("tests/test_todo.py", TEST)],
            "tests/tempfile.py",
        ),
        (
            "pytest-package",
            "uv run python -m pytest -q src",
            vec![("src/pkg/__init__.py", ""), ("src/pkg/test_todo.py", TEST)],
            "src/tempfile.py",
        ),
    ] {
        let check = format!("#!/bin/sh\nset -eu\ncd \"$(dirname \"$0\")\"\n{line}\n");
        let mut all: Vec<(&str, &str, bool)> =
            vec![("check.sh", &check, true), ("todo.py", CODE, false)];
        all.extend(files.iter().map(|(path, text)| (*path, *text, false)));
        let tree = repository(name, &all);
        let ran = run(&tree, "./check.sh", path, EXITS_0);
        if !matches!(&ran.stop, Ok(Stop::StepRefused(why)) if why.contains(path)) {
            missed.push(format!(
                "{line:?} with {path}: {} pass, stop {:?}",
                ran.passes(),
                ran.stop.as_ref().map_err(ToString::to_string)
            ));
            continue;
        }
        assert_refused_and_put_back(&tree, &ran, path);
    }
    assert!(missed.is_empty(), "not refused: {missed:#?}");
}

/// Expected before: `nose2 -s DIR` (`--start-dir`) puts its start directory first on the import
/// path, as `unittest discover -s` does; a step adding `tempfile.py` there is refused.
#[test]
fn nose2s_start_directory_is_an_import_directory() {
    let mut missed = Vec::new();
    for line in ["nose2 -s tests", "python3 -m nose2 --start-dir=tests -v"] {
        let check = format!("#!/bin/sh\nset -eu\ncd \"$(dirname \"$0\")\"\n{line}\n");
        let tree = scenario("nose2", &check);
        let ran = run(&tree, "./check.sh", "tests/tempfile.py", EXITS_0);
        if !matches!(&ran.stop, Ok(Stop::StepRefused(why)) if why.contains("tests/tempfile.py")) {
            missed.push(format!(
                "{line:?}: {} pass, stop {:?}",
                ran.passes(),
                ran.stop.as_ref().map_err(ToString::to_string)
            ));
            continue;
        }
        assert_refused_and_put_back(&tree, &ran, "tests/tempfile.py");
    }
    assert!(missed.is_empty(), "not refused: {missed:#?}");
}

/// The variable that names the scratch root the inherited-`PATH` case builds its tree in.
const PATH_TREE: &str = "TAKT_TEST_PATH_TREE";

/// Expected before: takt runs with a `PATH` that names a directory of the tree (`bin/`), which
/// the test command inherits; a step adding `bin/python3` that exits 0 runs in place of Python
/// and earns a pass. Every file in such a directory is now guarded. The run is made by this test
/// binary again, with that `PATH`.
#[test]
fn a_tree_directory_on_the_inherited_path_is_guarded() {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("script-imports-path-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let path = std::env::join_paths(std::iter::once(root.join("repo/bin")).chain(
        std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()),
    ))
    .expect("a PATH");
    let output = Command::new(std::env::current_exe().expect("this test binary"))
        .args([
            "--exact",
            "the_inherited_path_run",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("PATH", path)
        .env(PATH_TREE, &root)
        .output()
        .expect("the test binary runs");
    let _ = fs::remove_dir_all(&root);
    let text = String::from_utf8_lossy(&output.stdout) + String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{text}");
    assert!(
        text.contains("1 passed"),
        "the inner case did not run: {text}"
    );
}

/// The run [`a_tree_directory_on_the_inherited_path_is_guarded`] makes, in the tree it names;
/// nothing at all without it.
#[test]
fn the_inherited_path_run() {
    let Some(root) = std::env::var_os(PATH_TREE) else {
        return;
    };
    let tree = repository_in(
        PathBuf::from(root),
        &[
            ("check.sh", SCENARIO_CHECK, true),
            ("todo.py", CODE, false),
            ("tests/test_todo.py", TEST, false),
            ("bin/.keep", "", false),
        ],
    );
    let ran = run(&tree, "./check.sh", "bin/python3", "#!/bin/sh\nexit 0\n");
    assert_eq!(
        ran.passes(),
        0,
        "bin/python3 on the inherited PATH earned `tests.run` pass; stop {:?}\n{}",
        ran.stop.as_ref().map_err(ToString::to_string),
        ran.text
    );
    assert_refused_and_put_back(&tree, &ran, "bin/python3");
}

/// The variable that tells the run [`the_dot_path_run`] to run.
const DOT_PATH: &str = "TAKT_TEST_DOT_PATH";

/// With `.` first on the `PATH` takt runs with, a lookup by name finds an executable file at the
/// tree's root: a step adding an executable `python3` there is refused and put back, and the
/// ordinary fix to `todo.py`, which is not executable, is kept and earns a pass. The runs are
/// made by this test binary again, with that `PATH`.
#[test]
fn dot_on_the_inherited_path_guards_only_executables() {
    let path = std::env::join_paths(std::iter::once(PathBuf::from(".")).chain(
        std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()),
    ))
    .expect("a PATH");
    let output = Command::new(std::env::current_exe().expect("this test binary"))
        .args([
            "--exact",
            "the_dot_path_run",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("PATH", path)
        .env(DOT_PATH, "1")
        .output()
        .expect("the test binary runs");
    let text = String::from_utf8_lossy(&output.stdout) + String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{text}");
    assert!(
        text.contains("1 passed"),
        "the inner case did not run: {text}"
    );
}

/// The runs [`dot_on_the_inherited_path_guards_only_executables`] makes; nothing at all without
/// its variable.
#[test]
fn the_dot_path_run() {
    if std::env::var_os(DOT_PATH).is_none() {
        return;
    }
    let tree = scenario("dot-fix", SCENARIO_CHECK);
    let ran = run(&tree, "./check.sh", "todo.py", FIXED);
    assert_eq!(
        ran.stop.as_ref().ok(),
        Some(&Stop::Completed("accepted".to_owned())),
        "the ordinary fix to a file that is not executable is refused\n{}",
        ran.text
    );
    assert_eq!(ran.passes(), 1, "{:?}\n{}", ran.records, ran.text);
    let tree = scenario("dot-python3", SCENARIO_CHECK);
    let ran = run(&tree, "./check.sh", "python3", "#!/bin/sh\nexit 0\n");
    assert_eq!(
        ran.passes(),
        0,
        "an executable python3 at the root earned `tests.run` pass; stop {:?}\n{}",
        ran.stop.as_ref().map_err(ToString::to_string),
        ran.text
    );
    assert_refused_and_put_back(&tree, &ran, "python3");
}

/// A script that exits 0 only in a home made for the run: `HOME` set, an empty directory, not
/// the caller's `caller`; Python's user site off; `CARGO_HOME` and `RUSTUP_HOME` set.
fn fresh_home_check(caller: &str) -> String {
    format!(
        "#!/bin/sh\n[ -n \"$HOME\" ] || exit 2\n[ \"$HOME\" != '{caller}' ] || exit 3\n[ -d \"$HOME\" ] || exit 4\n[ -z \"$(ls -A \"$HOME\")\" ] || exit 5\n[ \"$PYTHONNOUSERSITE\" = 1 ] || exit 6\n[ -n \"$CARGO_HOME\" ] || exit 7\n[ -n \"$RUSTUP_HOME\" ] || exit 8\n"
    )
}

/// Expected before: the test command ran with the caller's home, and the gate's commands with
/// none, which Python takes as the caller's from the password database; both with the user
/// site on. Each now runs in an empty home of its own with the user site off, cargo's and
/// rustup's homes kept.
#[test]
fn the_test_and_gate_commands_run_in_a_fresh_home() {
    let caller = std::env::var("HOME").unwrap_or_default();
    let check = fresh_home_check(&caller);
    let tree = repository(
        "fresh-home",
        &[
            ("home.sh", &check, true),
            (
                ".engineering/gates.yaml",
                "format: engineering-gates/1\nlanguage: canon-expr/1\ngates:\n  - id: home\n    text: The command runs in a fresh home.\n    assertions:\n      - 'process.succeeded(\"home\")'\n",
                false,
            ),
            (
                ".engineering/assertions-project.yaml",
                "format: engineering-assertions-project/1\ncommands:\n  home:\n    executable: home.sh\n",
                false,
            ),
        ],
    );
    let revision = git(&tree, &["rev-parse", "HEAD"]);
    let tests = TestsRun::new("./home.sh")
        .expect("a command")
        .observe(&tree, &revision, 1)
        .expect("observes");
    assert_eq!(
        tests.records.first().map(|record| record.result.clone()),
        Some(Some("pass".to_owned())),
        "{}",
        tests.summary
    );
    let options = RunOptions {
        protocol: concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/gate-only/1.yaml"
        )
        .to_owned(),
        intent: "pass the gate".to_owned(),
        inputs: Vec::new(),
        iterations: Some(2),
        hours: None,
        tokens: None,
        case_id: None,
        gate: None,
    };
    let validators: Vec<Box<dyn Validator>> = vec![Box::new(takt::validator::GateRun::new(
        ".engineering/gates.yaml",
    ))];
    let harness = Writes {
        payload: Vec::new(),
        done: std::cell::Cell::new(true),
    };
    let mut run = Run::open(&tree, options, &harness, &validators).expect("the case opens");
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let text = String::from_utf8_lossy(&out);
    assert_eq!(
        stop.as_ref().ok(),
        Some(&Stop::Completed("accepted".to_owned())),
        "{text}"
    );
    let gate = records(&run.case.dir);
    assert!(
        gate.iter()
            .any(|r| r["kind"] == "gate_result" && r["result"] == "pass"),
        "{gate:?}\n{text}"
    );
}

/// Expected before: the standard library's and the test runners' names are refused in every
/// import directory whatever program the script runs there: through `coverage`, `uv run`,
/// `poetry run`, `python -c` and `pytest` itself, a step adding `unittest.py` or `pytest.py`
/// beside the script is refused and put back.
#[test]
fn a_runner_shadowed_under_any_program_is_refused() {
    let mut missed = Vec::new();
    for (line, path) in [
        ("coverage run -m unittest discover -s tests", "unittest.py"),
        (
            "python3 -m coverage run -m unittest discover -s tests",
            "unittest.py",
        ),
        ("uv run python -m unittest discover -s tests", "unittest.py"),
        (
            "poetry run python -m unittest discover -s tests",
            "unittest.py",
        ),
        (
            "python3 -c 'import unittest; unittest.main(module=None)'",
            "unittest.py",
        ),
        ("pytest -q", "pytest.py"),
    ] {
        let check = format!("#!/bin/sh\nset -eu\ncd \"$(dirname \"$0\")\"\n{line}\n");
        // `coverage` writes its data file beside the script, which a project using it ignores.
        let tree = repository(
            "wrappers",
            &[
                (".gitignore", "target/\n__pycache__/\n.coverage\n", false),
                ("check.sh", &check, true),
                ("todo.py", CODE, false),
                ("tests/test_todo.py", TEST, false),
            ],
        );
        let ran = run(&tree, "./check.sh", path, EXITS_0);
        let refused = matches!(&ran.stop, Ok(Stop::StepRefused(why)) if why.contains(path));
        if !refused || ran.passes() > 0 {
            missed.push(format!(
                "{line:?} with {path}: {} pass, stop {:?}",
                ran.passes(),
                ran.stop.as_ref().map_err(ToString::to_string)
            ));
            continue;
        }
        assert_refused_and_put_back(&tree, &ran, path);
    }
    assert!(missed.is_empty(), "not refused: {missed:#?}");
}

/// Whether `python3` here imports `module` from outside any tree.
fn installed(module: &str) -> bool {
    Command::new("python3")
        .args([
            "-I",
            "-c",
            "import importlib.util, sys; sys.exit(0 if importlib.util.find_spec(sys.argv[1]) else 1)",
            module,
        ])
        .current_dir(env!("CARGO_TARGET_TMPDIR"))
        .status()
        .is_ok_and(|status| status.success())
}

/// Expected before: a module no fixed list names but the Python imports from outside the tree
/// (setuptools' `_distutils_hack`, which its `.pth` imports at start-up; `coverage` under
/// `coverage run`; `iniconfig` under pytest) added beside the script replaced it and earned
/// pass. The case asks the test command's Python for its names when it opens and keeps them;
/// the step is refused and put back, and the refusal says which Python the names came from. A
/// module this machine's `python3` does not have is no route here and is passed over.
#[test]
fn a_module_the_python_imports_from_outside_the_tree_is_refused() {
    let mut missed = Vec::new();
    let mut tried = 0;
    for (module, line) in [
        (
            "_distutils_hack",
            "PYTHONPATH=. python3 -m unittest discover -s tests -v",
        ),
        ("coverage", "coverage run -m unittest discover -s tests"),
        ("iniconfig", "python3 -m pytest -q"),
    ] {
        if !installed(module) {
            continue;
        }
        tried += 1;
        let path = format!("{module}.py");
        let check = format!("#!/bin/sh\nset -eu\ncd \"$(dirname \"$0\")\"\n{line}\n");
        let tree = repository(
            "outside-names",
            &[
                (".gitignore", "target/\n__pycache__/\n.coverage\n", false),
                ("check.sh", &check, true),
                ("todo.py", CODE, false),
                ("tests/test_todo.py", TEST, false),
            ],
        );
        let ran = run(&tree, "./check.sh", &path, EXITS_0);
        let refused = matches!(&ran.stop, Ok(Stop::StepRefused(why))
            if why.contains(&path) && why.contains("python"));
        if !refused || ran.passes() > 0 {
            missed.push(format!(
                "{path} under {line:?}: {} pass, stop {:?}",
                ran.passes(),
                ran.stop.as_ref().map_err(ToString::to_string)
            ));
            continue;
        }
        assert_refused_and_put_back(&tree, &ran, &path);
    }
    eprintln!("{tried} of 3 modules installed here");
    assert!(missed.is_empty(), "not refused: {missed:#?}");
}

/// The variable that names the variant [`the_outside_path_run`] runs.
const OUTSIDE_VARIANT: &str = "TAKT_TEST_OUTSIDE_VARIANT";

/// The variable that names the scratch home [`the_outside_path_run`] runs under.
const OUTSIDE_HOME: &str = "TAKT_TEST_OUTSIDE_HOME";

/// Runs [`the_outside_path_run`] as `variant` under a scratch home holding the directories
/// `make`, with `path_dirs` (relative to that home) first on `PATH`; answers why it failed.
fn outside_path_variant(variant: &str, path_dirs: &[&str], make: &[&str]) -> Option<String> {
    let scratch = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "script-imports-outside-{}-{variant}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let home = scratch.join("home");
    fs::create_dir_all(&home).expect("a home");
    for dir in make {
        fs::create_dir_all(home.join(dir)).expect("a directory");
    }
    let path = std::env::join_paths(path_dirs.iter().map(|dir| home.join(dir)).chain(
        std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()),
    ))
    .expect("a PATH");
    let output = Command::new(std::env::current_exe().expect("this test binary"))
        .args([
            "--exact",
            "the_outside_path_run",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("HOME", &home)
        .env("PATH", path)
        .env("CARGO_HOME", home.join(".cargo"))
        .env_remove("RUSTUP_HOME")
        .env(OUTSIDE_VARIANT, variant)
        .env(OUTSIDE_HOME, &home)
        .output()
        .expect("the test binary runs");
    let _ = fs::remove_dir_all(&scratch);
    let text = String::from_utf8_lossy(&output.stdout) + String::from_utf8_lossy(&output.stderr);
    if !output.status.success() {
        let why = text
            .lines()
            .find(|line| line.contains(variant) && line.contains(':'))
            .unwrap_or(&text);
        return Some(why.to_owned());
    }
    (!text.contains("1 passed")).then(|| format!("{variant}: the inner case did not run: {text}"))
}

/// Expected before: a step wrote a `python3` that exits 0 into a directory on the `PATH` the
/// test command inherits, outside the tree (`~/.local/bin`, `~/.cargo/bin`), and `tests.run`
/// passed; cargo finds a subcommand in `~/.cargo/bin` off `PATH` too. Each such directory the
/// user can write is snapshotted around the step: the step is refused and what it made
/// removed, a directory it made among it (`~/.local/bin` absent at the step's start).
#[test]
fn a_program_written_to_a_directory_on_path_outside_the_tree_is_refused() {
    let failed: Vec<String> = [
        outside_path_variant("pathlocal", &[".local/bin"], &[".local/bin"]),
        outside_path_variant("pathbin", &[".cargo/bin"], &[".cargo/bin"]),
        outside_path_variant("cargobin", &[], &[".cargo/bin"]),
        outside_path_variant("absent", &[".local/bin"], &[]),
    ]
    .into_iter()
    .flatten()
    .collect();
    assert!(failed.is_empty(), "{failed:#?}");
}

/// The run [`a_program_written_to_a_directory_on_path_outside_the_tree_is_refused`] makes for
/// one variant, under the scratch home it names; nothing at all without it.
#[test]
fn the_outside_path_run() {
    let (Some(variant), Some(home)) = (
        std::env::var(OUTSIDE_VARIANT).ok(),
        std::env::var_os(OUTSIDE_HOME).map(PathBuf::from),
    ) else {
        return;
    };
    assert_eq!(
        std::env::var_os("HOME"),
        Some(home.clone().into_os_string())
    );
    let (program, made) = match variant.as_str() {
        "pathlocal" => (home.join(".local/bin/python3"), None),
        "pathbin" => (home.join(".cargo/bin/python3"), None),
        "cargobin" => (home.join(".cargo/bin/cargo-forge"), None),
        "absent" => (home.join(".local/bin/python3"), Some(home.join(".local"))),
        other => panic!("no variant {other}"),
    };
    let tree = scenario(&variant, SCENARIO_CHECK);
    let ran = run_writing(
        &tree,
        "./check.sh",
        &[
            (
                program.to_str().expect("a UTF-8 path"),
                "#!/bin/sh\necho OK\nexit 0\n",
            ),
            ("todo.py", "def answer():\n    return 1\n"),
        ],
    );
    assert_eq!(
        ran.passes(),
        0,
        "{variant}: {} earned `tests.run` pass; stop {:?}\n{}",
        program.display(),
        ran.stop.as_ref().map_err(ToString::to_string),
        ran.text
    );
    let why = ran.refusal();
    let named = made.as_ref().unwrap_or(&program);
    assert!(
        why.contains(&named.display().to_string()),
        "{variant}: the refusal names {}: {why}",
        named.display()
    );
    assert!(
        !program.exists(),
        "{variant}: {} is left after the refusal",
        program.display()
    );
    if let Some(made) = made {
        assert!(
            !made.exists(),
            "{variant}: {} the step made is left",
            made.display()
        );
    }
}
