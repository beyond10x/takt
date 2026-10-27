//! `takt run --hours` ends a harness step when the budget runs out: with a `claude` on the
//! program path that sleeps 20 s and `--hours 0.001` (3.6 s), the run stops on the time budget
//! within the budget and a grace of 10 s, exit 3, the child killed and its change discarded.
//! The `claude` is a double; nothing reaches a model.

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

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

fn executable(path: &Path, text: &str) {
    fs::write(path, text).expect("a script");
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("exec bit");
}

/// Whether process `pid` is still running: listed by `ps`, and not a zombie.
fn alive(pid: &str) -> bool {
    Command::new("ps")
        .args(["-o", "stat=", "-p", pid])
        .output()
        .is_ok_and(|output| {
            let state = String::from_utf8_lossy(&output.stdout);
            let state = state.trim();
            output.status.success() && !state.is_empty() && !state.starts_with('Z')
        })
}

#[test]
fn a_claude_that_sleeps_past_the_hour_budget_is_killed_and_the_run_exits_on_hours() {
    let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("hours-step-cli-{}", std::process::id()));
    if outer.exists() {
        fs::remove_dir_all(&outer).expect("a stale scratch tree is removed");
    }
    let scratch = Scratch(outer.clone());
    let repo = outer.join("repo");
    let bin = outer.join("bin");
    fs::create_dir_all(&repo).expect("a repository");
    fs::create_dir_all(&bin).expect("a program directory");
    fs::write(repo.join("README.md"), "a story\n").expect("a file");
    executable(&repo.join("check.sh"), "#!/bin/sh\nexit 1\n");
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "start"]);
    let pids = outer.join("pids");
    executable(
        &bin.join("claude"),
        &format!(
            "#!/bin/sh\ncase $1 in --version) echo '2.1.309 (Claude Code)'; exit 0 ;; esac\necho stray > stray.txt\nsleep 20 &\necho $$ $! > '{}'\nwait\n",
            pids.display()
        ),
    );

    let started = Instant::now();
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args([
            "run",
            "--continue-past-red-base",
            "--repo",
            repo.to_str().expect("utf-8"),
            "--intent",
            "probe",
            "--harness",
            "claude",
            "--forge",
            "none",
            "--iterations",
            "3",
            "--hours",
            "0.001",
            "--test-cmd",
            "./check.sh",
        ])
        .env(
            "PATH",
            format!(
                "{}:{}",
                bin.display(),
                std::env::var("PATH").unwrap_or_default()
            ),
        )
        .env("XDG_STATE_HOME", outer.join("xdg-state"))
        .env_remove("B10X_GATES_POLICY")
        .output()
        .expect("takt runs");
    let elapsed = started.elapsed();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let budget = Duration::from_secs_f64(0.001 * 3600.0);
    assert!(
        elapsed < budget + Duration::from_secs(10),
        "the run took {elapsed:?}\n{stdout}\n{stderr}"
    );
    assert!(
        stdout.contains("stopped: Hours (0.001)"),
        "the run stops on the hour budget\n{stdout}\n{stderr}"
    );
    assert_eq!(output.status.code(), Some(3), "{stdout}\n{stderr}");
    assert!(
        !repo.join("stray.txt").exists(),
        "the killed step's file is discarded\n{stdout}"
    );
    let pids = fs::read_to_string(&pids).expect("the step started");
    for pid in pids.split_whitespace() {
        assert!(
            !alive(pid),
            "process {pid} of the step still runs\n{stdout}"
        );
    }
    drop(scratch);
}
