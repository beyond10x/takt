//! Adversary cases for story:the-default-confinement-is-named-as-the-default.
//!
//! - A run without `--confinement` says on the `case` line's confinement segment, not only
//!   somewhere in the tree's path, that `none` is the default, and never prints `--confinement`.
//! - Its `opened` event records the same: the adapter `none` and a reason that names the default.
//! - Apart from that reason, a run without the flag records the same adapter as one with
//!   `--confinement none`: the same Substrate adapter, families and facts.
//! - `takt run --help` still says which confinement a run gets when the flag is left out.

#[path = "../../takt/tests/support/own_home.rs"]
mod own_home;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// A scratch directory whose path holds none of the words the cases look for.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn scratch(name: &str) -> Scratch {
    let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("adv-dc-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&outer);
    fs::create_dir_all(&outer).expect("scratch");
    Scratch(outer)
}

fn repository(outer: &Path) -> PathBuf {
    let repo = outer.join("repo");
    fs::create_dir_all(&repo).expect("repo");
    fs::write(repo.join("seed"), "seed").expect("seed");
    for args in [
        vec!["init", "-q", "-b", "main"],
        vec!["add", "seed"],
        vec![
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@localhost",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-q",
            "-m",
            "seed",
        ],
    ] {
        assert!(
            Command::new("git")
                .args(args)
                .current_dir(&repo)
                .status()
                .expect("git")
                .success()
        );
    }
    repo
}

fn takt_run(outer: &Path, repo: &Path, extra: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_takt"))
        .args(["run", "--repo"])
        .arg(repo)
        .args([
            "--intent",
            "probe",
            "--harness",
            "none",
            "--forge",
            "none",
            "--iterations",
            "1",
            "--test-cmd",
            "true",
        ])
        .args(extra)
        .env("XDG_STATE_HOME", outer.join("state"))
        .output()
        .expect("takt runs")
}

fn shown(output: &Output) -> String {
    format!(
        "stdout {}\nstderr {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// The `case` line's text from `; confinement ` on: what it says of the confinement, without
/// the tree's path before it.
fn confinement_segment(output: &Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout
        .lines()
        .find(|line| line.starts_with("case "))
        .unwrap_or_else(|| panic!("no case line: {}", shown(output)));
    let at = line
        .rfind("; confinement ")
        .unwrap_or_else(|| panic!("no confinement segment: {line}"));
    line[at..].to_owned()
}

/// The `opened` event of the one case under `outer`.
fn opened(outer: &Path) -> serde_json::Value {
    fn find(dir: &Path, into: &mut Vec<PathBuf>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                find(&path, into);
            } else if path.file_name().is_some_and(|name| name == "events.jsonl") {
                into.push(path);
            }
        }
    }
    let mut found = Vec::new();
    find(outer, &mut found);
    let events = found
        .iter()
        .find_map(|path| fs::read_to_string(path).ok())
        .unwrap_or_else(|| panic!("an events record under {}", outer.display()));
    let line = events
        .lines()
        .find(|line| line.contains("\"event\":\"opened\""))
        .unwrap_or_else(|| panic!("an opened line in {events}"));
    serde_json::from_str(line).expect("json")
}

/// Acceptance 1, on the segment that names the confinement: the run's own fixture in
/// `run_console.rs` puts the word `default` in the tree's path, which the `case` line prints, so
/// its `line.contains("default")` holds whatever the confinement segment says.
#[test]
fn a_default_run_says_default_in_its_confinement_segment() {
    let outer = scratch("plain");
    let repo = repository(&outer.0);
    let output = takt_run(&outer.0, &repo, &[]);
    let segment = confinement_segment(&output);
    assert!(
        segment.starts_with("; confinement none"),
        "the segment names none: {segment}"
    );
    assert!(
        segment.contains("default"),
        "acceptance 1: the confinement segment says none is the default: {segment}"
    );
    assert!(
        !segment.contains("--confinement"),
        "acceptance 1: no --confinement on the line: {segment}"
    );
}

/// Acceptance 1 carried to the record: the `opened` event of a run without the flag names `none`
/// and a declared-insecure reason that says it is the default and prints no `--confinement`.
#[test]
fn a_default_run_records_the_default_in_its_opened_event() {
    let outer = scratch("plain-opened");
    let repo = repository(&outer.0);
    let output = takt_run(&outer.0, &repo, &[]);
    let opened = opened(&outer.0);
    assert_eq!(
        opened["confinement"],
        "none",
        "{opened}\n{}",
        shown(&output)
    );
    let reason = opened["substrate"]["declared_insecure"]
        .as_str()
        .unwrap_or_else(|| panic!("a declared-insecure reason: {opened}"));
    assert!(reason.contains("default"), "{reason}");
    assert!(!reason.contains("--confinement"), "{reason}");
}

/// Question 1: a run without the flag and one with `--confinement none` record the same
/// adapter, Substrate adapter, families and facts, and their reasons differ only before the
/// first `:`.
#[test]
fn a_default_run_and_a_flagged_run_record_the_same_adapter() {
    let plain = scratch("same-plain");
    let flagged = scratch("same-flagged");
    let plain_repo = repository(&plain.0);
    let flagged_repo = repository(&flagged.0);
    let a = takt_run(&plain.0, &plain_repo, &[]);
    let b = takt_run(&flagged.0, &flagged_repo, &["--confinement", "none"]);
    assert_eq!(
        a.status.code(),
        b.status.code(),
        "{}\n{}",
        shown(&a),
        shown(&b)
    );
    let a = opened(&plain.0);
    let b = opened(&flagged.0);
    assert_eq!(a["confinement"], b["confinement"]);
    for key in ["adapter", "inner", "families", "facts"] {
        assert_eq!(a["substrate"][key], b["substrate"][key], "{key}: {a}\n{b}");
    }
    let tail = |value: &serde_json::Value| {
        value["substrate"]["declared_insecure"]
            .as_str()
            .and_then(|reason| reason.split_once(": ").map(|(_, tail)| tail.to_owned()))
    };
    assert_eq!(tail(&a), tail(&b), "{a}\n{b}");
    assert!(tail(&a).is_some(), "{a}");
}

/// The help of `takt run` is where an operator learns which confinement a run without the flag
/// gets. Before this unit clap printed `[default: none]` for `--confinement`; with the
/// default removed from the argument, the entry must say it in its own text.
#[test]
fn the_run_help_says_which_confinement_is_the_default() {
    for subcommand in [vec!["run"], vec!["scenario", "run"]] {
        let output = Command::new(env!("CARGO_BIN_EXE_takt"))
            .args(&subcommand)
            .arg("--help")
            .output()
            .expect("takt runs");
        let help = String::from_utf8_lossy(&output.stdout);
        let start = help
            .find("--confinement <")
            .unwrap_or_else(|| panic!("{subcommand:?}: a --confinement entry: {help}"));
        let rest = &help[start..];
        let end = rest[1..].find("\n      --").map_or(rest.len(), |at| at + 1);
        let entry = &rest[..end];
        assert!(
            entry.contains("[default: none]")
                || entry.contains("`none` (default)")
                || entry.contains("default is `none`")
                || entry.contains("by default"),
            "{subcommand:?}: the --confinement entry names none as the default:\n{entry}"
        );
    }
}

/// Acceptance 2 in every spelling of the flag and with a `--config` file beside it: the flag
/// given is named as given, in the `case` line and the `opened` event; without it, with or
/// without a file, the default is named. Pass 2: the flag's origin is read from clap's value
/// source in `LoopArgs::configured`, which runs on both the file and the no-file branch.
#[test]
fn the_flag_is_told_apart_from_the_default_in_every_spelling() {
    let config_dir = scratch("config");
    let config = config_dir.0.join("run.toml");
    fs::write(&config, "iterations = 1\n").expect("a run configuration file");
    let config = config.to_str().expect("utf-8");
    for (label, extra, given) in [
        ("eq", vec!["--confinement=none"], true),
        (
            "cfg-flag",
            vec!["--config", config, "--confinement", "none"],
            true,
        ),
        (
            "flag-cfg",
            vec!["--confinement", "none", "--config", config],
            true,
        ),
        ("cfg-only", vec!["--config", config], false),
    ] {
        let outer = scratch(&format!("spell-{label}"));
        let repo = repository(&outer.0);
        let output = takt_run(&outer.0, &repo, &extra);
        let segment = confinement_segment(&output);
        let reason = opened(&outer.0)["substrate"]["declared_insecure"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        if given {
            assert!(
                segment.contains("--confinement none") && !segment.contains("default"),
                "{label}: the flag is named as given: {segment}"
            );
            assert_eq!(reason, takt::confinement::LOCAL_REASON, "{label}");
        } else {
            assert!(
                segment.contains("default") && !segment.contains("--confinement"),
                "{label}: the default is named: {segment}"
            );
            assert!(
                reason.contains("default") && !reason.contains("--confinement"),
                "{label}: {reason}"
            );
        }
    }
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

fn git(tree: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .current_dir(tree)
        .output()
        .expect("git runs");
    assert!(output.status.success(), "git {args:?}");
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// `takt scenario run` shares `LoopArgs` with `takt run` and reaches `confine` through
/// `ScenarioRunArgs::configured`: without the flag its `case` line names the default, with
/// `--confinement none` the flag.
#[test]
fn a_scenario_run_tells_the_flag_from_the_default() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../takt/tests/fixtures/one-ready");
    for (label, extra, given) in [
        ("scn-plain", vec![], false),
        ("scn-flag", vec!["--confinement", "none"], true),
    ] {
        let outer = scratch(label);
        let repo = outer.0.join("repo");
        copy_dir(&fixtures, &repo);
        git(&repo, &["init", "-q", "-b", "main"]);
        git(&repo, &["add", "-A"]);
        git(&repo, &["commit", "-q", "-m", "seed"]);
        let seed = git(&repo, &["rev-parse", "HEAD"]);
        fs::write(
            repo.join(".engineering/scenarios.conf"),
            format!("[scenario \"1\"]\n\tstory = story:ready-one\n\tseed = {seed}\n"),
        )
        .expect("the declaration");
        git(&repo, &["add", "-A"]);
        git(&repo, &["commit", "-q", "-m", "declare the scenarios"]);
        let output = Command::new(env!("CARGO_BIN_EXE_takt"))
            .args(["scenario", "run", "--repo"])
            .arg(&repo)
            .args([
                "1",
                "--iterations",
                "0",
                "--harness",
                "none",
                "--forge",
                "none",
                "--test-cmd",
                "true",
            ])
            .args(&extra)
            .env("XDG_STATE_HOME", outer.0.join("state"))
            .env_remove("B10X_GATES_POLICY")
            .env_remove("DBUS_SESSION_BUS_ADDRESS")
            .output()
            .expect("takt runs");
        let segment = confinement_segment(&output);
        if given {
            assert!(
                segment.contains("--confinement none") && !segment.contains("default"),
                "{label}: {segment}"
            );
        } else {
            assert!(
                segment.contains("default") && !segment.contains("--confinement"),
                "{label}: {segment}"
            );
        }
    }
}
