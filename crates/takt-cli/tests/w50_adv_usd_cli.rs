//! Adversary pass on story:bench-subset-and-budget: `takt run --usd <dollars>` against the
//! story's acceptance and its edges, on the fixture task of story:bench-task-protocol with a
//! `claude` double first on the program path. Nothing reaches a network or a model.

#[path = "../../takt/tests/support/own_home.rs"]
mod own_home;

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
const PROTOCOL: &str = "protocols/bench-task/1.yaml";
const TASK: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../takt/tests/fixtures/bench-task"
);

/// A `claude` double that appends a line to `other.txt` (never `work.txt`, so `check.sh` never
/// passes) and ends its stream with a result event; `COST` is replaced by the shell text that
/// prints the cost member (with its leading comma) for this step, or nothing. `$n` is the
/// number of lines `other.txt` holds after this step's append.
const CLAUDE: &str = r#"#!/bin/sh
case $1 in --version) echo '2.1.309 (Claude Code)'; exit 0 ;; esac
echo "more" >> other.txt
n=$(wc -l < other.txt)
cost=COST
printf '{"type":"result","subtype":"success","result":"{\\"action\\": \\"implementation.edit\\", \\"summary\\": \\"the double\\"}","usage":{"input_tokens":1}%s}' "$cost"
"#;

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

/// The fixture task committed under `<scratch>/task`, the double with `cost` (shell text) in
/// `<scratch>/bin`, a state home in `<scratch>/state`.
fn world(name: &str, cost: &str) -> (Scratch, PathBuf, PathBuf, PathBuf) {
    let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("w50-adv-usd-{}-{name}", std::process::id()));
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

/// A cost member reported on every step.
fn every_step(usd: &str) -> String {
    format!(r#"',"total_cost_usd":{usd}'"#)
}

/// `takt run --repo <task> <args>` with the double first on the program path.
fn takt(task: &Path, bin: &Path, state: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_takt"))
        .current_dir(ROOT)
        .args(["run", "--repo"])
        .arg(task)
        .args(args)
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

/// As the unit's own test runs it: `--protocol protocols/bench-task/1.yaml` and `extra`.
fn run(task: &Path, bin: &Path, state: &Path, extra: &[&str]) -> Output {
    let mut args = vec![
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
    ];
    args.extend_from_slice(extra);
    takt(task, bin, state, &args)
}

fn said(output: &Output) -> String {
    format!(
        "exit {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn stop_line(output: &Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout
        .lines()
        .filter(|line| line.starts_with("stopped:"))
        .collect();
    assert_eq!(lines.len(), 1, "one stop line\n{}", said(output));
    lines[0].to_owned()
}

/// The acceptance's command as the story writes it, on the fixture task of
/// story:bench-task-protocol, read with that task's protocol, `bench.task/1` (the coordinator's
/// reading): it exits 3 with a stop line that begins `stopped: Usd (0.03)`.
#[test]
fn the_acceptance_command_on_the_fixture_tasks_protocol_stops_on_the_dollar_budget() {
    let (_scratch, task, bin, state) = world("literal", &every_step("0.02"));
    let output = takt(
        &task,
        &bin,
        &state,
        &[
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
            "--iterations",
            "20",
            "--usd",
            "0.03",
        ],
    );
    assert_eq!(
        output.status.code(),
        Some(3),
        "story:bench-subset-and-budget acceptance: its command is read with the fixture task's protocol, bench.task/1\n{}",
        said(&output)
    );
    assert!(
        stop_line(&output).starts_with("stopped: Usd (0.03)"),
        "story:bench-subset-and-budget acceptance: its command is read with the fixture task's protocol, bench.task/1\n{}",
        said(&output)
    );
}

/// Three steps of one cent each have reported three cents: the run stops `Usd (0.03)` after
/// the third step, not after a fourth.
#[test]
fn three_steps_of_a_cent_reach_a_budget_of_three_cents() {
    let (_scratch, task, bin, state) = world("cents", &every_step("0.01"));
    let output = run(
        &task,
        &bin,
        &state,
        &["--iterations", "20", "--usd", "0.03"],
    );
    assert_eq!(output.status.code(), Some(3), "{}", said(&output));
    let stop = stop_line(&output);
    assert!(stop.starts_with("stopped: Usd (0.03)"), "{}", said(&output));
    assert!(
        stop.contains("; steps 3,"),
        "0.01 + 0.01 + 0.01 reported is the budget of 0.03 reached (story:bench-subset-and-budget)\n{}",
        said(&output)
    );
}

/// Ten steps of ten cents have reported one dollar: the run stops `Usd (1)` after the tenth
/// step. Summed in binary floating point the ten reports come to 0.9999999999999999, under 1,
/// and an eleventh step would be paid for.
#[test]
fn ten_steps_of_ten_cents_reach_a_budget_of_one_dollar() {
    let (_scratch, task, bin, state) = world("dimes", &every_step("0.1"));
    let output = run(&task, &bin, &state, &["--iterations", "20", "--usd", "1"]);
    assert_eq!(output.status.code(), Some(3), "{}", said(&output));
    let stop = stop_line(&output);
    assert!(stop.starts_with("stopped: Usd (1)"), "{}", said(&output));
    assert!(
        stop.contains("; steps 10,"),
        "ten reports of 0.1 are the budget of 1 reached; the run must not take an 11th step (story:bench-subset-and-budget)\n{}",
        said(&output)
    );
}

/// A harness that reports a cost on some steps and not on others: 0.02, none, 0.02. The count
/// reaches 0.04 after the third step and the run stops there; the unreported step is said once.
#[test]
fn a_harness_reporting_cost_on_some_steps_stops_on_what_it_reported() {
    let cost = r#"$(if [ $((n % 2)) -eq 1 ]; then echo ',"total_cost_usd":0.02'; fi)"#;
    let (_scratch, task, bin, state) = world("partial", cost);
    let output = run(
        &task,
        &bin,
        &state,
        &["--iterations", "20", "--usd", "0.03"],
    );
    assert_eq!(output.status.code(), Some(3), "{}", said(&output));
    let stop = stop_line(&output);
    assert!(stop.starts_with("stopped: Usd (0.03)"), "{}", said(&output));
    assert!(stop.contains("; steps 3,"), "{}", said(&output));
    let all = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        all.matches("reported no cost").count(),
        1,
        "the line reads `a harness step reported no cost` since others did (story:bench-subset-and-budget)\n{}",
        said(&output)
    );
}

/// A second `takt run` on the case: the dollar count restarts with the invocation, as the
/// story says, so the resumed run takes two more steps (cost $0.04) before it stops again.
#[test]
fn a_resumed_run_counts_its_dollars_from_zero() {
    let (_scratch, task, bin, state) = world("resume", &every_step("0.02"));
    let first = run(
        &task,
        &bin,
        &state,
        &["--iterations", "20", "--usd", "0.03"],
    );
    assert_eq!(first.status.code(), Some(3), "{}", said(&first));
    let cases: Vec<String> = fs::read_dir(task.join(".takt/cases"))
        .expect("the case directory")
        .map(|entry| {
            entry
                .expect("an entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert_eq!(cases.len(), 1, "{cases:?}");
    let second = takt(
        &task,
        &bin,
        &state,
        &[
            "--case",
            &cases[0],
            "--test-cmd",
            "sh check.sh",
            "--harness",
            "claude",
            "--forge",
            "none",
            "--iterations",
            "20",
            "--usd",
            "0.03",
        ],
    );
    assert_eq!(second.status.code(), Some(3), "{}", said(&second));
    let stop = stop_line(&second);
    assert!(stop.starts_with("stopped: Usd (0.03)"), "{}", said(&second));
    assert!(
        stop.contains("cost $0.04"),
        "the resumed invocation counts its own two steps (story:bench-subset-and-budget)\n{}",
        said(&second)
    );
}

/// `--usd` and `--tokens` both reached by the same step: one stop line, never two.
#[test]
fn a_run_reaching_both_budgets_at_once_prints_one_stop() {
    let (_scratch, task, bin, state) = world("both", &every_step("0.02"));
    let output = run(
        &task,
        &bin,
        &state,
        &["--iterations", "20", "--usd", "0.03", "--tokens", "2"],
    );
    assert_eq!(output.status.code(), Some(3), "{}", said(&output));
    let stop = stop_line(&output);
    assert!(
        stop.starts_with("stopped: Tokens (2)") || stop.starts_with("stopped: Usd (0.03)"),
        "{}",
        said(&output)
    );
}
