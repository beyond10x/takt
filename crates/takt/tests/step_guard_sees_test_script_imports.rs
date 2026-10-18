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

/// An `implementation.edit` step that writes the payload (`path` and its text, from the
/// scratch root) into the tree the first time it is offered, and changes nothing after.
struct Writes {
    payload: (String, String),
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
            let (path, text) = &self.payload;
            write(context.tree, path, text);
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
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(TestsRun::new(command).expect("a command"))];
    let harness = Writes {
        payload: (path.to_owned(), text.to_owned()),
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

fn write(tree: &Path, path: &str, text: &str) {
    let at = tree.join(path);
    fs::create_dir_all(at.parent().expect("a parent")).expect("a directory");
    fs::write(at, text).expect("a file");
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
        payload: (String::new(), String::new()),
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
