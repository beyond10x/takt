//! story:bench-task-protocol: takt runs a benchmark task under `protocols/bench-task/1.yaml`,
//! `bench.task/1`. On the fixture task `crates/takt/tests/fixtures/bench-task/`, made a git
//! repository here, whose `check.sh` exits 0 only once `work.txt` exists, with a `claude` double
//! first on the program path whose first step writes `work.txt`, `takt run --protocol
//! protocols/bench-task/1.yaml --test-cmd "sh check.sh" --harness claude --forge none` exits 0
//! and stops `Completed (completed)`. The file declares no forge action, so `--forge none` leaves
//! none of them to the harness. Nothing reaches a network or a model.

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// How long the run may take before it is killed: it takes a few steps.
const LIMIT: Duration = Duration::from_secs(120);

/// The repository root, the working directory the protocol's relative path is read from.
const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");

/// The protocol, as the story's acceptance names it.
const PROTOCOL: &str = "protocols/bench-task/1.yaml";

/// The fixture task.
const TASK: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../takt/tests/fixtures/bench-task"
);

/// A `claude` double, as `scenario_run.rs` builds one: each step writes `work.txt` and names
/// `implementation.edit`, but one offered `change.handover` once `work.txt` exists, which hands
/// the task over and changes no file.
const CLAUDE: &str = r#"#!/bin/sh
case $1 in --version) echo '2.1.309 (Claude Code)'; exit 0 ;; esac
case "$2" in
  *"These need an approval"*) action=repository.merge ;;
  *"- change.handover:"*)
    if [ -e work.txt ]; then action=change.handover;
    else echo "the work" >> work.txt; action=implementation.edit; fi ;;
  *) echo "the work" >> work.txt; action=implementation.edit ;;
esac
printf '{"result":"{\\"action\\": \\"%s\\", \\"summary\\": \\"the double\\"}","usage":{"input_tokens":1}}' "$action"
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

/// The fixture task copied to `<scratch>/task` and committed, the `claude` double in
/// `<scratch>/bin`, a state home in `<scratch>/state`.
fn world(name: &str) -> (Scratch, PathBuf, PathBuf, PathBuf) {
    let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("bench-task-cli-{}-{name}", std::process::id()));
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
    fs::write(&claude, CLAUDE).expect("the double is written");
    fs::set_permissions(&claude, fs::Permissions::from_mode(0o755)).expect("executable");
    let state = outer.join("state");
    fs::create_dir_all(&state).expect("a state home");
    (Scratch(outer), task, bin, state)
}

/// The story's acceptance: the run on `bench.task/1` exits 0 with the stop line
/// `stopped: Completed (completed)`, the visible test command passing on the double's work.
#[test]
fn a_benchmark_task_completes_once_its_visible_tests_pass() {
    let (_scratch, task, bin, state) = world("completes");
    assert_ne!(
        Command::new("sh")
            .arg("check.sh")
            .current_dir(&task)
            .status()
            .expect("sh runs")
            .code(),
        Some(0),
        "the task's visible test fails before the run"
    );
    let stdout_file = task.with_file_name("stdout");
    let stderr_file = task.with_file_name("stderr");
    let mut child = Command::new(env!("CARGO_BIN_EXE_takt"))
        .current_dir(ROOT)
        .args(["run", "--repo"])
        .arg(&task)
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
        .env_remove("B10X_GATES_POLICY")
        .env("XDG_STATE_HOME", &state)
        .env(
            "PATH",
            format!(
                "{}:{}",
                bin.display(),
                std::env::var("PATH").unwrap_or_default()
            ),
        )
        .stdin(Stdio::null())
        .stdout(fs::File::create(&stdout_file).expect("a stdout file"))
        .stderr(fs::File::create(&stderr_file).expect("a stderr file"))
        .spawn()
        .expect("takt runs");
    // The acceptance's command has no step budget, so a protocol that never reaches its outcome
    // would step forever: past `LIMIT` the run is killed and the test fails.
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().expect("the run is waited on") {
            break Some(status);
        }
        if started.elapsed() > LIMIT {
            child.kill().expect("the run is killed");
            child.wait().expect("the killed run is reaped");
            break None;
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    let stdout = fs::read_to_string(&stdout_file).expect("stdout");
    let stderr = fs::read_to_string(&stderr_file).expect("stderr");
    let status = status.unwrap_or_else(|| {
        panic!("the run did not stop within {LIMIT:?}\nstdout:\n{stdout}\nstderr:\n{stderr}")
    });
    assert_eq!(
        status.code(),
        Some(0),
        "stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stdout.contains("stopped: Completed (completed)"),
        "stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        task.join("work.txt").is_file(),
        "the double's work is in the task"
    );
}

/// "No forge" is a property of the file: `bench.task/1` declares no `pr.open`, `ci.read` or
/// `repository.merge`, which `--forge none` would leave to the harness; its one obligation is
/// `tests.pass`, which only the visible test command's `tests.run` decides, beside
/// `change.handed_over`, the harness's choice that gates that run; and its outcomes are
/// `completed` and `abandoned`.
#[test]
fn the_benchmark_protocol_declares_no_forge_action() {
    let path = Path::new(ROOT).join(PROTOCOL);
    let loaded = takt::protocol::LoadedProtocol::from_file(&path).expect("the protocol loads");
    assert_eq!(loaded.id, "bench.task/1");
    for forge in ["pr.open", "ci.read", "repository.merge"] {
        assert!(
            !loaded.actions.iter().any(|action| action == forge),
            "{forge} is declared: {:?}",
            loaded.actions
        );
    }
    let doc: serde_yaml_ng::Value =
        serde_yaml_ng::from_str(&loaded.yaml).expect("the protocol reads");
    let keys = |section: &str| -> Vec<String> {
        doc[section]
            .as_mapping()
            .map(|map| {
                map.keys()
                    .filter_map(|key| key.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default()
    };
    assert_eq!(keys("claims"), ["tests.pass", "change.handed_over"]);
    assert_eq!(keys("obligations"), ["tests.required"]);
    assert_eq!(
        doc["obligations"]["tests.required"]["discharged_when"]["claim"].as_str(),
        Some("tests.pass")
    );
    assert_eq!(keys("outcomes"), ["completed", "abandoned"]);
    let producers: Vec<String> = doc["actions"]
        .as_mapping()
        .expect("actions")
        .iter()
        .filter(|(_, action)| {
            action["may_produce"]
                .as_sequence()
                .is_some_and(|kinds| kinds.iter().any(|kind| kind["evidence"] == "test_result"))
        })
        .filter_map(|(id, _)| id.as_str().map(str::to_owned))
        .collect();
    assert_eq!(
        producers,
        ["tests.run"],
        "only the test command's validator writes test_result"
    );
    assert!(
        doc["actions"]
            .as_mapping()
            .expect("actions")
            .values()
            .all(|action| action["requires"].is_null()),
        "no action needs a capability, so no step waits for a grant"
    );
}
