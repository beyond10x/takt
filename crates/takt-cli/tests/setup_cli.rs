//! story:declared-setup-runs-before-the-first-check, where `takt run --config` starts it: a
//! command of the run file's `setup` that ends 3 is a `SetupFailed` stop at exit 1, naming the
//! command and 3, before the red-base check and with no model started; one that ends 0 is
//! recorded and the run goes on to its check. The `claude` here is a double that counts the
//! sessions it is started as.

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn git(tree: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(args)
        .current_dir(tree)
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?}");
}

struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn executable(path: &Path, text: &str) {
    fs::write(path, text).expect("a script");
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("exec bit");
}

struct World {
    _scratch: Scratch,
    repo: PathBuf,
    elsewhere: PathBuf,
}

fn world(name: &str) -> World {
    let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("setup-cli-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&outer);
    let repo = outer.join("repo");
    let elsewhere = outer.join("elsewhere");
    for dir in [&repo, &elsewhere] {
        fs::create_dir_all(dir).expect("a directory");
    }
    fs::write(repo.join("README.md"), "a story\n").expect("a file");
    executable(&repo.join("check.sh"), "#!/bin/sh\nexit 1\n");
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "start"]);
    let claude = elsewhere.join("claude");
    executable(
        &claude,
        &"#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then echo double; exit 0; fi\nprintf 'step\\n' >> '@CALLS@'\nexit 1\n"
            .replace("@CALLS@", &elsewhere.join("calls").display().to_string()),
    );
    World {
        _scratch: Scratch(outer),
        repo,
        elsewhere,
    }
}

impl World {
    fn run(&self, setup: &str) -> Output {
        let config = self.elsewhere.join("run.yaml");
        fs::write(&config, format!("test_cmd: ./check.sh\nsetup:\n{setup}")).expect("a file");
        let claude = self.elsewhere.join("claude");
        Command::new(env!("CARGO_BIN_EXE_takt"))
            .args([
                "run",
                "--repo",
                self.repo.to_str().expect("utf-8"),
                "--intent",
                "probe",
                "--harness",
                "claude",
                "--harness-program",
                claude.to_str().expect("utf-8"),
                "--forge",
                "none",
                "--iterations",
                "1",
                "--config",
                config.to_str().expect("utf-8"),
            ])
            .env("XDG_STATE_HOME", self.elsewhere.join("state"))
            .env_remove("B10X_GATES_POLICY")
            .output()
            .expect("takt runs")
    }

    fn steps(&self) -> usize {
        fs::read_to_string(self.elsewhere.join("calls"))
            .unwrap_or_default()
            .lines()
            .count()
    }

    fn events(&self) -> Vec<serde_json::Value> {
        let cases: Vec<PathBuf> = fs::read_dir(self.repo.join(".takt/cases"))
            .expect("the cases")
            .map(|entry| entry.expect("an entry").path())
            .collect();
        assert_eq!(cases.len(), 1);
        fs::read_to_string(cases[0].join("events.jsonl"))
            .expect("the run record")
            .lines()
            .map(|line| serde_json::from_str(line).expect("a line"))
            .collect()
    }
}

fn said(output: &Output) -> String {
    format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// A setup command that exits 3: exit 1, a stop line naming the command and 3, no session
/// started, no test command result in the record.
#[test]
fn a_failed_setup_stops_before_a_model_call() {
    let world = world("failed");
    let output = world.run("  - name: deps\n    argv: [sh, -c, exit 3]\n");
    let said = said(&output);
    assert_eq!(output.status.code(), Some(1), "{said}");
    assert!(
        said.contains("stopped: SetupFailed (deps exited 3)"),
        "{said}"
    );
    assert_eq!(world.steps(), 0, "{said}");
    let events = world.events();
    assert!(
        !events.iter().any(|line| line["event"] == "step"),
        "a step ran: {events:?}"
    );
    assert_eq!(
        events.last().expect("a line")["stop"],
        "SetupFailed (deps exited 3)"
    );
}

/// A setup that ends 0 is recorded, and the run goes on to the red-base check, which stops it
/// (the double's `check.sh` fails) before any session.
#[test]
fn setup_runs_before_the_red_base_check() {
    let world = world("order");
    let output = world.run("  - name: deps\n    argv: [/bin/true]\n");
    let said = said(&output);
    assert_eq!(output.status.code(), Some(3), "{said}");
    assert!(said.contains("stopped: RedBase"), "{said}");
    let events = world.events();
    let setup = events
        .iter()
        .position(|line| line["event"] == "setup")
        .expect("a setup line");
    let check = events
        .iter()
        .position(|line| line["event"] == "step")
        .expect("the check's line");
    assert!(setup < check, "{events:?}");
    assert_eq!(world.steps(), 0, "{said}");
}
