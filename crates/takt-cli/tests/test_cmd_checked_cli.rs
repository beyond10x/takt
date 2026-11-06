//! story:test-cmd-checked-before-open: `takt run` checks `--test-cmd` before it opens a case. A
//! command whose program cannot be started, or that holds shell quoting, is refused with exit 1,
//! naming the program or saying the command is split at white space without a shell, and no case
//! directory and no `takt/` branch is created. The program is checked where the test step starts
//! it: a path against the tree, a bare name on the `PATH` the step gives the command, never
//! against takt's own working directory.

#[path = "../../takt/tests/support/own_home.rs"]
mod own_home;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

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

/// A scratch directory under this crate's build directory, removed on drop.
struct Scratch(PathBuf);

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

/// Writes `text` at `path`, executable when `executable`.
fn file(path: &Path, text: &str, executable: bool) {
    use std::os::unix::fs::PermissionsExt as _;
    fs::write(path, text).expect("the file");
    let mode = if executable { 0o755 } else { 0o644 };
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).expect("its mode");
}

/// A scratch directory `<scratch>` holding a committed repository at `<scratch>/repo`, with an
/// executable `check.sh` and a non-executable `plain.sh`, and a directory `<scratch>/elsewhere`
/// that holds an executable `no-such-check.sh`, takt's working directory in every run here, so a
/// program checked against takt's own directory instead of the tree is found.
fn world(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("test-cmd-checked-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    let repo = dir.join("repo");
    let elsewhere = dir.join("elsewhere");
    fs::create_dir_all(&repo).expect("the repository");
    fs::create_dir_all(&elsewhere).expect("the other directory");
    file(&repo.join("check.sh"), "#!/bin/sh\nexit 0\n", true);
    file(&repo.join("plain.sh"), "#!/bin/sh\nexit 0\n", false);
    file(
        &elsewhere.join("no-such-check.sh"),
        "#!/bin/sh\nexit 0\n",
        true,
    );
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "start"]);
    Scratch(dir)
}

/// `takt run --intent probe --harness none --forge none` on `<scratch>/repo` with `test_cmd`
/// (the default when none) and `iterations`, from `<scratch>/elsewhere`; its exit code, stdout
/// and stderr.
fn run(scratch: &Scratch, test_cmd: Option<&str>, iterations: &str) -> (i32, String, String) {
    let repo = scratch.0.join("repo");
    let mut args = vec![
        "run",
        "--repo",
        repo.to_str().expect("utf-8"),
        "--intent",
        "probe",
        "--case-id",
        "probe-1",
        "--harness",
        "none",
        "--forge",
        "none",
        "--iterations",
        iterations,
    ];
    if let Some(test_cmd) = test_cmd {
        args.extend(["--test-cmd", test_cmd]);
    }
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args(&args)
        .current_dir(scratch.0.join("elsewhere"))
        .output()
        .expect("takt runs");
    (
        output.status.code().expect("an exit code"),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// Nothing of a case in `<scratch>/repo`: no cases directory entry, no `takt/` branch, the tree
/// still on `main`.
fn nothing_opened(scratch: &Scratch, what: &str) {
    let repo = scratch.0.join("repo");
    let cases = repo.join(".takt/cases");
    let entries: Vec<String> = fs::read_dir(&cases)
        .map(|entries| {
            entries
                .map(|entry| {
                    entry
                        .expect("an entry")
                        .file_name()
                        .to_string_lossy()
                        .into_owned()
                })
                .collect()
        })
        .unwrap_or_default();
    assert!(entries.is_empty(), "{what}: case directories {entries:?}");
    assert_eq!(
        git(&repo, &["branch", "--list", "takt/*"]),
        "",
        "{what}: no case branch"
    );
    assert_eq!(
        git(&repo, &["rev-parse", "--abbrev-ref", "HEAD"]),
        "main",
        "{what}: the tree stays on main"
    );
}

/// The case `probe-1` was opened in `<scratch>/repo`.
fn opened(scratch: &Scratch, what: &str, (code, out, err): &(i32, String, String)) {
    assert!(
        scratch
            .0
            .join("repo/.takt/cases/probe-1/case.yaml")
            .is_file(),
        "{what}: the case opens; exit {code}\n{out}\n{err}"
    );
}

/// The story's predicate: a test command whose program is not in the tree is refused with exit
/// 1 naming the program once, before any case opens, though takt's own working directory holds
/// a program of that name.
#[test]
fn a_test_command_whose_program_is_not_in_the_tree_is_refused_before_the_case_opens() {
    let scratch = world("missing");
    let (code, out, err) = run(&scratch, Some("./no-such-check.sh"), "2");
    assert_eq!(code, 1, "{out}\n{err}");
    assert_eq!(
        err.matches("./no-such-check.sh").count(),
        1,
        "the program named once: {err}"
    );
    nothing_opened(&scratch, "./no-such-check.sh");
}

/// A program path naming a file in the tree that is not executable, and a bare name found on no
/// directory of `PATH`, are refused the same way.
#[test]
fn a_test_program_that_cannot_be_started_is_refused_before_the_case_opens() {
    let scratch = world("unstartable");
    for program in ["./plain.sh", "takt-no-such-test-program"] {
        let (code, out, err) = run(&scratch, Some(program), "2");
        assert_eq!(code, 1, "{program}: {out}\n{err}");
        assert!(err.contains(program), "{program}: {err}");
        nothing_opened(&scratch, program);
    }
}

/// A test command holding `'` or `"` is refused: it is split at white space and run without a
/// shell, so its quoting would reach the program as broken words.
#[test]
fn a_test_command_holding_shell_quoting_is_refused_before_the_case_opens() {
    let scratch = world("quoted");
    for command in [
        "sh -c 'A=1 python3 -m unittest'",
        "sh -c \"A=1 python3 -m unittest\"",
    ] {
        let (code, out, err) = run(&scratch, Some(command), "2");
        assert_eq!(code, 1, "{command}: {out}\n{err}");
        assert!(
            err.contains("split at white space") && err.contains("without a shell"),
            "{command}: {err}"
        );
        nothing_opened(&scratch, command);
    }
}

/// What can be started passes the check and the case opens: a bare name on `PATH`, a script in
/// the tree by its path though takt's working directory has none, and the default
/// `cargo test --quiet`.
#[test]
fn a_test_command_that_can_be_started_gets_past_the_check() {
    for (name, command) in [
        ("path", Some("true")),
        ("tree", Some("./check.sh")),
        ("default", None),
    ] {
        let scratch = world(name);
        let ran = run(&scratch, command, "0");
        opened(&scratch, name, &ran);
    }
}
