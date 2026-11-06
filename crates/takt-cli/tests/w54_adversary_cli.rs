//! Adversary cases for wave 54, CLI level.
//!
//! - story:resume-names-a-missing-branch: what `takt run --case` does when `takt/<id>` is not
//!   under `refs/heads`, and what its refusal prints and leaves.
//! - story:test-cmd-checked-before-open: the check of `--test-cmd` on a resume runs before the
//!   resume checks the case's branch out.
//! - story:gate-flag-names-a-protocol-action: `--gate` on the paths the story's own tests do
//!   not take.

#[path = "../../takt/tests/support/own_home.rs"]
mod own_home;

use std::collections::BTreeMap;
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
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// A scratch directory under this crate's build directory, removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn repo(&self) -> PathBuf {
        self.0.join("repo")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// A scratch directory holding a committed repository at `<scratch>/repo` with an executable
/// `check.sh`.
fn world(name: &str) -> Scratch {
    use std::os::unix::fs::PermissionsExt as _;
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("w54-adversary-cli-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let repo = dir.join("repo");
    fs::create_dir_all(&repo).expect("the repository");
    let check = repo.join("check.sh");
    fs::write(&check, "#!/bin/sh\nexit 0\n").expect("the test command");
    fs::set_permissions(&check, fs::Permissions::from_mode(0o755)).expect("its mode");
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "start"]);
    Scratch(dir)
}

/// `takt run --repo <repo> --harness none --forge none` with `extra`, run in the repository;
/// `--test-cmd ./check.sh` unless `extra` gives one. Its exit code, stdout and stderr.
fn run(scratch: &Scratch, extra: &[&str]) -> (i32, String, String) {
    let repo = scratch.repo();
    let mut args = vec![
        "run",
        "--repo",
        repo.to_str().expect("utf-8"),
        "--harness",
        "none",
        "--forge",
        "none",
    ];
    if !extra.contains(&"--test-cmd") {
        args.extend(["--test-cmd", "./check.sh"]);
    }
    args.extend_from_slice(extra);
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args(&args)
        .current_dir(&repo)
        .output()
        .expect("takt runs");
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// Opens the case `id` on an intent with no step taken, then puts the tree back on `main`.
fn open(scratch: &Scratch, id: &str) {
    let (code, out, err) = run(
        scratch,
        &["--intent", "probe", "--case-id", id, "--iterations", "0"],
    );
    assert!(
        scratch
            .repo()
            .join(format!(".takt/cases/{id}/case.yaml"))
            .is_file(),
        "the case {id} opens; exit {code}\n{out}\n{err}"
    );
    git(&scratch.repo(), &["checkout", "-q", "main"]);
}

/// Every file under `dir`, by its path relative to `dir`, with its bytes; empty when absent.
fn files(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, into: &mut BTreeMap<PathBuf, Vec<u8>>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries {
            let path = entry.expect("an entry").path();
            if path.is_dir() {
                walk(root, &path, into);
            } else {
                let bytes = fs::read(&path).unwrap_or_default();
                into.insert(
                    path.strip_prefix(root).expect("under the root").to_owned(),
                    bytes,
                );
            }
        }
    }
    let mut into = BTreeMap::new();
    walk(dir, dir, &mut into);
    into
}

/// Every file under `.takt`, every ref with its object, and where `HEAD` is.
fn state(repo: &Path) -> (BTreeMap<PathBuf, Vec<u8>>, String, String) {
    (
        files(&repo.join(".takt")),
        git(repo, &["for-each-ref", "--format=%(refname) %(objectname)"]),
        git(repo, &["rev-parse", "--symbolic-full-name", "HEAD"]),
    )
}

/// story:resume-names-a-missing-branch: a case pushed to `origin` whose local branch was
/// deleted still has `refs/remotes/origin/takt/<id>`. Before this change `git checkout -q
/// takt/<id>` recreated the local branch from it (git's checkout guess) and the case resumed;
/// the new check reads `refs/heads` alone and refuses it as a branch that "does not exist".
#[test]
fn w54_c_a_branch_kept_on_origin_still_resumes() {
    let scratch = world("origin");
    let repo = scratch.repo();
    open(&scratch, "a");
    let tip = git(&repo, &["rev-parse", "refs/heads/takt/a"]);
    let bare = scratch.0.join("origin.git");
    git(
        &scratch.0,
        &["init", "-q", "--bare", bare.to_str().expect("utf-8")],
    );
    git(
        &repo,
        &["remote", "add", "origin", bare.to_str().expect("utf-8")],
    );
    git(&repo, &["push", "-q", "origin", "takt/a"]);
    git(&repo, &["fetch", "-q", "origin"]);
    git(&repo, &["branch", "-q", "-D", "takt/a"]);
    assert_eq!(
        git(&repo, &["rev-parse", "refs/remotes/origin/takt/a"]),
        tip,
        "origin keeps the case's branch"
    );
    let (code, out, err) = run(&scratch, &["--case", "a", "--iterations", "0"]);
    assert!(
        !err.contains("does not exist"),
        "story:resume-names-a-missing-branch: takt/a is on origin, which git checks out as the local branch, and the resume refuses it as missing; exit {code}\n{out}\n{err}"
    );
    assert_eq!(
        git(&repo, &["rev-parse", "--abbrev-ref", "HEAD"]),
        "takt/a",
        "the case resumed on its branch; exit {code}\n{out}\n{err}"
    );
}

/// story:resume-names-a-missing-branch: the refusal prints a `git branch` line to copy. A case
/// id may hold `;` (git and `check_id` accept it); printed unquoted, the line a shell runs is
/// two commands, the second named by the rest of the id.
#[test]
fn w54_c_the_printed_git_branch_line_is_safe_to_paste() {
    let scratch = world("quote");
    let repo = scratch.repo();
    open(&scratch, "x;y");
    git(&repo, &["branch", "-q", "-D", "takt/x;y"]);
    let (code, out, err) = run(&scratch, &["--case", "x;y", "--iterations", "0"]);
    assert_eq!(code, 1, "the resume is refused\n{out}\n{err}");
    assert!(
        err.contains("git branch"),
        "a git branch line is printed\n{err}"
    );
    assert!(
        !err.contains("git branch takt/x;y "),
        "story:resume-names-a-missing-branch: the printed line `git branch takt/x;y <head>` runs `git branch takt/x` and then a command `y` in a shell; it is not quoted\n{err}"
    );
}

/// story:resume-names-a-missing-branch: a tag `takt/a` and no branch: refused as missing, the
/// tree left where it was (before, `git checkout takt/a` detached `HEAD` at the tag).
#[test]
fn w54_c_a_tag_named_like_the_branch_is_no_branch() {
    let scratch = world("tag");
    let repo = scratch.repo();
    open(&scratch, "a");
    git(&repo, &["tag", "takt/a", "refs/heads/takt/a"]);
    git(&repo, &["branch", "-q", "-D", "takt/a"]);
    let before = state(&repo);
    let (code, out, err) = run(&scratch, &["--case", "a", "--iterations", "0"]);
    assert_eq!(code, 1, "{out}\n{err}");
    assert!(err.contains("takt/a"), "{err}");
    assert!(
        state(&repo) == before,
        "nothing changed, HEAD still on main\n{err}"
    );
}

/// story:resume-names-a-missing-branch: a refused resume writes nothing: a `.takt/.gitignore`
/// that is absent stays absent, `case.yaml` keeps its bytes, and no hold is left.
#[test]
fn w54_c_a_refused_resume_writes_no_gitignore_and_no_hold() {
    let scratch = world("nothing");
    let repo = scratch.repo();
    open(&scratch, "a");
    git(&repo, &["branch", "-q", "-D", "takt/a"]);
    let _ = fs::remove_file(repo.join(".takt/.gitignore"));
    let before = state(&repo);
    let (code, out, err) = run(&scratch, &["--case", "a", "--iterations", "0"]);
    assert_eq!(code, 1, "{out}\n{err}");
    assert!(!repo.join(".takt/.gitignore").exists(), "no .gitignore");
    assert!(!repo.join(".takt/cases/a/run.lock").exists(), "no hold");
    assert!(state(&repo) == before, "nothing changed\n{err}");
}

/// story:test-cmd-checked-before-open: the test command is checked before the resume checks
/// the case's branch out. A script the case's branch holds and `main` does not, named on a
/// resume from `main`, is a program the step will start (the step runs on the branch), and
/// the check refuses it.
#[test]
fn w54_b_a_resume_from_another_branch_checks_the_script_on_the_case_branch() {
    use std::os::unix::fs::PermissionsExt as _;
    let scratch = world("resume-script");
    let repo = scratch.repo();
    open(&scratch, "a");
    git(&repo, &["checkout", "-q", "takt/a"]);
    let script = repo.join("run.sh");
    fs::write(&script, "#!/bin/sh\nexit 0\n").expect("a script");
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).expect("its mode");
    git(&repo, &["add", "run.sh"]);
    git(&repo, &["commit", "-q", "-m", "a test script of the case"]);
    git(&repo, &["checkout", "-q", "main"]);
    assert!(!script.exists(), "main has no run.sh");
    let (code, out, err) = run(
        &scratch,
        &["--case", "a", "--test-cmd", "./run.sh", "--iterations", "0"],
    );
    assert!(
        !err.contains("--test-cmd refused"),
        "story:test-cmd-checked-before-open: ./run.sh is on the case's branch, where the step runs it, and the check read main's tree before the resume's checkout; exit {code}\n{out}\n{err}"
    );
}

/// story:gate-flag-names-a-protocol-action: `--gate ""` names no action: refused with exit 1,
/// and nothing at all is written: no `.takt`, no ref, `HEAD` where it was, a clean status.
#[test]
fn w54_d_an_empty_gate_is_refused_and_writes_nothing() {
    let scratch = world("empty-gate");
    let repo = scratch.repo();
    let before = state(&repo);
    let (code, out, err) = run(
        &scratch,
        &["--intent", "probe", "--iterations", "0", "--gate", ""],
    );
    assert_eq!(code, 1, "{out}\n{err}");
    assert!(err.contains("software.change/1"), "{err}");
    assert!(!repo.join(".takt").exists(), "no .takt\n{err}");
    assert!(state(&repo) == before, "nothing changed\n{err}");
    assert_eq!(
        git(&repo, &["status", "--porcelain", "--ignored"]),
        "",
        "a clean tree"
    );
}

/// story:gate-flag-names-a-protocol-action: an action of the protocol that requires no
/// capability is no approval gate: refused, naming the approval-required ones.
#[test]
fn w54_d_an_action_needing_no_capability_is_refused() {
    let protocol = takt::LoadedProtocol::resolve("software.change/1").expect("the protocol");
    let approval = protocol.approval_actions();
    let doc: serde_yaml_ng::Value = serde_yaml_ng::from_str(&protocol.yaml).expect("yaml");
    let free = doc["actions"]
        .as_mapping()
        .expect("actions")
        .keys()
        .filter_map(|key| key.as_str())
        .find(|name| !approval.iter().any(|a| a == name))
        .expect("an action that requires no capability")
        .to_owned();
    let scratch = world("free-action");
    let repo = scratch.repo();
    let (code, out, err) = run(
        &scratch,
        &["--intent", "probe", "--iterations", "0", "--gate", &free],
    );
    assert_eq!(code, 1, "--gate {free}: {out}\n{err}");
    assert!(err.contains(&approval[0]), "{err}");
    assert!(!repo.join(".takt").exists(), "no .takt\n{err}");
}
