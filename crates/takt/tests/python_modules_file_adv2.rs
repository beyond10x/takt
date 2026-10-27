//! Adversarial cases for story:python-modules-are-kept-only-when-a-python-answers: whether a
//! case keeps `python-modules.json` now hangs on `Modules::ask` returning exactly the built-in
//! lists (`Modules::default`). Every test command that runs a Python, found or not, must give
//! something else, so the file is kept; one that runs none gives exactly the built-in lists. The
//! words are split as `TestsRun::new` splits them, on white space, and run without a shell.

use std::fs;
use std::path::{Path, PathBuf};

use takt::run::Modules;

/// An absolute Python outside the tree that is not there: the interpreter cannot be established.
const MISSING_PYTHON: &str = "/nonexistent-takt-adv-python-modules/bin/python3";

struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn asked(name: &str, files: &[(&str, &str)], command: &str) -> Modules {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("python-modules-adv2-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("a directory");
    let scratch = Scratch(root);
    for (path, text) in files {
        let at = scratch.0.join(path);
        fs::write(&at, text).expect("a file");
        #[cfg(unix)]
        if text.starts_with("#!") {
            use std::os::unix::fs::PermissionsExt as _;
            fs::set_permissions(&at, fs::Permissions::from_mode(0o755)).expect("a mode");
        }
    }
    let words: Vec<String> = command.split_whitespace().map(str::to_owned).collect();
    Modules::ask(&words, &scratch.0)
}

fn python3_on_path() -> bool {
    std::env::var_os("PATH")
        .is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join("python3").is_file()))
}

/// A missing Python as the test command: fails closed, so it is kept.
#[test]
fn a_missing_python_alone_is_kept() {
    let modules = asked("alone", &[], &format!("{MISSING_PYTHON} -c pass"));
    assert!(modules.fail_closed.is_some(), "{:?}", modules.fail_closed);
    assert_ne!(modules, Modules::default());
}

/// A missing Python behind `env`.
#[test]
fn a_missing_python_behind_env_is_kept() {
    let modules = asked("env", &[], &format!("env A=1 {MISSING_PYTHON} -c pass"));
    assert!(modules.fail_closed.is_some(), "{:?}", modules.fail_closed);
    assert_ne!(modules, Modules::default());
}

/// A missing Python on a line of the tree's script the test command runs.
#[test]
fn a_missing_python_in_a_tree_script_is_kept() {
    let modules = asked(
        "in-script",
        &[("check.sh", &format!("{MISSING_PYTHON} -c pass\n"))],
        "sh check.sh",
    );
    assert!(modules.fail_closed.is_some(), "{:?}", modules.fail_closed);
    assert_ne!(modules, Modules::default());
}

/// A missing Python run by `exec` on a line of the tree's script.
#[test]
fn a_missing_python_execed_in_a_tree_script_is_kept() {
    let modules = asked(
        "exec-in-script",
        &[("check.sh", &format!("exec {MISSING_PYTHON} -c pass\n"))],
        "sh check.sh",
    );
    assert!(modules.fail_closed.is_some(), "{:?}", modules.fail_closed);
    assert_ne!(modules, Modules::default());
}

/// A Python of the tree, never run at open: fails closed, so it is kept.
#[test]
fn a_tree_python_is_kept() {
    let modules = asked("tree-python", &[], ".venv/bin/python -c pass");
    assert!(modules.fail_closed.is_some(), "{:?}", modules.fail_closed);
    assert_ne!(modules, Modules::default());
}

/// A script of the tree whose `#!` line names the host's `python3`: that Python answers.
#[test]
fn a_tree_script_with_a_python_shebang_is_kept() {
    if !python3_on_path() {
        eprintln!("skipped: no python3 on PATH");
        return;
    }
    let modules = asked(
        "shebang",
        &[("run_tests", "#!/usr/bin/env python3\nprint(1)\n")],
        "./run_tests",
    );
    assert_ne!(modules, Modules::default(), "{:?}", modules.from);
    assert!(
        modules.from.iter().any(|from| from.contains("(Python ")),
        "a Python answered: {:?}",
        modules.from
    );
}

/// A test command that runs no Python gives exactly the built-in lists: no file is kept.
#[test]
fn no_python_is_exactly_the_built_in_lists() {
    let modules = asked(
        "no-python",
        &[("check.sh", "test -f answer.txt\n")],
        "sh check.sh",
    );
    assert_eq!(modules, Modules::default());
}
