//! story:gate-compares-what-its-scripts-import and story:the-gate-compares-python-entry-points:
//! `gate.run` compares at the case's pinned base what Python loads from the tree for a gate's
//! commands, besides the files the project file names. In the directory a Python script runs
//! from (its own, or the directory Python runs as `python3 <dir>`): every module, compiled
//! module, `*.pth` file and package, nested or not; for a script at the tree's root, where the
//! command runs, the script and the `*.pth` files there. For `python3 -m a.b`: the module and
//! package `a` in the directory the command runs in and in every `PYTHONPATH` entry the binding
//! sets, as the step guard names them. A file there that differs, is added or is deleted: not
//! run, no record, the path named. The changes are commits made on the case's branch outside the
//! loop.
//!
//! story:gate-compares-python-run-through-a-shell: the same holds for a Python run through a
//! shell, by a `sh -c` string or a shell script the gate runs; a shell line the step guard's
//! reader cannot read with certainty fails closed, naming why.

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
    repository_running(name, PYTHON, args, env, files)
}

/// [`repository`] with the gate running `executable` rather than `PYTHON`.
fn repository_running(
    name: &str,
    executable: &str,
    args: &[&str],
    env: &[(&str, &str)],
    files: &[(&str, &str)],
) -> Scratch {
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
        "format: engineering-assertions-project/1\ncommands:\n  check:\n    executable: {executable}\n    args:\n"
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

/// A namespace package nested below the script's directory, with no module directly in its
/// first directory (`scripts/lib/core/util.py`), is compared whole as a package is.
#[test]
fn a_namespace_package_two_levels_below_the_script_is_compared_at_the_base() {
    let tree = repository(
        "nsdeep",
        &["scripts/check.py"],
        &[],
        &[
            (
                "scripts/check.py",
                "import sys\nfrom lib.core import util\nsys.exit(util.run())\n",
            ),
            ("scripts/lib/core/util.py", "def run():\n    return 0\n"),
        ],
    );
    let observation = observed(
        &tree,
        "nsdeep",
        &[(
            "scripts/lib/core/util.py",
            "def run():\n    return 0  # edited\n",
        )],
    );
    refused(&observation, "scripts/lib");
}

/// A script at the tree's root runs from the directory the command runs in, which holds the code
/// the gate checks: as for `-m`, its own file and the `*.pth` files there are compared, not every
/// module and package beside it, and a change to that code lets the gate run and record.
#[test]
fn code_beside_a_script_at_the_root_lets_the_gate_run_and_record() {
    let files = [
        ("check_cit.py", "raise SystemExit(0)\n"),
        ("todo.py", "VALUE = 1\n"),
        ("app/__init__.py", ""),
        ("app/core.py", "VALUE = 1\n"),
    ];
    let tree = repository("root-beside", &["check_cit.py"], &[], &files);
    let observation = observed(
        &tree,
        "root-beside",
        &[("todo.py", "VALUE = 2\n"), ("app/core.py", "VALUE = 2\n")],
    );
    recorded(&observation);
    for (name, changed) in [("root-self", "check_cit.py"), ("root-pth", "extra.pth")] {
        let tree = repository(name, &["check_cit.py"], &[], &files);
        let observation = observed(&tree, name, &[(changed, "import os\n")]);
        refused(&observation, changed);
    }
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

/// The shell every test of story:gate-compares-python-run-through-a-shell runs its gate with.
const SHELL: &str = "/bin/sh";

/// The review's repository ([`citations`]) with the gate running `args` under [`SHELL`] and
/// `scripts/check.sh` running the Python script besides.
fn through_a_shell(name: &str, args: &[&str]) -> Scratch {
    repository_running(
        name,
        SHELL,
        args,
        &[],
        &[
            (
                "scripts/check.py",
                "import sys\nimport checklib\nsys.exit(checklib.run())\n",
            ),
            ("scripts/checklib.py", "def run():\n    return 0\n"),
            (
                "scripts/check.sh",
                "#!/bin/sh\nexec python3 scripts/check.py\n",
            ),
            ("scripts/data.txt", "one\n"),
            ("src/app.py", "VALUE = 1\n"),
        ],
    )
}

/// story:gate-compares-python-run-through-a-shell, acceptance 1 (the review's probes `shc` and
/// `shcself`): `sh -c "python3 scripts/check.py"` runs the script with its own directory first
/// on the import path, as `python3 scripts/check.py` does; the script and the module beside it
/// are compared at the base. Code outside that directory, and data beside it, are not.
#[test]
fn python_run_through_sh_c_is_compared_at_the_base() {
    let args = ["-c", "python3 scripts/check.py"];
    for (name, changed) in [
        ("shc-lib", "scripts/checklib.py"),
        ("shc-self", "scripts/check.py"),
        ("shc-added", "scripts/json.py"),
    ] {
        let tree = through_a_shell(name, &args);
        let observation = observed(&tree, name, &[(changed, "raise SystemExit(0)  # x\n")]);
        refused(&observation, changed);
    }
    let tree = through_a_shell("shc-outside", &args);
    let observation = observed(
        &tree,
        "shc-outside",
        &[("src/app.py", "VALUE = 2\n"), ("scripts/data.txt", "two\n")],
    );
    recorded(&observation);
}

/// Acceptance 2 (the probes `shscript` and `shscriptself`): a shell script the gate runs whose
/// line runs Python, run by the shell as the gate's argument or from a `sh -c` string; the
/// shell script itself is compared too when the string names it.
#[test]
fn python_a_shell_script_runs_is_compared_at_the_base() {
    for (name, args, changed) in [
        ("shs-lib", &["scripts/check.sh"][..], "scripts/checklib.py"),
        ("shs-self", &["scripts/check.sh"][..], "scripts/check.py"),
        (
            "shs-c-lib",
            &["-c", "sh scripts/check.sh"][..],
            "scripts/checklib.py",
        ),
        (
            "shs-c-script",
            &["-c", "sh scripts/check.sh"][..],
            "scripts/check.sh",
        ),
    ] {
        let tree = through_a_shell(name, args);
        let observation = observed(&tree, name, &[(changed, "raise SystemExit(0)  # x\n")]);
        refused(&observation, changed);
    }
    let tree = through_a_shell("shs-outside", &["scripts/check.sh"]);
    let observation = observed(&tree, "shs-outside", &[("src/app.py", "VALUE = 2\n")]);
    recorded(&observation);
}

/// Acceptance 3: a shell line the step guard cannot read with certainty (`eval`, a loop) fails
/// closed in the gate as in the guard: every Python file, shell file and program of the tree is
/// compared, and the summary names the path and why. A change to anything else lets it run.
#[test]
fn a_shell_line_the_gate_cannot_read_fails_closed_naming_it() {
    for (name, args, changed, why) in [
        (
            "closed-eval",
            &["-c", "eval python3 scripts/check.py"][..],
            "src/app.py",
            "eval",
        ),
        (
            "closed-loop",
            &["-c", "for f in x; do python3 scripts/check.py; done"][..],
            "src/app.py",
            "compound command",
        ),
    ] {
        let tree = through_a_shell(name, args);
        let observation = observed(&tree, name, &[(changed, "VALUE = 2\n")]);
        refused(&observation, changed);
        assert!(
            observation.summary.contains(why),
            "the summary does not say why the gate fails closed ({why}): {}",
            observation.summary
        );
    }
    let tree = through_a_shell("closed-data", &["-c", "eval python3 scripts/check.py"]);
    let observation = observed(&tree, "closed-data", &[("README.md", "another story\n")]);
    recorded(&observation);
}

/// The bash every test of a bash-only form runs its gate with.
const BASH: &str = "/bin/bash";

/// [`through_a_shell`] with the gate running `args` under `shell`.
fn through(shell: &str, name: &str, args: &[&str]) -> Scratch {
    repository_running(
        name,
        shell,
        args,
        &[],
        &[
            (
                "scripts/check.py",
                "import sys\nimport checklib\nsys.exit(checklib.run())\n",
            ),
            ("scripts/checklib.py", "def run():\n    return 0\n"),
            (
                "scripts/check.sh",
                "#!/bin/sh\nexec python3 scripts/check.py\n",
            ),
            ("scripts/data.txt", "one\n"),
            ("src/app.py", "VALUE = 1\n"),
        ],
    )
}

/// Not run, and the summary says why the gate's commands fail closed, quoting `why`.
fn closed_naming(observation: &Observation, path: &str, why: &str) {
    refused(observation, path);
    assert!(
        observation.summary.contains(why),
        "the summary does not say why the gate fails closed ({why}): {}",
        observation.summary
    );
}

/// Review of wave 19, finding 1 (the probes `glob` and `brace`): a word holding a glob or a
/// brace expansion where the reader names a script (`python3 scripts/chec?.py`, bash's
/// `python3 scripts/{check,check}.py`) was read as the literal path, which names nothing, so a
/// change to the module the script imports decided the gate. The reader does not expand them:
/// the line fails closed, naming the word.
#[test]
fn a_glob_or_brace_word_the_reader_does_not_expand_fails_closed() {
    for (name, shell, line, why) in [
        (
            "glob-q",
            SHELL,
            "python3 scripts/chec?.py",
            "scripts/chec?.py",
        ),
        (
            "glob-star",
            SHELL,
            "python3 scripts/check*.py",
            "scripts/check*.py",
        ),
        (
            "glob-class",
            SHELL,
            "python3 scripts/chec[k].py",
            "scripts/chec[k].py",
        ),
        (
            "brace",
            BASH,
            "python3 scripts/{check,check}.py",
            "scripts/{check,check}.py",
        ),
        (
            "glob-program",
            SHELL,
            "scripts/chec?.sh",
            "scripts/chec?.sh",
        ),
    ] {
        let tree = through(shell, name, &["-c", line]);
        let observation = observed(
            &tree,
            name,
            &[("scripts/checklib.py", "def run():\n    return 0  # x\n")],
        );
        closed_naming(&observation, "scripts/checklib.py", why);
    }
    // A glob among a program's data, not where the reader names what runs, is read as before.
    let tree = through(
        SHELL,
        "glob-data",
        &["-c", "python3 scripts/check.py scripts/*.txt"],
    );
    let observation = observed(&tree, "glob-data", &[("src/app.py", "VALUE = 2\n")]);
    recorded(&observation);
}

/// Finding 2 (the probe `execa`): `exec -a NAME PROGRAM` was read with NAME as the program, so the
/// Python it runs was not seen. `exec`'s options are read (`-a NAME`, `-c`, `-l`) and the next
/// word is the program; an option the reader does not know fails closed.
#[test]
fn exec_and_its_options_run_the_program_after_them() {
    for (name, line) in [
        ("exec-a", "exec -a chk python3 scripts/check.py"),
        ("exec-c", "exec -c python3 scripts/check.py"),
        ("exec-cla", "exec -cla chk python3 scripts/check.py"),
        ("exec-dashdash", "exec -l -- python3 scripts/check.py"),
    ] {
        let tree = through(BASH, name, &["-c", line]);
        let observation = observed(
            &tree,
            name,
            &[("scripts/checklib.py", "def run():\n    return 0  # x\n")],
        );
        refused(&observation, "scripts/checklib.py");
    }
    let tree = through(
        BASH,
        "exec-a-outside",
        &["-c", "exec -a chk python3 scripts/check.py"],
    );
    let observation = observed(&tree, "exec-a-outside", &[("src/app.py", "VALUE = 2\n")]);
    recorded(&observation);
    let tree = through(
        BASH,
        "exec-unknown",
        &["-c", "exec -z python3 scripts/check.py"],
    );
    let observation = observed(&tree, "exec-unknown", &[("src/app.py", "VALUE = 2\n")]);
    closed_naming(&observation, "src/app.py", "-z");
}

/// Finding 3 (the probes `shstdin` and `shpipe`): a shell reading its script from its standard
/// input, by a redirection (`sh < scripts/check.sh`) or a pipe (`cat scripts/check.sh | sh`), ran
/// a script the reader neither read nor failed closed on. A plain file of the tree is read as the
/// script the shell runs; any other input (`echo … | sh`) fails closed.
#[test]
fn a_shell_reading_its_script_from_standard_input_reads_that_file() {
    for (name, line, changed) in [
        (
            "stdin-redirect",
            "sh < scripts/check.sh",
            "scripts/checklib.py",
        ),
        (
            "stdin-redirect-s",
            "sh -s < scripts/check.sh",
            "scripts/checklib.py",
        ),
        (
            "stdin-pipe",
            "cat scripts/check.sh | sh",
            "scripts/checklib.py",
        ),
        (
            "stdin-pipe-self",
            "cat scripts/check.sh | sh",
            "scripts/check.py",
        ),
    ] {
        let tree = through(SHELL, name, &["-c", line]);
        let observation = observed(&tree, name, &[(changed, "raise SystemExit(0)  # x\n")]);
        refused(&observation, changed);
    }
    let tree = through(SHELL, "stdin-outside", &["-c", "cat scripts/check.sh | sh"]);
    let observation = observed(&tree, "stdin-outside", &[("src/app.py", "VALUE = 2\n")]);
    recorded(&observation);
    let tree = through(
        SHELL,
        "stdin-echo",
        &["-c", "echo python3 scripts/check.py | sh"],
    );
    let observation = observed(&tree, "stdin-echo", &[("src/app.py", "VALUE = 2\n")]);
    closed_naming(&observation, "src/app.py", "standard input");
}

/// Fix round 2 of wave 19, finding 1: a descriptor before a redirection (`2>`, `0<`) was left as
/// a word and `2>&1` was split at `&`, so the reader took `2` (or `0`) as the script of the shell
/// or the Python before it: `sh < scripts/check.sh 2>&1` read no script and the stdin rule never
/// fired. A hand commit to the module the script imports must leave the gate not run, and a
/// change outside it must let the gate run and record: the line is read, not failed closed.
fn read_through_a_redirection(name: &str, shell: &str, line: &str) {
    let tree = through(shell, name, &["-c", line]);
    let observation = observed(&tree, name, &[("scripts/checklib.py", HAND)]);
    refused(&observation, "scripts/checklib.py");
    let outside = format!("{name}-outside");
    let tree = through(shell, &outside, &["-c", line]);
    let observation = observed(&tree, &outside, &[("src/app.py", "VALUE = 2\n")]);
    recorded(&observation);
}

/// The probe `shin21`: `sh < FILE 2>&1`.
#[test]
fn a_shell_reading_standard_input_with_stderr_on_stdout_reads_that_file() {
    read_through_a_redirection("fd-shin21", SHELL, "sh < scripts/check.sh 2>&1");
}

/// The probe `shin2null`: `sh < FILE 2>/dev/null`.
#[test]
fn a_shell_reading_standard_input_with_stderr_to_null_reads_that_file() {
    read_through_a_redirection("fd-shin2null", SHELL, "sh < scripts/check.sh 2>/dev/null");
}

/// The probe `shpipe2`: `cat FILE | sh 2>&1`.
#[test]
fn a_shell_piped_its_script_with_stderr_on_stdout_reads_that_file() {
    read_through_a_redirection("fd-shpipe2", SHELL, "cat scripts/check.sh | sh 2>&1");
}

/// `sh 0< FILE`: descriptor 0 is standard input.
#[test]
fn a_shell_reading_descriptor_zero_from_a_file_reads_that_file() {
    read_through_a_redirection("fd-shin0", SHELL, "sh 0< scripts/check.sh");
}

/// The probe `py2null`, which slipped at main too: `python3 2>/dev/null SCRIPT`.
#[test]
fn python_with_a_redirection_before_its_script_runs_that_script() {
    read_through_a_redirection("fd-py2null", SHELL, "python3 2>/dev/null scripts/check.py");
}

/// The redirections that only move output (`>&2`, `2>&-`, `&>`, `&>>`, `>|`, `>>`) take their
/// target and leave the program and its script where they were.
#[test]
fn output_redirections_leave_the_script_read() {
    for (name, shell, line) in [
        ("fd-dup-out", SHELL, "python3 scripts/check.py >&2 2>&-"),
        ("fd-amp-out", BASH, "python3 &>/dev/null scripts/check.py"),
        (
            "fd-amp-append",
            BASH,
            "python3 &>>/dev/null scripts/check.py",
        ),
        ("fd-clobber", SHELL, "python3 >|/dev/null scripts/check.py"),
        ("fd-append", SHELL, "python3 1>>/dev/null scripts/check.py"),
    ] {
        read_through_a_redirection(name, shell, line);
    }
}

/// What a shell reads on a duplicated descriptor (`sh <&3`) is not a file the reader can name, and
/// a redirection form it does not know (bash's `{fd}>`, `>>>`) fails closed, naming why.
#[test]
fn a_redirection_the_reader_cannot_follow_fails_closed() {
    for (name, shell, line, why) in [
        (
            "fd-dup-in",
            BASH,
            "exec 3< scripts/check.sh; sh <&3",
            "standard input",
        ),
        (
            "fd-named",
            BASH,
            "python3 scripts/check.py {fd}>/dev/null",
            "{fd}>",
        ),
        (
            "fd-unknown",
            SHELL,
            "python3 scripts/check.py >>> /dev/null",
            ">>>",
        ),
        (
            "fd-procsub",
            BASH,
            "bash < <(cat scripts/check.sh)",
            "subshell",
        ),
    ] {
        let tree = through(shell, name, &["-c", line]);
        let observation = observed(&tree, name, &[("src/app.py", "VALUE = 2\n")]);
        closed_naming(&observation, "src/app.py", why);
    }
}

/// The script of the review's probes: it imports `checklib` and exits with what it returns.
const MAIN: &str = "import sys\nimport checklib\nsys.exit(checklib.run())\n";

/// What the review's probes commit to the module the gate's Python imports.
const HAND: &str = "def run():\n    return 0  # a hand commit\n";

/// A review probe of a shell form the gate does not yet read: `line` run by `/bin/sh -c` in a
/// tree holding `files` (path, text) and a `checklib` module at `lib`; a hand commit to `lib`
/// must leave the gate not run, naming it.
fn slipping_form(name: &str, line: &str, files: &[(&str, &str)], lib: &str) {
    let mut all = files.to_vec();
    all.push((lib, "def run():\n    return 0\n"));
    let tree = repository_running(name, SHELL, &["-c", line], &[], &all);
    let observation = observed(&tree, name, &[(lib, HAND)]);
    refused(&observation, lib);
}

/// The review's probe `pypath`: a module the script imports from a `PYTHONPATH` entry the line
/// sets before the program.
#[test]
fn pypath() {
    slipping_form(
        "form-pypath",
        "PYTHONPATH=lib python3 scripts/check.py",
        &[("scripts/check.py", MAIN)],
        "lib/checklib.py",
    );
}

/// The review's probe `exportpp`: the same `PYTHONPATH` entry set by `export`.
#[test]
fn exportpp() {
    slipping_form(
        "form-exportpp",
        "export PYTHONPATH=lib; python3 scripts/check.py",
        &[("scripts/check.py", MAIN)],
        "lib/checklib.py",
    );
}

/// The review's probe `dashm`: `cd scripts && python3 -m check`, a module beside the one `-m`
/// runs.
#[test]
fn dashm() {
    slipping_form(
        "form-dashm",
        "cd scripts && python3 -m check",
        &[("scripts/check.py", MAIN)],
        "scripts/checklib.py",
    );
}

/// The review's probe `xargs`: the script named on `xargs`'s standard input.
#[test]
fn xargs() {
    slipping_form(
        "form-xargs",
        "echo scripts/check.py | xargs python3",
        &[("scripts/check.py", MAIN)],
        "scripts/checklib.py",
    );
}

/// The review's probe `source`: a sourced file sets `PYTHONPATH` by assignment and `export`.
#[test]
fn source() {
    slipping_form(
        "form-source",
        ". scripts/env.sh && python3 scripts/check.py",
        &[
            ("scripts/check.py", MAIN),
            ("scripts/env.sh", "PYTHONPATH=lib\nexport PYTHONPATH\n"),
        ],
        "lib/checklib.py",
    );
}

/// The final confirm's probe `execin0`: `exec 0< FILE` before a shell that reads its script from
/// standard input.
#[test]
fn exec_stdin_descriptor_then_sh() {
    slipping_form(
        "form-execin0",
        "exec 0< scripts/check.sh; sh",
        &[
            ("scripts/check.sh", "python3 scripts/check.py\n"),
            ("scripts/check.py", MAIN),
        ],
        "scripts/checklib.py",
    );
}

/// The final confirm's probe `execin`: `exec < FILE` before `sh`.
#[test]
fn exec_stdin_then_sh() {
    slipping_form(
        "form-execin",
        "exec < scripts/check.sh; sh",
        &[
            ("scripts/check.sh", "python3 scripts/check.py\n"),
            ("scripts/check.py", MAIN),
        ],
        "scripts/checklib.py",
    );
}

/// The final confirm's probe `execinbash`: `exec < FILE` before `bash`.
#[test]
fn exec_stdin_then_bash() {
    slipping_form(
        "form-execinbash",
        "exec < scripts/check.sh; bash",
        &[
            ("scripts/check.sh", "python3 scripts/check.py\n"),
            ("scripts/check.py", MAIN),
        ],
        "scripts/checklib.py",
    );
}

/// The probe `envCm`: `env -C dir python3 -m x`, a module beside the one `-m` runs.
#[test]
fn env_chdir_dash_m() {
    slipping_form(
        "form-envcm",
        "env -C scripts python3 -m check",
        &[("scripts/check.py", MAIN)],
        "scripts/checklib.py",
    );
}

/// The confirm's probe `nestedcd`: `cd dir && python3 -c …` importing from that directory.
#[test]
fn cd_then_dash_c() {
    slipping_form(
        "form-nestedcd",
        "cd scripts && python3 -c \"import sys, checklib; sys.exit(checklib.run())\"",
        &[],
        "scripts/checklib.py",
    );
}

/// The confirm's probe `stdincd`: `cd dir && python3 - < x.py` importing from that directory.
#[test]
fn cd_then_stdin_script() {
    slipping_form(
        "form-stdincd",
        "cd scripts && python3 - < check.py",
        &[("scripts/check.py", MAIN)],
        "scripts/checklib.py",
    );
}

/// A review probe of a program that runs another program: `line` run by `/bin/sh -c` as
/// [`slipping_form`] runs it, and the summary names `why` when `why` is given: the line fails
/// closed, since the reader does not know how the program runs what it is given.
fn wrapped_form(name: &str, line: &str, why: Option<&str>) {
    let files = [("scripts/check.py", MAIN)];
    let lib = "scripts/checklib.py";
    let mut all = files.to_vec();
    all.push((lib, "def run():\n    return 0\n"));
    let tree = repository_running(name, SHELL, &["-c", line], &[], &all);
    let observation = observed(&tree, name, &[(lib, HAND)]);
    refused(&observation, lib);
    match why {
        Some(why) => assert!(
            observation.summary.contains(why),
            "the summary does not say why the gate fails closed ({why}): {}",
            observation.summary
        ),
        None => assert!(
            !observation.summary.contains("fail closed"),
            "the gate fails closed on a wrapper it reads: {}",
            observation.summary
        ),
    }
}

/// Acceptance 4: `taskset MASK CMD` is read.
#[test]
fn taskset_runs_the_program_after_its_mask() {
    wrapped_form("wrap-taskset", "taskset 1 python3 scripts/check.py", None);
}

/// Acceptance 4: `chrt [policy] PRIORITY CMD` is read.
#[test]
fn chrt_runs_the_program_after_its_priority() {
    wrapped_form("wrap-chrt", "chrt -f 10 python3 scripts/check.py", None);
}

/// Acceptance 4: `unbuffer CMD` is read.
#[test]
fn unbuffer_runs_the_program_after_it() {
    wrapped_form("wrap-unbuffer", "unbuffer python3 scripts/check.py", None);
}

/// Acceptance 4: `script -qec CMD FILE` runs its command string through a shell, read as `sh -c`
/// is.
#[test]
fn script_runs_its_command_string() {
    wrapped_form(
        "wrap-script",
        "script -qec \"python3 scripts/check.py\" /dev/null",
        None,
    );
}

/// Acceptance 4: `uv run CMD` runs the program after its options; `uv run FILE.py` runs the
/// file under the project's Python.
#[test]
fn uv_run_runs_the_program_or_script_after_its_options() {
    wrapped_form("wrap-uv", "uv run --no-sync python3 scripts/check.py", None);
    wrapped_form("wrap-uv-script", "uv run scripts/check.py", None);
}

/// Acceptance 4: `env -S STRING` runs the string's words.
#[test]
fn env_split_string_runs_its_words() {
    wrapped_form("wrap-env-s", "env -S \"python3 scripts/check.py\"", None);
}

/// Acceptance 4: `pytest FILE` runs the file as a test module, its directory first on the
/// import path.
#[test]
fn pytest_runs_the_files_it_is_given() {
    wrapped_form("wrap-pytest", "pytest -q scripts/check.py", None);
}

/// Acceptance 4: `xargs CMD` runs the command with the words of its input appended, which the
/// reader does not read: the command is read and the line fails closed, naming `xargs`.
#[test]
fn xargs_runs_its_command_and_fails_closed_on_its_input() {
    wrapped_form(
        "wrap-xargs",
        "echo scripts/check.py | xargs python3",
        Some("`xargs` runs"),
    );
}

/// Acceptance 4: a program the reader does not know that is given a program it knows fails
/// closed, naming both.
#[test]
fn an_unlisted_program_given_a_program_fails_closed() {
    wrapped_form(
        "wrap-unlisted",
        "doas python3 scripts/check.py",
        Some("`doas` is given `python3`"),
    );
}

/// A gate running `scripts/check.py`, which imports `m1` from the `PYTHONPATH` entry `lib`,
/// where only what is imported is compared; `lib/m<k>.py` imports `m<k+1>` up to
/// `lib/m<length>.py`, which imports nothing. The gate passes at the base.
fn chain(name: &str, length: usize) -> Scratch {
    let mut files: Vec<(String, String)> = vec![(
        "scripts/check.py".to_owned(),
        "import sys\nimport m1\nsys.exit(m1.run())\n".to_owned(),
    )];
    for k in 1..=length {
        let text = if k < length {
            format!(
                "import m{}\n\n\ndef run():\n    return m{}.run()\n",
                k + 1,
                k + 1
            )
        } else {
            "def run():\n    return 0\n".to_owned()
        };
        files.push((format!("lib/m{k}.py"), text));
    }
    let files: Vec<(&str, &str)> = files
        .iter()
        .map(|(path, text)| (path.as_str(), text.as_str()))
        .collect();
    repository(
        name,
        &["scripts/check.py"],
        &[("PYTHONPATH", "lib")],
        &files,
    )
}

/// The gate reads the imports of at most 64 Python files for a command (the script and the 63
/// modules after it). Review of wave 22, unit A, condition 1: a chain that runs past the cap
/// fails closed, naming the file at which the cap was reached, so every Python file of the
/// tree is compared and a module past the cap, which the walk never reads, decides nothing.
#[test]
fn an_import_chain_past_the_cap_fails_closed_naming_where_it_was_reached() {
    let tree = chain("cap", 70);
    let observation = observed(
        &tree,
        "cap",
        &[("lib/m70.py", "def run():\n    return 1\n")],
    );
    closed_naming(&observation, "lib/m70.py", "lib/m64.py");
    assert!(
        observation.summary.contains("64 Python files"),
        "the summary does not name the cap: {}",
        observation.summary
    );
}

/// Review of wave 22, unit A, condition 2: a Python file the walk reads that imports by a form
/// no import line names (`importlib` in any use, `__import__(`, a call of `exec(` or `eval(`)
/// fails closed, naming the file and the form, so the module it loads is compared with every
/// other Python file of the tree.
#[test]
fn a_dynamic_import_in_a_read_module_fails_closed_naming_the_file_and_form() {
    for (name, lib, form) in [
        (
            "dyn-import-module",
            "import importlib\n\n\ndef run():\n    return importlib.import_module(\"plugin\").run()\n",
            "importlib.import_module(",
        ),
        (
            "dyn-from",
            "from importlib import import_module\n\n\ndef run():\n    return import_module(\"plugin\").run()\n",
            "from importlib",
        ),
        (
            "dyn-dunder",
            "def run():\n    return __import__(\"plugin\").run()\n",
            "__import__(",
        ),
        (
            "dyn-exec",
            "def run():\n    exec(\"import plugin\")\n    return 0\n",
            "exec(",
        ),
        (
            "dyn-eval",
            "def run():\n    return eval(\"__import__('plugin').run()\")\n",
            "eval(",
        ),
    ] {
        let tree = repository(
            name,
            &["scripts/check.py"],
            &[("PYTHONPATH", "lib")],
            &[
                (
                    "scripts/check.py",
                    "import sys\nimport checklib\nsys.exit(checklib.run())\n",
                ),
                ("lib/checklib.py", lib),
                ("lib/plugin.py", "def run():\n    return 0\n"),
            ],
        );
        let observation = observed(
            &tree,
            name,
            &[("lib/plugin.py", "def run():\n    return 1\n")],
        );
        closed_naming(&observation, "lib/plugin.py", "lib/checklib.py");
        assert!(
            observation.summary.contains(form),
            "the summary does not name the form {form}: {}",
            observation.summary
        );
    }
}

/// The control: a chain under the cap with no dynamic import is read whole and nothing fails
/// closed. A Python file outside what it imports lets the gate run and record; its last
/// module is compared, and a change there keeps the gate from running, named.
#[test]
fn an_import_chain_under_the_cap_is_read_whole_and_the_gate_runs() {
    let tree = chain("under-cap", 10);
    let observation = observed(&tree, "under-cap", &[("src/app.py", "VALUE = 2\n")]);
    recorded(&observation);
    let tree = chain("under-cap-last", 10);
    let observation = observed(
        &tree,
        "under-cap-last",
        &[("lib/m10.py", "def run():\n    return 1\n")],
    );
    refused(&observation, "lib/m10.py");
    assert!(
        !observation.summary.contains("fail closed"),
        "a chain under the cap fails closed: {}",
        observation.summary
    );
}
