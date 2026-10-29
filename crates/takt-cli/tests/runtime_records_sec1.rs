//! Security conformance cases for the runtime-records stories, through the built binary:
//! story:cases-live-under-dot-takt (what a test command does around the cases root) and
//! story:the-state-directory-holds-every-host-record (how a refused state directory is named).
//! Nothing reaches a model or a network.

#[path = "../../takt/tests/support/stand_in_policy.rs"]
mod stand_in_policy;

#[path = "../../takt/tests/support/own_home.rs"]
mod own_home;

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// A scratch directory under this crate's build directory, removed on drop.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn git(tree: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(args)
        .current_dir(tree)
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?}");
}

/// `git <args>` in `tree`; its standard output, trimmed.
fn git_out(tree: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(tree)
        .output()
        .expect("git runs");
    assert!(output.status.success(), "git {args:?}");
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// A committed repository with a README and `files`, at `<scratch>/tree`.
fn repository(name: &str, files: &[(&str, &str)]) -> (Scratch, PathBuf) {
    let outer = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "runtime-records-sec1-{}-{name}",
        std::process::id()
    ));
    if outer.exists() {
        fs::remove_dir_all(&outer).expect("a stale scratch tree is removed");
    }
    let repo = outer.join("tree");
    fs::create_dir_all(&repo).expect("a repository");
    fs::write(repo.join("README.md"), "a story\n").expect("a file");
    for (path, text) in files {
        let path = repo.join(path);
        fs::create_dir_all(path.parent().expect("a parent")).expect("a parent directory");
        fs::write(&path, text).expect("a file");
    }
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "start"]);
    (Scratch(outer), repo)
}

/// Runs the built `takt` binary with `args`: the exit code, stdout and stderr.
fn takt(args: &[OsString]) -> (i32, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args(args)
        .env_remove("B10X_GATES_POLICY")
        .output()
        .expect("takt runs");
    (
        output.status.code().expect("an exit code"),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// `takt run` of one iteration on `repo` with no harness and no forge, the case `id`, the test
/// command `sh tests/run.sh`, and `extra` after.
fn run_args(repo: &Path, id: &str, extra: &[OsString]) -> Vec<OsString> {
    let mut args: Vec<OsString> = [
        "run",
        "--repo",
        repo.to_str().expect("utf-8"),
        "--intent",
        "probe",
        "--case-id",
        id,
        "--harness",
        "none",
        "--forge",
        "none",
        "--iterations",
        "1",
        "--test-cmd",
        "sh tests/run.sh",
    ]
    .iter()
    .map(OsString::from)
    .collect();
    args.extend(extra.iter().cloned());
    args
}

/// A test command runs code a model may have written, and what it leaves is checked against the
/// tree before it. Rewriting `.takt/.gitignore` so that it still ignores itself but no longer
/// the cases (`.gitignore` alone) is an ignored file changed off the judge surface: the
/// observation stands, takt writes its own file again only when it is gone, and the case shows
/// as untracked work in `git status`. The invariant: a test command cannot replace
/// `.takt/.gitignore` without refusal and undo.
#[test]
fn a_test_command_cannot_replace_the_takt_gitignore_without_refusal_and_undo() {
    let (_scratch, repo) = repository(
        "rewrite-ignore",
        &[(
            "tests/run.sh",
            "#!/bin/sh\nprintf '.gitignore\\n' > .takt/.gitignore\n",
        )],
    );
    let (code, out, err) = takt(&run_args(&repo, "rewrite-ignore", &[]));
    assert!(
        repo.join(".takt/cases/rewrite-ignore/case.yaml").is_file(),
        "the case opens under .takt/cases ({code})\nstdout {out}\nstderr {err}"
    );
    assert_eq!(
        fs::read_to_string(repo.join(".takt/.gitignore")).unwrap_or_default(),
        "*\n",
        "takt's .takt/.gitignore is put back after the test command replaced it ({code})\nstdout {out}\nstderr {err}"
    );
    assert_eq!(
        git_out(&repo, &["status", "--porcelain"]),
        "",
        "git status shows none of the run's records ({code})\nstdout {out}\nstderr {err}"
    );
}

/// A test command that replaces `.takt` by a link to an identical copy beside the tree, in a
/// repository whose root `.gitignore` ignores `.takt`: the snapshot of `.takt/cases`, read
/// through the link, sees the copy unchanged, and the link is an ignored path changed off the
/// judge surface, so the observation stands. The run then writes the case's records through the
/// link, outside the tree. The invariant: a test command cannot redirect a case's records
/// through a link without refusal and undo.
#[test]
fn a_test_command_cannot_redirect_the_cases_root_through_a_link() {
    let (scratch, repo) = repository(
        "link-dot-takt",
        &[
            (".gitignore", ".takt\n"),
            (
                "tests/run.sh",
                "#!/bin/sh\n[ -L .takt ] || { cp -r .takt ../copy && rm -rf .takt && ln -s ../copy .takt; }\n",
            ),
        ],
    );
    let (code, out, err) = takt(&run_args(&repo, "link-dot-takt", &[]));
    let copy = scratch.0.join("copy");
    assert!(
        fs::symlink_metadata(repo.join(".takt")).is_ok_and(|meta| meta.is_dir()),
        "after the run .takt is a directory of the tree again, not a link ({code}); the copy outside holds {:?}\nstdout {out}\nstderr {err}",
        fs::read_dir(copy.join("cases/link-dot-takt"))
            .map(|entries| entries
                .filter_map(Result::ok)
                .map(|entry| entry.file_name())
                .collect::<Vec<_>>())
            .unwrap_or_default()
    );
    // The copy is taken by the test command before the observation, so it already holds the
    // case's `decisions/0001.json` (the decision before step 1) and an empty `evidence/`, which
    // the case opens with (story:cases-live-under-dot-takt). What the observation would write
    // through the link is an evidence record, or a decision after `0001.json`.
    let evidence = fs::read_dir(copy.join("cases/link-dot-takt/evidence"))
        .map(|entries| entries.count())
        .unwrap_or(0);
    let decisions: Vec<OsString> = fs::read_dir(copy.join("cases/link-dot-takt/decisions"))
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.file_name())
                .collect()
        })
        .unwrap_or_default();
    assert!(
        evidence == 0
            && decisions
                .iter()
                .all(|name| name.to_str() == Some("0001.json")),
        "story:cases-live-under-dot-takt: no record of the case is written outside the tree, through the link: evidence entries {evidence}, decisions {decisions:?} ({code})\nstdout {out}\nstderr {err}"
    );
}

/// A state directory that cannot be used is refused before anything is opened, the refusal
/// naming it. A name holding a control character (an escape sequence, here ESC `[2J`) must not
/// reach standard error raw, where it would shape the operator's terminal or a log line of its
/// own: refusals name paths with their control characters escaped (`takt::git::shown`).
#[test]
fn a_refused_state_directory_is_named_with_its_control_characters_escaped() {
    let (scratch, repo) = repository("state-escape", &[("tests/run.sh", "#!/bin/sh\nexit 0\n")]);
    let blocker = scratch.0.join("blocker");
    fs::write(&blocker, "a file on the way\n").expect("a file");
    let mut state = blocker.into_os_string();
    state.push("/\u{1b}[2Jstate");
    let (code, out, err) = takt(&run_args(
        &repo,
        "state-escape",
        &[OsString::from("--state-dir"), state],
    ));
    assert_ne!(
        code, 0,
        "the state directory is refused\nstdout {out}\nstderr {err}"
    );
    assert!(
        err.contains("state directory"),
        "the refusal names the state directory ({code})\nstdout {out}\nstderr {err:?}"
    );
    assert!(
        !err.contains('\u{1b}') && !out.contains('\u{1b}'),
        "no raw control character reaches the refusal ({code})\nstdout {out:?}\nstderr {err:?}"
    );
}

/// A run given a state directory keeps its identity scan's text under that directory
/// (story:the-state-directory-holds-every-host-record), so it needs neither `HOME` nor
/// `XDG_STATE_HOME`. The bot route's scan runs `b10x-gates`, here a double on the program path
/// that passes and records what it was given to scan; the scratch directory is removed after.
#[cfg(unix)]
#[test]
fn a_state_directory_holds_the_identity_scan_without_a_home() {
    use std::os::unix::fs::PermissionsExt;

    let (scratch, repo) = repository("identity-scan", &[("tests/run.sh", "#!/bin/sh\nexit 0\n")]);
    git(
        &repo,
        &[
            "remote",
            "add",
            "origin",
            "https://github.com/example/fixture.git",
        ],
    );
    let bin = scratch.0.join("bin");
    fs::create_dir_all(&bin).expect("a bin directory");
    let calls = scratch.0.join("calls.txt");
    let double = bin.join("b10x-gates");
    fs::write(
        &double,
        format!(
            "#!/bin/sh\necho \"$*\" >> \"{}\"\nexit 0\n",
            calls.display()
        ),
    )
    .expect("the gates double");
    fs::set_permissions(&double, fs::Permissions::from_mode(0o755)).expect("executable");
    let state = scratch.0.join("state");
    let path = format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let args: Vec<OsString> = [
        "run",
        "--repo",
        repo.to_str().expect("utf-8"),
        "--intent",
        "probe",
        "--case-id",
        "identity-scan",
        "--harness",
        "none",
        "--forge",
        "bot",
        "--commit-identity",
        "takt <takt@localhost>",
        "--iterations",
        "1",
        "--test-cmd",
        "sh tests/run.sh",
        "--state-dir",
    ]
    .iter()
    .map(OsString::from)
    .chain([state.clone().into_os_string()])
    .collect();
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args(&args)
        .env("B10X_GATES_POLICY", stand_in_policy::path())
        .env_remove("HOME")
        .env_remove("XDG_STATE_HOME")
        .env("PATH", &path)
        .output()
        .expect("takt runs");
    let out = String::from_utf8_lossy(&output.stdout).into_owned();
    let err = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(
        out.contains("step 1:"),
        "the run opens and takes its first step without a home directory\nstdout {out}\nstderr {err}"
    );
    let scanned = fs::read_to_string(&calls).unwrap_or_default();
    assert!(
        scanned.contains(&format!("{}/identity-scans/", state.display())),
        "the identity scan's text is kept under the state directory\nscanned {scanned:?}\nstdout {out}\nstderr {err}"
    );
    assert!(
        fs::read_dir(state.join("identity-scans")).is_ok_and(|mut dirs| dirs.next().is_none()),
        "the identity scan's directory is removed after the scan\nstdout {out}\nstderr {err}"
    );
}
