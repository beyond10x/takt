//! story:bench-subset-and-budget: `takt run --usd <dollars>` stops once the cost the harness
//! reports reaches the budget, the way `--tokens` stops on tokens. On the fixture task of
//! story:bench-task-protocol (`crates/takt/tests/fixtures/bench-task/`, made a git repository
//! here), under `protocols/bench-task/1.yaml`, with a `claude` double first on the program path
//! that reports `"total_cost_usd": 0.02` on every step and never makes `check.sh` pass,
//! `takt run ... --harness claude --forge none --iterations 20 --usd 0.03` exits 3 with a stop
//! line that begins `stopped: Usd (0.03)`. A harness that reports no cost leaves the dollar count
//! unknown: `--usd` stops nothing, and the run says so once. `takt run --help` names `--usd`, and
//! a `--usd` that is no number of dollars above zero does not parse, exit 2. Nothing reaches a
//! network or a model.

#[path = "../../takt/tests/support/own_home.rs"]
mod own_home;

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

/// The repository root, the working directory the protocol's relative path is read from.
const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");

/// The protocol of a benchmark task.
const PROTOCOL: &str = "protocols/bench-task/1.yaml";

/// The fixture task.
const TASK: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../takt/tests/fixtures/bench-task"
);

/// A `claude` double whose every step edits `other.txt`, never `work.txt`, so `check.sh` never
/// passes and no step hands the task over; its result event, as Claude Code ends its stream,
/// reports `COST` as the session's `total_cost_usd`, or no cost when `COST` is replaced by
/// nothing.
const CLAUDE: &str = r#"#!/bin/sh
case $1 in --version) echo '2.1.309 (Claude Code)'; exit 0 ;; esac
echo "more" >> other.txt
printf '{"type":"result","subtype":"success","result":"{\\"action\\": \\"implementation.edit\\", \\"summary\\": \\"the double\\"}","usage":{"input_tokens":1}COST}'
"#;

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

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("a directory");
    for entry in fs::read_dir(from).expect("a readable fixture") {
        let entry = entry.expect("an entry");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("a file type").is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).expect("a copied file");
        }
    }
}

/// The fixture task copied to `<scratch>/task` and committed, the `claude` double reporting
/// `cost` (a JSON member with its leading comma, or nothing) in `<scratch>/bin`, a state home
/// in `<scratch>/state`.
fn world(name: &str, cost: &str) -> (Scratch, PathBuf, PathBuf, PathBuf) {
    let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("usd-budget-cli-{}-{name}", std::process::id()));
    if outer.exists() {
        fs::remove_dir_all(&outer).expect("a stale scratch tree is removed");
    }
    let task = outer.join("task");
    copy_dir(Path::new(TASK), &task);
    git(&task, &["init", "-q", "-b", "main"]);
    git(&task, &["add", "-A"]);
    git(&task, &["commit", "-q", "-m", "the task"]);
    let bin = outer.join("bin");
    fs::create_dir_all(&bin).expect("a program directory");
    let claude = bin.join("claude");
    fs::write(&claude, CLAUDE.replace("COST", cost)).expect("the double is written");
    fs::set_permissions(&claude, fs::Permissions::from_mode(0o755)).expect("executable");
    let state = outer.join("state");
    fs::create_dir_all(&state).expect("a state home");
    (Scratch(outer), task, bin, state)
}

/// `takt run` on the task with the double first on the program path and `extra` arguments.
fn run(task: &Path, bin: &Path, state: &Path, extra: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_takt"))
        .current_dir(ROOT)
        .args(["run", "--repo"])
        .arg(task)
        .args([
            "--protocol",
            PROTOCOL,
            "--intent",
            "make check.sh pass",
            "--test-cmd",
            "sh check.sh",
            "--harness",
            "claude",
            "--forge",
            "none",
        ])
        .args(extra)
        .env("XDG_STATE_HOME", state)
        .env(
            "PATH",
            format!(
                "{}:{}",
                bin.display(),
                std::env::var("PATH").unwrap_or_default()
            ),
        )
        .stdin(Stdio::null())
        .output()
        .expect("takt runs")
}

/// The one `stopped:` line of a run's stdout.
fn stop_line(stdout: &str) -> &str {
    let lines: Vec<&str> = stdout
        .lines()
        .filter(|line| line.starts_with("stopped:"))
        .collect();
    assert_eq!(lines.len(), 1, "one stop line\n{stdout}");
    lines[0]
}

/// The story's acceptance: two steps report 0.02 each, so the count reaches 0.04 after the
/// second, at or over 0.03, and the run stops before a third, `Usd (0.03)`, exit 3; after the
/// first, at 0.02, it does not.
#[test]
fn a_run_stops_once_the_cost_the_harness_reports_reaches_the_dollar_budget() {
    let (_scratch, task, bin, state) = world("stops", r#","total_cost_usd":0.02"#);
    let output = run(
        &task,
        &bin,
        &state,
        &["--iterations", "20", "--usd", "0.03"],
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let said = format!("stdout:\n{stdout}\nstderr:\n{stderr}");
    assert_eq!(output.status.code(), Some(3), "{said}");
    let stop = stop_line(&stdout);
    assert!(stop.starts_with("stopped: Usd (0.03)"), "{said}");
    assert!(
        stop.contains("; steps 2,"),
        "the run stops after the step that brings the cost to the budget, not before\n{said}"
    );
    assert!(
        !stdout.contains("reported no cost"),
        "a harness that reports its cost is not said to report none\n{said}"
    );
}

/// A harness that reports no cost leaves the dollar count unknown, never 0: `--usd` stops
/// nothing, the step budget ends the run, and the run says once that the harness reports no
/// cost.
#[test]
fn a_harness_that_reports_no_cost_never_stops_on_the_dollar_budget() {
    let (_scratch, task, bin, state) = world("unknown", "");
    let output = run(&task, &bin, &state, &["--iterations", "3", "--usd", "0.03"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let said = format!("stdout:\n{stdout}\nstderr:\n{stderr}");
    assert_eq!(output.status.code(), Some(3), "{said}");
    assert!(
        stop_line(&stdout).starts_with("stopped: Iterations (3)"),
        "{said}"
    );
    assert_eq!(
        format!("{stdout}{stderr}")
            .matches("reported no cost")
            .count(),
        1,
        "the run says once that a harness step reported no cost (story:bench-subset-and-budget)\n{said}"
    );
}

/// `takt run --help` names `--usd`.
#[test]
fn run_help_names_the_dollar_budget() {
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args(["run", "--help"])
        .output()
        .expect("takt runs");
    assert!(output.status.success());
    let help = String::from_utf8_lossy(&output.stdout);
    assert!(help.contains("--usd <DOLLARS>"), "{help}");
}

/// A `--usd` that is no number of dollars above zero does not parse: exit 2, before any case
/// opens.
#[test]
fn a_dollar_budget_that_is_no_positive_number_does_not_parse() {
    let (_scratch, task, bin, state) = world("refused", r#","total_cost_usd":0.02"#);
    for value in ["x", "-1", "0", "NaN", "inf", ""] {
        let output = run(&task, &bin, &state, &["--iterations", "1", "--usd", value]);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            output.status.code(),
            Some(2),
            "--usd {value:?} does not parse\n{stderr}"
        );
        assert!(stderr.contains("--usd"), "{stderr}");
        assert!(
            !task.join(".takt/cases").exists(),
            "--usd {value:?} opens no case"
        );
    }
}
