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

/// A virtual environment outside any tree, made `--without-pip` from this machine's `python3`,
/// whose `site-packages` holds a `.pth` that imports `venvhook` at start-up and an empty
/// `venvhook.py`: the hermetic stand-in for setuptools' `_distutils_hack` and its kin, the same
/// on every runner. Removed when dropped.
struct Venv {
    root: PathBuf,
}

impl Venv {
    fn new(name: &str) -> Self {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("script-imports-venv-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let made = Command::new("python3")
            .args(["-m", "venv", "--without-pip"])
            .arg(&root)
            .output()
            .expect("python3 runs");
        assert!(
            made.status.success(),
            "python3 -m venv --without-pip makes the fixture: {}",
            String::from_utf8_lossy(&made.stderr)
        );
        let site = Command::new(root.join("bin/python"))
            .args([
                "-I",
                "-c",
                "import sysconfig; print(sysconfig.get_paths()['purelib'])",
            ])
            .output()
            .expect("the venv's python runs");
        let site = PathBuf::from(String::from_utf8_lossy(&site.stdout).trim());
        fs::write(site.join("venvhook.pth"), "import venvhook\n").expect("the .pth");
        fs::write(site.join("venvhook.py"), "").expect("the module");
        Self { root }
    }

    /// The venv's `bin` directory.
    fn bin(&self) -> String {
        self.root.join("bin").display().to_string()
    }

    /// The venv's `site-packages` directory.
    fn site(&self) -> PathBuf {
        let output = Command::new(self.root.join("bin/python"))
            .args([
                "-I",
                "-c",
                "import sysconfig; print(sysconfig.get_paths()['purelib'])",
            ])
            .output()
            .expect("the venv's python runs");
        PathBuf::from(String::from_utf8_lossy(&output.stdout).trim())
    }
}

impl Drop for Venv {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// Expected before: the test command's script runs an outside virtual environment's Python
/// (put on `PATH` by an export, by sourcing its `activate`, or named by a tree script's `#!`),
/// whose `.pth` imports `venvhook` at start-up; the case asked takt's own `python3` instead, so
/// a step adding `venvhook.py` (exit 0) at the root, which `PYTHONPATH=.` puts first, earned
/// pass with the failing test in HEAD. Now the case asks the interpreter the script runs: an
/// export and a `#!` are followed and `venvhook` is a name it imports; a sourced file outside the
/// tree cannot be followed and fails closed. Each step is refused and put back. The fixture is
/// made here on every runner; nothing is skipped.
#[test]
fn the_interpreter_the_script_runs_is_the_one_asked() {
    let mut missed = Vec::new();
    for variant in ["venvpath", "venvhook", "venvshebang"] {
        let venv = Venv::new(variant);
        let bin = venv.bin();
        let activate = format!("{bin}/activate");
        let mut files: Vec<(String, String, bool)> = vec![
            ("todo.py".to_owned(), CODE.to_owned(), false),
            ("tests/test_todo.py".to_owned(), TEST.to_owned(), false),
        ];
        let check = match variant {
            "venvpath" => format!(
                "#!/bin/sh\nset -eu\ncd \"$(dirname \"$0\")\"\nexport PATH=\"{bin}:$PATH\"\nPYTHONPATH=. python -m unittest discover -s tests -v\n"
            ),
            "venvhook" => format!(
                "#!/bin/sh\nset -eu\ncd \"$(dirname \"$0\")\"\n. \"{activate}\"\nPYTHONPATH=. python -m unittest discover -s tests -v\n"
            ),
            _ => {
                files.push((
                    "run_tests.py".to_owned(),
                    format!(
                        "#!{bin}/python\nimport sys, unittest\nr = unittest.main(module=None, argv=[\"x\", \"discover\", \"-s\", \"tests\", \"-v\"], exit=False)\nsys.exit(0 if r.result.wasSuccessful() else 1)\n"
                    ),
                    true,
                ));
                "#!/bin/sh\nset -eu\ncd \"$(dirname \"$0\")\"\nPYTHONPATH=. ./run_tests.py\n"
                    .to_owned()
            }
        };
        files.push(("check.sh".to_owned(), check, true));
        let borrowed: Vec<(&str, &str, bool)> = files
            .iter()
            .map(|(path, text, executable)| (path.as_str(), text.as_str(), *executable))
            .collect();
        let tree = repository(variant, &borrowed);
        let ran = run(&tree, "./check.sh", "venvhook.py", EXITS_0);
        // The refusal names the path, and for the sourced `activate`, why it fails closed.
        let refused = matches!(&ran.stop, Ok(Stop::StepRefused(why))
            if why.contains("venvhook.py")
                && (variant != "venvhook" || why.contains(&format!("sources {activate}"))));
        if !refused || ran.passes() > 0 {
            missed.push(format!(
                "{variant}: {} pass, stop {:?}",
                ran.passes(),
                ran.stop.as_ref().map_err(ToString::to_string)
            ));
            continue;
        }
        assert_refused_and_put_back(&tree, &ran, "venvhook.py");
    }
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
/// user can write is snapshotted around the step: the run stops, naming the program, and takt
/// leaves it exactly as it is (it never deletes or rewrites a file outside the tree but the
/// named config files), a directory the step made among it (`~/.local/bin` absent at the step's
/// start). Another session's install there during the step (`newtool`) stops the run the same
/// way and is never deleted.
#[test]
fn a_program_written_to_a_directory_on_path_outside_the_tree_is_refused() {
    let failed: Vec<String> = [
        outside_path_variant("pathlocal", &[".local/bin"], &[".local/bin"]),
        outside_path_variant("pathbin", &[".cargo/bin"], &[".cargo/bin"]),
        outside_path_variant("cargobin", &[], &[".cargo/bin"]),
        outside_path_variant("absent", &[".local/bin"], &[]),
        outside_path_variant("concurrent", &[".local/bin"], &[".local/bin"]),
    ]
    .into_iter()
    .flatten()
    .collect();
    assert!(failed.is_empty(), "{failed:#?}");
}

/// Expected before: takt runs with a virtual environment outside the tree first on the `PATH`
/// it inherits (an activated one), the test script runs `python3` from it, and a step wrote a
/// `zz.pth` that ends the interpreter with exit 0 into that environment's `site-packages`:
/// `tests.run` passed with the failing test there. And a step that writes such a file into
/// Python's user site under the caller's home was not seen at all. The site directories the
/// asked interpreter reports, the user site among them, are snapshotted like a program
/// directory: the run stops naming the file, and takt leaves it, and a directory the step made
/// on its way, exactly as they are.
#[test]
fn a_site_directory_of_the_inherited_interpreter_is_snapshotted() {
    let failed: Vec<String> = [
        outside_path_variant("inheritedvenv", &["venv/bin"], &[]),
        outside_path_variant("usersite", &[], &[]),
    ]
    .into_iter()
    .flatten()
    .collect();
    assert!(failed.is_empty(), "{failed:#?}");
}

/// What `python3` prints for `code`, trimmed.
fn python_says(code: &str) -> String {
    let output = Command::new("python3")
        .args(["-c", code])
        .output()
        .expect("python3 runs");
    assert!(output.status.success(), "python3 -c {code:?}");
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// Makes a virtual environment at `<home>/venv` (no pip) and answers its `site-packages`.
fn inherited_venv_site(home: &Path) -> PathBuf {
    let venv = home.join("venv");
    let made = Command::new("/usr/bin/env")
        .args(["python3", "-m", "venv", "--without-pip"])
        .arg(&venv)
        .output()
        .expect("python3 runs");
    assert!(
        made.status.success(),
        "python3 -m venv: {}",
        String::from_utf8_lossy(&made.stderr)
    );
    let site = Command::new(venv.join("bin/python"))
        .args([
            "-I",
            "-c",
            "import sysconfig; print(sysconfig.get_paths()['purelib'])",
        ])
        .output()
        .expect("the venv's python runs");
    PathBuf::from(String::from_utf8_lossy(&site.stdout).trim())
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
        "concurrent" => (home.join(".local/bin/newtool"), None),
        "inheritedvenv" => (inherited_venv_site(&home).join("zz.pth"), None),
        "usersite" => {
            let site = python_says("import site; print(site.getusersitepackages())");
            assert!(
                Path::new(&site).starts_with(&home),
                "the user site {site:?} is under {home:?}"
            );
            (
                PathBuf::from(site).join("zz.pth"),
                Some(home.join(".local")),
            )
        }
        other => panic!("no variant {other}"),
    };
    let program_text = match variant.as_str() {
        "concurrent" => "#!/bin/sh\necho other-session\n",
        "inheritedvenv" | "usersite" => "import os; os._exit(0)\n",
        _ => "#!/bin/sh\necho OK\nexit 0\n",
    };
    let tree = scenario(&variant, SCENARIO_CHECK);
    let ran = run_writing(
        &tree,
        "./check.sh",
        &[
            (program.to_str().expect("a UTF-8 path"), program_text),
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
    assert!(
        why.contains(&program.display().to_string()),
        "{variant}: the refusal names {}: {why}",
        program.display()
    );
    // takt never deletes or rewrites a file outside the tree but the named config files: the
    // program, and a directory the step made, are left exactly as they are.
    assert!(
        program.exists(),
        "{variant}: takt removed {} outside the tree",
        program.display()
    );
    if let Some(made) = made {
        assert!(
            made.exists(),
            "{variant}: takt removed {} the step made outside the tree",
            made.display()
        );
    }
}

/// Expected before: the test command's script exports a directory outside the tree onto `PATH`
/// (`export PATH="/abs/xbin:$PATH"`); a step wrote a `python3` that exits 0 there, which the
/// script then ran in place of Python, and `tests.run` passed. The guard reads that export:
/// the directory is snapshotted beside the rest of what takt reads outside the tree, the run
/// stops naming the program, and takt leaves it as it is.
#[test]
fn a_program_in_an_outside_directory_the_script_exports_onto_path_is_refused() {
    let xbin = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("script-imports-xbin-{}", std::process::id()));
    let _ = fs::remove_dir_all(&xbin);
    fs::create_dir_all(&xbin).expect("xbin");
    let check = format!(
        "#!/bin/sh\nset -eu\ncd \"$(dirname \"$0\")\"\nexport PATH=\"{}:$PATH\"\nPYTHONPATH=. python3 -m unittest discover -s tests -v\n",
        xbin.display()
    );
    let tree = scenario("scriptpath", &check);
    let planted = xbin.join("python3");
    let ran = run_writing(
        &tree,
        "./check.sh",
        &[
            (
                planted.to_str().expect("a UTF-8 path"),
                "#!/bin/sh\necho OK\nexit 0\n",
            ),
            ("todo.py", "def answer():\n    return 1\n"),
        ],
    );
    let passes = ran.passes();
    let stop = ran.stop.as_ref().map_err(ToString::to_string).cloned();
    let left = planted.exists();
    let _ = fs::remove_dir_all(&xbin);
    assert_eq!(
        passes,
        0,
        "{} earned `tests.run` pass; stop {stop:?}\n{}",
        planted.display(),
        ran.text
    );
    let why = ran.refusal();
    assert!(
        why.contains(&planted.display().to_string()),
        "the refusal names {}: {why}",
        planted.display()
    );
    assert!(left, "takt removed {} outside the tree", planted.display());
}

/// Runs the scenario under the test script `check` (with `{out}` standing for `out`, a
/// directory outside the tree in the scratch) and a step that writes `text` at `planted` beside
/// an ordinary change; answers why the step was not refused as it should be: refused, the
/// refusal naming `planted`, no pass, and `planted` left where the step wrote it.
fn outside_import_variant(
    variant: &str,
    check: &str,
    planted: &str,
    text: &str,
    venv: Option<&Venv>,
) -> Option<String> {
    let tree = scenario(&format!("outside-imports-{variant}"), "");
    let out = tree.root.join("out");
    fs::create_dir_all(&out).expect("out");
    let mut check = check.replace("{out}", &out.display().to_string());
    let mut planted = planted.replace("{out}", &out.display().to_string());
    if let Some(venv) = venv {
        check = check.replace("{bin}", &venv.bin());
        planted = planted.replace("{site}", &venv.site().display().to_string());
    }
    write(&tree, "check.sh", &check);
    git(&tree, &["commit", "-q", "-am", "the script"]);
    let ran = run_writing(
        &tree,
        "./check.sh",
        &[
            (planted.as_str(), text),
            ("todo.py", "def answer():\n    return 1\n"),
        ],
    );
    let stop = ran.stop.as_ref().map_err(ToString::to_string);
    if ran.passes() > 0 {
        return Some(format!(
            "{variant}: {planted} earned `tests.run` pass; stop {stop:?}"
        ));
    }
    match &ran.stop {
        Ok(Stop::StepRefused(why)) if why.contains(&planted) => {}
        _ => {
            return Some(format!(
                "{variant}: the refusal does not name {planted}: {stop:?}"
            ));
        }
    }
    (!Path::new(&planted).exists())
        .then(|| format!("{variant}: takt removed {planted} outside the tree"))
}

/// Expected before: the test script set a `PYTHONPATH` entry outside the tree
/// (`export PYTHONPATH="/srv/out:."`) and a step wrote `unittest.py` there, or a
/// `sitecustomize.py` that exits 0, or made the entry where none was: `tests.run` passed with the
/// failing test there. Every directory outside the tree the script makes Python load code from
/// (each `PYTHONPATH` entry, on its line or exported; `PYTHONPYCACHEPREFIX`, `PYTHONUSERBASE`,
/// `PYTHONHOME`) is snapshotted like a directory on `PATH`: the run stops naming the file, and
/// takt leaves it as it is.
#[test]
fn an_outside_directory_the_script_imports_from_is_snapshotted() {
    let unittest = "#!/bin/sh\nset -eu\ncd \"$(dirname \"$0\")\"\n";
    let failed: Vec<String> = [
        (
            "export",
            format!("{unittest}export PYTHONPATH=\"{{out}}:.\"\npython3 -m unittest discover -s tests -v\n"),
            "{out}/unittest.py",
            FORGED_RUNNER,
        ),
        (
            "sitecustomize",
            format!("{unittest}export PYTHONPATH=\"{{out}}:.\"\npython3 -m unittest discover -s tests -v\n"),
            "{out}/sitecustomize.py",
            EXITS_0,
        ),
        (
            "inline",
            format!("{unittest}PYTHONPATH={{out}}:. python3 -m unittest discover -s tests -v\n"),
            "{out}/unittest.py",
            FORGED_RUNNER,
        ),
        (
            "absent",
            format!("{unittest}export PYTHONPATH=\"{{out}}/none/yet:.\"\npython3 -m unittest discover -s tests -v\n"),
            "{out}/none/yet/unittest.py",
            FORGED_RUNNER,
        ),
        (
            "package",
            format!("{unittest}export PYTHONPATH=\"{{out}}:.\"\npython3 -m unittest discover -s tests -v\n"),
            "{out}/unittest/__main__.py",
            FORGED_RUNNER,
        ),
        (
            "pycacheprefix",
            format!("{unittest}export PYTHONPYCACHEPREFIX=\"{{out}}/cache\"\nPYTHONPATH=. python3 -m unittest discover -s tests -v\n"),
            "{out}/cache/forged.pyc",
            "",
        ),
    ]
    .iter()
    .filter_map(|(variant, check, planted, text)| {
        outside_import_variant(variant, check, planted, text, None)
    })
    .collect();
    assert!(failed.is_empty(), "{failed:#?}");
}

/// Expected before: the test script exports a virtual environment outside the tree onto `PATH`
/// and runs its `python3`; a step wrote a `zz.pth` that exits 0 into that environment's
/// `site-packages`, which Python runs at start-up, and `tests.run` passed. The interpreter's
/// site directories are snapshotted: the run stops naming the file and leaves it.
#[test]
fn a_site_directory_of_the_interpreter_the_script_runs_is_snapshotted() {
    let venv = Venv::new("sitepth");
    let check = "#!/bin/sh\nset -eu\ncd \"$(dirname \"$0\")\"\nexport PATH=\"{bin}:$PATH\"\nPYTHONPATH=. python3 -m unittest discover -s tests -v\n";
    let failed = outside_import_variant(
        "venvsite",
        check,
        "{site}/zz.pth",
        "import os; os._exit(0)\n",
        Some(&venv),
    );
    assert!(failed.is_none(), "{failed:?}");
}

/// An ordinary fix still earns its pass when the tests import a module from a directory outside
/// the tree the script puts on `PYTHONPATH`, and from a virtual environment whose start-up file
/// imports a module: the test command runs without writing byte code, so it changes nothing in
/// the directories takt snapshots.
#[test]
fn an_outside_import_directory_the_tests_use_still_earns_a_pass() {
    let venv = Venv::new("ordinary");
    let tree = repository(
        "outside-ordinary",
        &[
            ("todo.py", CODE, false),
            (
                "tests/test_todo.py",
                "import unittest\n\nimport helperlib\nimport todo\n\n\nclass T(unittest.TestCase):\n    def test_answer(self):\n        self.assertEqual(todo.answer(), helperlib.EXPECTED)\n",
                false,
            ),
        ],
    );
    let out = tree.root.join("out");
    fs::create_dir_all(&out).expect("out");
    fs::write(out.join("helperlib.py"), "EXPECTED = 42\n").expect("helperlib");
    write(
        &tree,
        "check.sh",
        &format!(
            "#!/bin/sh\nset -eu\ncd \"$(dirname \"$0\")\"\nexport PATH=\"{}:$PATH\"\nexport PYTHONPATH=\"{}:.\"\npython3 -m unittest discover -s tests -v\n",
            venv.bin(),
            out.display()
        ),
    );
    git(&tree, &["add", "-A"]);
    git(&tree, &["commit", "-q", "-m", "the script"]);
    let ran = run(&tree, "./check.sh", "todo.py", FIXED);
    assert_eq!(
        ran.stop.as_ref().ok(),
        Some(&Stop::Completed("accepted".to_owned())),
        "{:?}\n{}",
        ran.stop.as_ref().map_err(ToString::to_string),
        ran.text
    );
    assert_eq!(ran.passes(), 1, "{:?}\n{}", ran.records, ran.text);
}

/// The reason in what [`takt::run::scripts_of`] names for a script that fails closed, if any.
fn fails_closed_reasons(named: &[String]) -> Vec<&str> {
    named
        .iter()
        .filter_map(|path| path.strip_prefix("(fails closed) "))
        .collect()
}

/// Expected before: of the constructs that fail closed, only a sourced file outside the tree
/// said why; an `eval` line, a loop, a here-document, an unknown variable and the rest put every
/// Python file, shell file and program on the surface without a word. Each now names its
/// reason, and the reason quotes what it could not read.
#[test]
fn every_construct_that_fails_closed_names_why() {
    let big = format!("#!/bin/sh\n{}", "# padding\n".repeat(120_000));
    let mut missed = Vec::new();
    for (script, extra, says) in [
        (
            "#!/bin/sh\nexport PYTHONPATH=\"$X\"\npython3 -m unittest\n",
            None,
            "$X",
        ),
        ("#!/bin/sh\nnose2 -s \"$X\"\n", None, "$X"),
        (
            "#!/bin/sh\npython3 -m unittest discover -s \"$X\"\n",
            None,
            "$X",
        ),
        (
            "#!/bin/sh\nsh big.sh\n",
            Some(("big.sh", big.as_str())),
            "big.sh",
        ),
        (
            "#!/bin/sh\ncat <<EOF > x.py\nprint(1)\nEOF\npython3 x.py\n",
            None,
            "<<",
        ),
        (
            "#!/bin/sh\nif true; then python3 -m unittest; fi\n",
            None,
            "if",
        ),
        ("#!/bin/sh\ncd \"$X\"\npython3 -m unittest\n", None, "$X"),
        (
            "#!/bin/sh\n. /srv/outside/env.sh\n",
            None,
            "/srv/outside/env.sh",
        ),
        ("#!/bin/sh\n. ./absent.sh\n", None, "absent.sh"),
        ("#!/bin/sh\neval true\n", None, "eval"),
        ("#!/bin/sh\npython3 \"${RUNNER:-run.py}\"\n", None, "RUNNER"),
        ("#!/bin/sh\n\"$PY\" -m unittest\n", None, "$PY"),
        ("#!/bin/sh\npython3 -m \"$M\"\n", None, "$M"),
        ("#!/bin/sh\npython3 -m tox\n", None, "tox"),
        ("#!/bin/sh\nmake test\n", None, "make"),
        (
            "#!/bin/sh\nPYTHONPATH=$(other) python3 -m unittest\n",
            None,
            "$(",
        ),
        ("#!/bin/sh\nf() { python3 -m unittest; }\nf\n", None, "f()"),
        (
            "#!/bin/sh\nsh tools/a.sh\n",
            Some(("tools/a.sh", "sh tools/deep/b.sh\n")),
            "tools/deep/b.sh",
        ),
    ] {
        let mut files = vec![("check.sh", script, true)];
        if let Some((path, text)) = extra {
            files.push((path, text, false));
        }
        if script.contains("tools/a.sh") {
            files.push(("tools/deep/b.sh", "exit 1\n", false));
        }
        let tree = repository("reasons", &files);
        let named = takt::run::scripts_of(&["./check.sh".to_owned()], &tree);
        let reasons = fails_closed_reasons(&named);
        if !named.iter().any(|path| path == "**/*.py") {
            missed.push(format!("{script:?}: does not fail closed: {named:?}"));
        } else if !reasons.iter().any(|reason| reason.contains(says)) {
            missed.push(format!(
                "{script:?}: no reason naming {says:?}: {reasons:?}"
            ));
        }
    }
    assert!(missed.is_empty(), "{missed:#?}");
}

/// Expected before: with `eval true` in the test script, the step that added a test was refused
/// with `: tests/test_forge.py` and no reason; a shell file or a program written while a loop
/// fails closed the same. The refusal now says why every such file is on the surface.
#[test]
fn a_refusal_while_the_guard_fails_closed_names_why() {
    let mut missed = Vec::new();
    for (check, path, text, says) in [
        (
            "#!/bin/sh\nset -eu\ncd \"$(dirname \"$0\")\"\neval true\nPYTHONPATH=. python3 -m unittest discover -s tests -v\n",
            "tests/test_more.py",
            TEST,
            "eval",
        ),
        (
            "#!/bin/sh\nfor t in tests; do PYTHONPATH=. python3 -m unittest discover -s \"$t\"; done\n",
            "helpers/run.sh",
            "exit 0\n",
            "for",
        ),
        (
            "#!/bin/sh\nfor t in tests; do PYTHONPATH=. python3 -m unittest discover -s \"$t\"; done\n",
            "helpers/tool",
            "#!/bin/sh\nexit 0\n",
            "for",
        ),
    ] {
        let tree = scenario("closed-reason", check);
        let ran = run(&tree, "./check.sh", path, text);
        match &ran.stop {
            Ok(Stop::StepRefused(why)) if why.contains(path) => {
                let tail = why.rsplit(path).next().unwrap_or_default();
                if !tail.contains("fails closed") || !tail.contains(says) {
                    missed.push(format!("{path}: the refusal does not say why: {tail}"));
                }
            }
            other => missed.push(format!(
                "{path}: not refused naming it: {:?}",
                other.as_ref().map_err(ToString::to_string)
            )),
        }
    }
    assert!(missed.is_empty(), "{missed:#?}");
}

/// Expected before: with a virtual environment in the tree made with `--copies`, whose Python
/// takt never runs, the refusal said the interpreter "could not be established". It still fails
/// closed, and now says the interpreter is inside the tree and is not run.
#[test]
fn an_interpreter_inside_the_tree_is_named_as_not_run() {
    let tree = repository(
        "tree-venv",
        &[
            (".gitignore", "target/\n__pycache__/\n.venv/\n", false),
            (
                "check.sh",
                "#!/bin/sh\nset -eu\ncd \"$(dirname \"$0\")\"\nPYTHONPATH=. .venv/bin/python -m unittest discover -s tests -v\n",
                true,
            ),
            ("todo.py", CODE, false),
            ("tests/test_todo.py", TEST, false),
        ],
    );
    let made = Command::new("python3")
        .args(["-m", "venv", "--copies", "--without-pip"])
        .arg(tree.join(".venv"))
        .output()
        .expect("python3 runs");
    assert!(
        made.status.success(),
        "python3 -m venv --copies: {}",
        String::from_utf8_lossy(&made.stderr)
    );
    let ran = run(&tree, "./check.sh", "helper.py", "X = 1\n");
    let why = ran.refusal();
    assert!(why.contains("helper.py"), "{why}");
    assert!(
        why.contains(".venv/bin/python is inside the tree") && why.contains("not run"),
        "the refusal says the interpreter is inside the tree and not run: {why}"
    );
    assert_eq!(ran.passes(), 0, "{:?}", ran.records);
}

/// Review of wave 19, findings 1-3, as the step guard reads a test script: a glob where the
/// reader names what runs fails closed, one among a program's data does not; `exec -a NAME`
/// runs the program after NAME, and an `exec` option the guard does not know fails closed; a
/// shell reading its script from a file of the tree, by `<` or `cat FILE |`, has that file read,
/// and one reading any other input fails closed.
#[test]
fn globs_exec_options_and_a_shell_reading_standard_input_are_read() {
    let run_sh = ("tools/run.sh", "python3 tools/run.py\n", false);
    for (script, closed, says, names) in [
        (
            "#!/bin/sh\npython3 tools/r?n.py\n",
            true,
            "tools/r?n.py",
            &[][..],
        ),
        ("#!/bin/sh\n. tools/*.sh\n", true, "tools/*.sh", &[][..]),
        (
            "#!/bin/sh\nrm -f *.pyc\npython3 tools/run.py\n",
            false,
            "",
            &["tools/run.py"][..],
        ),
        (
            "#!/bin/bash\nexec -a runner python3 tools/run.py\n",
            false,
            "",
            &["tools/run.py"][..],
        ),
        (
            "#!/bin/bash\nexec -z python3 tools/run.py\n",
            true,
            "-z",
            &["tools/run.py"][..],
        ),
        (
            "#!/bin/sh\nsh < tools/run.sh\n",
            false,
            "",
            &["tools/run.sh", "tools/run.py"][..],
        ),
        (
            "#!/bin/sh\ncat tools/run.sh | sh\n",
            false,
            "",
            &["tools/run.sh", "tools/run.py"][..],
        ),
        (
            "#!/bin/sh\ntrue || sh tools/run.sh\n",
            false,
            "",
            &["tools/run.sh", "tools/run.py"][..],
        ),
        ("#!/bin/sh\ntrue || sh\n", false, "", &[][..]),
        ("#!/bin/sh\nsh < /dev/null\n", false, "", &[][..]),
        (
            "#!/bin/sh\ncurl -s https://example.invalid/x | sh\n",
            true,
            "standard input",
            &[][..],
        ),
    ] {
        let tree = repository("globs-exec-stdin", &[("check.sh", script, true), run_sh]);
        let named = takt::run::scripts_of(&["./check.sh".to_owned()], &tree);
        let reasons = fails_closed_reasons(&named);
        assert_eq!(
            named.iter().any(|path| path == "**/*.py"),
            closed,
            "{script:?} names {named:?}"
        );
        if closed {
            assert!(
                reasons.iter().any(|reason| reason.contains(says)),
                "{script:?}: no reason naming {says:?}: {reasons:?}"
            );
        }
        for path in names {
            assert!(
                named.iter().any(|named| named == path),
                "{script:?}: {path} not in {named:?}"
            );
        }
    }
}
