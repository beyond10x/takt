//! story:forge-preflight: a run with the bot forge checks that it can reach the forge before the
//! case opens: `b10x-gates` on the program path and `B10X_GATES_POLICY` naming a file. A missing
//! one ends the run with exit 1, naming it, before a case directory is made and so before any
//! step; with both there the run gets past the forge's selection and opens its case, `gh`
//! nowhere on its program path.
//!
//! Every run here sees a program path of its own: a directory of links to every program on
//! this process's `PATH` but `gh` and `b10x-gates`, and, where the case has one, a directory
//! with a `b10x-gates` double that answers nothing and logs every line it is given.

#[path = "../../takt/tests/support/own_home.rs"]
mod own_home;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn git(tree: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(args)
        .current_dir(tree)
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?}");
}

/// A scratch directory under this crate's build directory, removed on drop.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// What one case runs in: the repository, its program directories and its state directory.
struct Setup {
    outer: Scratch,
    repo: PathBuf,
}

impl Setup {
    /// A committed repository at `<scratch>/repo` whose `origin` is on github.com, and the
    /// links to every program on `PATH` but `gh` and `b10x-gates` in `<scratch>/sys-bin`.
    fn new(name: &str) -> Self {
        let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("forge-preflight-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&outer);
        let repo = outer.join("repo");
        fs::create_dir_all(&repo).expect("a directory");
        fs::write(repo.join("README.md"), "a story\n").expect("a file");
        fs::create_dir_all(repo.join(".engineering")).expect("a directory");
        fs::write(
            repo.join(".engineering/commit-identity.conf"),
            "[identity]\n\tname = Test Bot\n\temail = bot@example.invalid\n",
        )
        .expect("the commit identity is written");
        git(&repo, &["init", "-q", "-b", "main"]);
        git(&repo, &["add", "-A"]);
        git(&repo, &["commit", "-q", "-m", "start"]);
        git(
            &repo,
            &[
                "remote",
                "add",
                "origin",
                "https://github.com/owner/repo.git",
            ],
        );
        let sys = outer.join("sys-bin");
        fs::create_dir_all(&sys).expect("a directory");
        for dir in std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()) {
            let Ok(entries) = fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let name = entry.file_name();
                if name == "gh" || name == "b10x-gates" || sys.join(&name).exists() {
                    continue;
                }
                let _ = std::os::unix::fs::symlink(entry.path(), sys.join(&name));
            }
        }
        Self {
            outer: Scratch(outer),
            repo,
        }
    }

    /// A `b10x-gates` double in `<scratch>/stub-bin`: it logs every line it is given to
    /// `<scratch>/b10x-gates.log`, passes `scan-text` (the commit identity's scan at open) and
    /// answers anything else with exit 2, so nothing else it is asked for succeeds and nothing
    /// reaches a host.
    fn with_gates(&self) -> &Self {
        let bin = self.outer.0.join("stub-bin");
        fs::create_dir_all(&bin).expect("a stub directory");
        let program = bin.join("b10x-gates");
        fs::write(
            &program,
            format!(
                "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{}'\n[ \"$1\" = scan-text ] && exit 0\necho \"b10x-gates double: $*\" >&2\nexit 2\n",
                self.outer.0.join("b10x-gates.log").display()
            ),
        )
        .expect("the double");
        {
            use std::os::unix::fs::PermissionsExt as _;
            fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).expect("executable");
        }
        self
    }

    /// A policy file, `<scratch>/policy.json`, at mode `0600`, as `b10x-gates` loads one.
    fn policy(&self) -> PathBuf {
        use std::os::unix::fs::PermissionsExt as _;
        let policy = self.outer.0.join("policy.json");
        fs::write(&policy, "{}\n").expect("a policy file");
        fs::set_permissions(&policy, fs::Permissions::from_mode(0o600)).expect("mode 0600");
        policy
    }

    /// `takt run --forge bot --harness none` on the repository, one step at most, its program
    /// path the stub directory (when there is one) and the links, `B10X_GATES_POLICY` `policy`
    /// or unset: the exit code, stdout and stderr.
    fn run(&self, case: &str, policy: Option<&Path>) -> (i32, String, String) {
        self.run_with_forge(case, policy, &["--forge", "bot"])
    }

    /// [`Setup::run`] with `forge` in place of `--forge bot`: empty, the run takes its default.
    fn run_with_forge(
        &self,
        case: &str,
        policy: Option<&Path>,
        forge: &[&str],
    ) -> (i32, String, String) {
        let path =
            std::env::join_paths([self.outer.0.join("stub-bin"), self.outer.0.join("sys-bin")])
                .expect("a program path");
        let mut command = Command::new(env!("CARGO_BIN_EXE_takt"));
        command
            .args([
                "run",
                "--repo",
                self.repo.to_str().expect("utf-8"),
                "--intent",
                "deliver the story",
                "--case-id",
                case,
                "--harness",
                "none",
                "--test-cmd",
                "true",
                "--iterations",
                "1",
            ])
            .args(forge)
            .env("PATH", path)
            .env("XDG_STATE_HOME", self.outer.0.join("state"));
        match policy {
            Some(policy) => command.env("B10X_GATES_POLICY", policy),
            None => command.env_remove("B10X_GATES_POLICY"),
        };
        let output = command.output().expect("takt runs");
        (
            output.status.code().expect("an exit code"),
            String::from_utf8_lossy(&output.stdout).into_owned(),
            String::from_utf8_lossy(&output.stderr).into_owned(),
        )
    }

    /// Every case directory under the repository's cases directory and the state directory.
    fn case_dirs(&self) -> Vec<PathBuf> {
        let mut found = Vec::new();
        for root in [self.repo.join(".takt/cases"), self.outer.0.join("state")] {
            walk(&root, &mut found);
        }
        found
    }
}

/// Every directory under `dir` that holds a `case.yaml`.
fn walk(dir: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.join("case.yaml").is_file() {
            found.push(path.clone());
        }
        if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            walk(&path, found);
        }
    }
}

/// Acceptance 1: `b10x-gates` absent from the program path: exit 1, no case directory, stderr
/// naming `b10x-gates`.
#[test]
fn no_gates_program_exits_one_before_the_case_opens() {
    let setup = Setup::new("no-gates");
    let policy = setup.policy();
    let (code, out, err) = setup.run("no-gates", Some(&policy));
    assert_eq!(code, 1, "stdout {out}\nstderr {err}");
    assert!(err.contains("b10x-gates"), "stderr names b10x-gates: {err}");
    assert!(
        setup.case_dirs().is_empty(),
        "no case directory: {:?}\nstdout {out}\nstderr {err}",
        setup.case_dirs()
    );
    assert!(!out.contains("case "), "no case opened: {out}");
}

/// Acceptance 2: `B10X_GATES_POLICY` unset, or naming no file: exit 1, no case directory,
/// stderr naming `B10X_GATES_POLICY`; the double was asked for nothing but the identity's scan.
#[test]
fn no_policy_file_exits_one_before_the_case_opens() {
    let setup = Setup::new("no-policy");
    setup.with_gates();
    let missing = setup.outer.0.join("no-such-policy.json");
    let directory = setup.outer.0.join("stub-bin");
    for (case, policy) in [
        ("unset", None),
        ("missing", Some(missing.as_path())),
        ("empty", Some(Path::new(""))),
        ("directory", Some(directory.as_path())),
    ] {
        let (code, out, err) = setup.run(case, policy);
        assert_eq!(code, 1, "{case}: stdout {out}\nstderr {err}");
        assert!(
            err.contains("B10X_GATES_POLICY"),
            "{case}: stderr names B10X_GATES_POLICY: {err}"
        );
        assert!(
            setup.case_dirs().is_empty(),
            "{case}: no case directory: {:?}\nstdout {out}\nstderr {err}",
            setup.case_dirs()
        );
    }
    let log = fs::read_to_string(setup.outer.0.join("b10x-gates.log")).unwrap_or_default();
    assert!(
        log.lines().all(|line| line.starts_with("scan-text ")),
        "nothing but scan-text: {log}"
    );
}

/// Acceptance 3: `b10x-gates` on the program path and `B10X_GATES_POLICY` naming a file, no
/// `gh` anywhere on it: the run gets past the forge's selection, says `forge: bot`, and its
/// case directory is made.
#[test]
fn with_both_and_no_gh_the_case_opens() {
    let setup = Setup::new("both");
    setup.with_gates();
    assert!(!setup.outer.0.join("sys-bin/gh").exists());
    assert!(!setup.outer.0.join("stub-bin/gh").exists());
    let policy = setup.policy();
    let (code, out, err) = setup.run("both", Some(&policy));
    assert!(
        out.contains("forge: bot"),
        "exit {code}\nstdout {out}\nstderr {err}"
    );
    assert!(
        !setup.case_dirs().is_empty(),
        "a case directory\nexit {code}\nstdout {out}\nstderr {err}"
    );
}

/// With a GitHub origin and no `--forge`, the run's forge is the bot route, through the same
/// preflight: `forge: bot (owner/repo)`, and nothing is pushed in one step. Moved here from the
/// CLI's unit tests, which run in one process and cannot give the run its own program path.
#[test]
fn with_an_origin_the_default_forge_is_the_bot_route() {
    let setup = Setup::new("default-bot");
    setup.with_gates();
    let policy = setup.policy();
    let (code, out, err) = setup.run_with_forge("default-bot", Some(&policy), &[]);
    assert!(
        out.contains("forge: bot (owner/repo)"),
        "exit {code}\nstdout {out}\nstderr {err}"
    );
    assert!(!out.contains("pr.open {}"), "nothing is pushed: {out}");
}
