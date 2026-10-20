//! story:gate-compares-what-its-scripts-import and story:the-gate-compares-python-entry-points:
//! `gate.run` compares at the case's pinned base what Python loads from the tree for a gate's
//! commands, besides the files the project file names. In the directory a Python script runs
//! from (its own, or the directory Python runs as `python3 <dir>`): every module, compiled
//! module, `*.pth` file and package. For `python3 -m a.b`: the module and package `a` in the
//! directory the command runs in and in every `PYTHONPATH` entry the binding sets, as the step
//! guard names them. A file there that differs, is added or is deleted: not run, no record, the
//! path named. The changes are commits made on the case's branch outside the loop.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::anyhow;
use takt::harness::{Harness, StepContext, StepReport};
use takt::validator::{GateRun, Observation, Validator};
use takt::{Run, RunOptions};

const PROTOCOL: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../protocols/story-delivery/1.yaml"
);

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

fn write(dir: &Path, path: &str, text: &str) {
    let path = dir.join(path);
    fs::create_dir_all(path.parent().expect("a parent")).expect("a parent directory");
    fs::write(&path, text).expect("a file");
}

/// The Python every test runs its gate with.
const PYTHON: &str = "/usr/bin/python3";

/// A committed repository whose one gate runs `PYTHON` with `args` and the binding's `env`
/// (name, value), with `files` (path, text) besides; the gate passes at the base.
fn repository(name: &str, args: &[&str], env: &[(&str, &str)], files: &[(&str, &str)]) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("gate-imports-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(&dir).expect("scratch dir");
    let dir = Scratch(dir);
    write(&dir, "README.md", "a story\n");
    write(
        &dir,
        ".engineering/gates.yaml",
        "format: engineering-gates/1\nlanguage: canon-expr/1\ngates:\n  - id: check\n    \
         text: The check holds.\n    assertions:\n      - 'process.succeeded(\"check\")'\n",
    );
    let mut project = format!(
        "format: engineering-assertions-project/1\ncommands:\n  check:\n    executable: {PYTHON}\n    args:\n"
    );
    for arg in args {
        project.push_str(&format!("      - '{arg}'\n"));
    }
    if !env.is_empty() {
        project.push_str("    env:\n");
        for (name, value) in env {
            project.push_str(&format!("      {name}: '{value}'\n"));
        }
    }
    write(&dir, ".engineering/assertions-project.yaml", &project);
    for (path, text) in files {
        write(&dir, path, text);
    }
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "start"]);
    dir
}

/// The review's repository: `scripts/check.py` imports `checklib` from its own directory.
fn citations(name: &str) -> Scratch {
    repository(
        name,
        &["scripts/check.py"],
        &[],
        &[
            (
                "scripts/check.py",
                "import sys\nimport checklib\nsys.exit(checklib.run())\n",
            ),
            ("scripts/checklib.py", "def run():\n    return 0\n"),
            ("scripts/data.txt", "one\n"),
            ("src/app.py", "VALUE = 1\n"),
        ],
    )
}

fn options(id: &str) -> RunOptions {
    RunOptions {
        protocol: PROTOCOL.to_owned(),
        intent: "deliver the story".to_owned(),
        inputs: Vec::new(),
        iterations: Some(2),
        hours: None,
        tokens: None,
        case_id: Some(id.to_owned()),
        gate: Some("repository.merge".to_owned()),
    }
}

struct NoHarness;

impl Harness for NoHarness {
    fn name(&self) -> &str {
        "none"
    }

    fn step(&self, context: &StepContext<'_>) -> anyhow::Result<StepReport> {
        Err(anyhow!(
            "no harness: a judgement step among {} is left",
            context.offered.join(", ")
        ))
    }
}

/// Opens a case on `tree`, commits `changes` (path, text) on its branch and observes `gate.run`
/// at the commit.
fn observed(tree: &Path, id: &str, changes: &[(&str, &str)]) -> Observation {
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(GateRun::new(".engineering/gates.yaml"))];
    let _run = Run::open(tree, options(id), &NoHarness, &validators).expect("opens");
    for (path, text) in changes {
        write(tree, path, text);
        git(tree, &["add", "--", path]);
    }
    git(tree, &["commit", "-q", "-m", "a change outside the loop"]);
    let head = git(tree, &["rev-parse", "HEAD"]);
    GateRun::new(".engineering/gates.yaml")
        .observe(tree, &head, 1)
        .expect("observes")
}

/// Not run, no record, and the summary names `path`.
fn refused(observation: &Observation, path: &str) {
    assert!(
        observation.records.is_empty(),
        "the gate wrote a record after {path} changed: {}",
        observation.summary
    );
    assert!(
        observation.summary.contains("not run"),
        "{}",
        observation.summary
    );
    assert!(
        observation.summary.contains(path),
        "the summary does not name {path}: {}",
        observation.summary
    );
}

/// The gate ran and recorded `pass`.
fn recorded(observation: &Observation) {
    assert_eq!(
        observation.records.len(),
        1,
        "the gate did not record: {}",
        observation.summary
    );
    assert_eq!(
        observation.records[0].result.as_deref(),
        Some("pass"),
        "{}",
        observation.summary
    );
}

/// The review's probe: a commit to the module a gate script imports from its own directory.
#[test]
fn a_module_the_script_imports_from_its_own_directory_is_compared_at_the_base() {
    let tree = citations("sibling");
    let observation = observed(
        &tree,
        "sibling",
        &[(
            "scripts/checklib.py",
            "def run():\n    return 0  # edited\n",
        )],
    );
    refused(&observation, "scripts/checklib.py");
}

/// A module added beside the script is compared as one changed is.
#[test]
fn a_module_added_beside_the_script_keeps_the_gate_from_running() {
    let tree = citations("added");
    let observation = observed(
        &tree,
        "added",
        &[("scripts/json.py", "raise SystemExit(0)\n")],
    );
    refused(&observation, "scripts/json.py");
}

/// A package beside the script: everything under it.
#[test]
fn a_package_beside_the_script_keeps_the_gate_from_running() {
    let tree = citations("package");
    let observation = observed(
        &tree,
        "package",
        &[("scripts/checklib2/__init__.py", "RUN = 0\n")],
    );
    refused(&observation, "scripts/checklib2/__init__.py");
}

/// A `*.pth` file beside the script.
#[test]
fn a_path_file_beside_the_script_keeps_the_gate_from_running() {
    let tree = citations("pth");
    let observation = observed(&tree, "pth", &[("scripts/extra.pth", "import os\n")]);
    refused(&observation, "scripts/extra.pth");
}

/// A gate whose script imports nothing from the tree is decided as before: a Python file
/// outside the script's directory, or data beside it, lets it run and record.
#[test]
fn a_change_outside_the_scripts_imports_lets_the_gate_run_and_record() {
    let tree = citations("outside");
    let observation = observed(
        &tree,
        "outside",
        &[("src/app.py", "VALUE = 2\n"), ("scripts/data.txt", "two\n")],
    );
    recorded(&observation);
}

/// `python3 tool` runs `tool/__main__.py`, with `tool` first on the import path.
#[test]
fn the_main_module_of_a_directory_python_runs_is_compared_at_the_base() {
    let files = [
        ("tool/__main__.py", "import helper\nhelper.run()\n"),
        ("tool/helper.py", "def run():\n    pass\n"),
    ];
    for (name, changed) in [
        ("dir-main", "tool/__main__.py"),
        ("dir-helper", "tool/helper.py"),
    ] {
        let tree = repository(name, &["tool"], &[], &files);
        let observation = observed(&tree, name, &[(changed, "raise SystemExit(0)\n")]);
        refused(&observation, changed);
    }
}

/// `python3 -m pkg.check` runs `pkg/check.py`, or `pkg/check/__main__.py`, after
/// `pkg/__init__.py`, from the directory the command runs in.
#[test]
fn each_file_a_module_option_resolves_to_is_compared_at_the_base() {
    let files = [
        ("pkg/__init__.py", ""),
        ("pkg/check.py", "raise SystemExit(0)\n"),
    ];
    for (name, changed) in [
        ("module-file", "pkg/check.py"),
        ("module-init", "pkg/__init__.py"),
        ("module-main", "pkg/check/__main__.py"),
    ] {
        let tree = repository(name, &["-m", "pkg.check"], &[], &files);
        let observation = observed(&tree, name, &[(changed, "raise SystemExit(0)  # x\n")]);
        refused(&observation, changed);
    }
}

/// `-m` is found in each `PYTHONPATH` entry the gate's binding sets, as in the directory the
/// command runs in.
#[test]
fn a_module_in_a_pythonpath_entry_the_binding_sets_is_compared_at_the_base() {
    let tree = repository(
        "pythonpath",
        &["-m", "runner"],
        &[("PYTHONPATH", "tools/lib")],
        &[("tools/lib/runner.py", "raise SystemExit(0)\n")],
    );
    let observation = observed(
        &tree,
        "pythonpath",
        &[("tools/lib/runner.py", "raise SystemExit(0)  # edited\n")],
    );
    refused(&observation, "tools/lib/runner.py");
}

/// The code a `-m` gate checks, in the directory the command runs in, is not what it runs: a
/// change there lets it run and record.
#[test]
fn code_beside_a_module_the_gate_runs_lets_it_run_and_record() {
    let tree = repository(
        "module-beside",
        &["-m", "runner"],
        &[],
        &[
            ("runner.py", "raise SystemExit(0)\n"),
            ("app/__init__.py", ""),
            ("app/core.py", "VALUE = 1\n"),
        ],
    );
    let observation = observed(&tree, "module-beside", &[("app/core.py", "VALUE = 2\n")]);
    recorded(&observation);
}
