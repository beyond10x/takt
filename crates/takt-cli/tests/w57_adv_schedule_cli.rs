//! Adversarial cases for `takt run --every` and `--outcome-file` (wave 57,
//! story:a-run-on-a-schedule), through the binary.

#[path = "../../takt/tests/support/own_home.rs"]
mod own_home;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn git(tree: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(args)
        .current_dir(tree)
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?}");
}

/// A scratch directory under this crate's build directory with a committed tree, an empty
/// item file and a state directory.
fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("w57-adv-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let tree = dir.join("tree");
    fs::create_dir_all(&tree).expect("the tree");
    fs::create_dir_all(dir.join("items")).expect("the items");
    fs::create_dir_all(dir.join("state")).expect("the state directory");
    fs::write(dir.join("items/none.json"), "").expect("an empty item file");
    fs::write(tree.join("check.txt"), "broken\n").expect("a file");
    git(&tree, &["init", "-q", "-b", "main"]);
    git(&tree, &["add", "-A"]);
    git(&tree, &["commit", "-q", "-m", "start"]);
    dir
}

/// `takt run --help` says of --outcome-file: "Written on every exit of the command", and
/// main.rs says a run refused before its arguments are read writes one too. A value clap
/// refuses (`--every 0s`) is such an exit: exit 2 and no file.
#[test]
fn w57_adv_outcome_file_on_a_refused_flag() {
    let dir = scratch("parse");
    let outcome = dir.join("outcome.json");
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args(["run", "--repo"])
        .arg(dir.join("tree"))
        .arg("--outcome-file")
        .arg(&outcome)
        .args(["--every", "0s"])
        .stdin(Stdio::null())
        .output()
        .expect("takt runs");
    assert_eq!(output.status.code(), Some(2));
    let written = fs::read_to_string(&outcome);
    let _ = fs::remove_dir_all(&dir);
    let text = written.expect("the outcome file is written on every exit");
    let document: serde_json::Value = serde_json::from_str(&text).expect("JSON");
    assert_eq!(document["exit"], 2, "{document}");
}

/// Starts of one schedule at the same moment: exactly one runs it, every other is refused,
/// exit 1 (design § 3: a second start while `schedule.json` names a live pid is refused).
#[test]
fn w57_adv_simultaneous_starts_run_one_schedule() {
    let dir = scratch("race");
    let config = dir.join("run.toml");
    fs::write(
        &config,
        "work = [\"command\"]\nwork_commands = { select = [\"sleep\", \"2\"] }\n",
    )
    .expect("a run file");
    let starts: Vec<_> = (0..8)
        .map(|_| {
            Command::new(env!("CARGO_BIN_EXE_takt"))
                .args(["run", "--repo"])
                .arg(dir.join("tree"))
                .args([
                    "--harness",
                    "none",
                    "--iterations",
                    "1",
                    "--test-cmd",
                    "true",
                ])
                .arg("--state-dir")
                .arg(dir.join("state"))
                .arg("--config")
                .arg(&config)
                .args(["--every", "1s", "--until", "exit:0"])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .expect("takt runs")
        })
        .collect();
    let codes: Vec<Option<i32>> = starts
        .into_iter()
        .map(|mut child| child.wait().expect("waited on").code())
        .collect();
    let schedules: Vec<PathBuf> = fs::read_dir(dir.join("state/schedule"))
        .expect("a schedule directory")
        .map(|entry| entry.expect("an entry").path())
        .collect();
    let ticks = fs::read_to_string(schedules[0].join("ticks.jsonl")).unwrap_or_default();
    let _ = fs::remove_dir_all(&dir);
    let ran = codes.iter().filter(|code| **code == Some(0)).count();
    assert_eq!(
        ran, 1,
        "one start runs the schedule, the rest are refused: {codes:?}"
    );
    assert_eq!(ticks.lines().count(), 1, "one tick: {ticks}");
}
