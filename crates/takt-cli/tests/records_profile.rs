//! story:a-run-keeps-every-decision-only-on-a-trial: the records profile a run keeps its case's
//! decisions under, `--records default|trial` or the run configuration's `records`. `default`
//! keeps the latest `decisions/NNNN.json` alone and the numbering goes on from it; `trial` keeps
//! every one. A scenario run and a `loop.improvement/1` case keep every one whatever the flag
//! says, and the `opened` line names the profile the case keeps. The `claude` is a double;
//! nothing reaches a model.

#[path = "../../takt/tests/support/own_home.rs"]
mod own_home;

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// The fixture stores of the library crate, one directory each.
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../takt/tests/fixtures");

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

fn executable(path: &Path, text: &str) {
    fs::write(path, text).expect("a script");
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("exec bit");
}

/// The `claude` double: it writes a file and answers `tests.write`, each call.
const CLAUDE: &str = "#!/bin/sh\ncase $1 in --version) echo '2.1.309 (Claude Code)'; exit 0 ;; esac\nmkdir -p tests\necho work >> tests/work.txt\nprintf '%s\\n' '{\"type\":\"result\",\"subtype\":\"success\",\"result\":\"{\\\"action\\\": \\\"tests.write\\\", \\\"summary\\\": \\\"wrote a test\\\"}\",\"usage\":{\"input_tokens\":3,\"output_tokens\":4}}'\n";

/// A scratch world under this crate's build directory, removed on drop: a repository with one
/// commit, a program directory holding the `claude` double, a `config` directory and an
/// `XDG_STATE_HOME` of its own.
struct World(PathBuf);

impl Drop for World {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

impl World {
    fn new(name: &str) -> Self {
        let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("records-profile-{name}-{}", std::process::id()));
        if outer.exists() {
            fs::remove_dir_all(&outer).expect("a stale scratch tree is removed");
        }
        let world = Self(outer);
        for dir in [world.repo(), world.bin(), world.config_dir()] {
            fs::create_dir_all(dir).expect("a directory");
        }
        fs::write(world.repo().join("README.md"), "a story\n").expect("a file");
        git(&world.repo(), &["init", "-q", "-b", "main"]);
        git(&world.repo(), &["add", "-A"]);
        git(&world.repo(), &["commit", "-q", "-m", "start"]);
        executable(&world.bin().join("claude"), CLAUDE);
        world
    }

    fn repo(&self) -> PathBuf {
        self.0.join("repo")
    }

    fn bin(&self) -> PathBuf {
        self.0.join("bin")
    }

    fn config_dir(&self) -> PathBuf {
        self.0.join("config")
    }

    /// Writes `text` as `config/<name>` and returns its path.
    fn config(&self, name: &str, text: &str) -> PathBuf {
        let path = self.config_dir().join(name);
        fs::write(&path, text).expect("a run configuration file");
        path
    }

    /// `takt run` on the repository with an intent, two iterations, a passing test command and
    /// the `claude` double, and `args` added.
    fn run(&self, args: &[&str]) -> Output {
        let mut all = vec!["run", "--intent", "probe"];
        all.extend(args);
        self.takt(&all)
    }

    /// `takt run --case <case>` on the repository: a resumed case, with the world's settings.
    fn resume(&self, case: &str) -> Output {
        self.takt(&["run", "--case", case])
    }

    /// `takt` with `args`, a subcommand first, on the repository, the `claude` double first on
    /// its path, with the harness, forge, iterations and test command the runs of this world
    /// share.
    fn takt(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_takt"))
            .args(args)
            .args(["--repo", self.repo().to_str().expect("utf-8")])
            .args(["--harness", "claude", "--forge", "none"])
            .args(["--iterations", "2", "--test-cmd", "true"])
            .env("PATH", self.path())
            .env("XDG_STATE_HOME", self.0.join("xdg-state"))
            .env_remove("DBUS_SESSION_BUS_ADDRESS")
            .output()
            .expect("takt runs")
    }

    fn path(&self) -> String {
        format!(
            "{}:{}",
            self.bin().display(),
            std::env::var("PATH").unwrap_or_default()
        )
    }

    /// The one case directory the repository holds.
    fn case(&self) -> PathBuf {
        let cases: Vec<PathBuf> = fs::read_dir(self.repo().join(".takt/cases"))
            .expect("the cases directory")
            .map(|entry| entry.expect("an entry").path())
            .collect();
        assert_eq!(cases.len(), 1, "{cases:?}");
        cases[0].clone()
    }

    /// The names under the case's `decisions/`, sorted.
    fn decisions(&self) -> Vec<String> {
        decisions_of(&self.case())
    }

    /// The run record's lines, each a JSON object.
    fn record(&self) -> Vec<serde_json::Value> {
        events_of(&self.case())
    }
}

/// The names under `case`'s `decisions/`, sorted.
fn decisions_of(case: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(case.join("decisions"))
        .expect("the decisions directory")
        .map(|entry| {
            entry
                .expect("an entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    names
}

/// The run record of `case`, one JSON object a line.
fn events_of(case: &Path) -> Vec<serde_json::Value> {
    fs::read_to_string(case.join("events.jsonl"))
        .expect("the run record")
        .lines()
        .map(|line| serde_json::from_str(line).expect("a JSON line"))
        .collect()
}

/// The decision lines of a run record: how many decisions the run evaluated.
fn evaluated(record: &[serde_json::Value]) -> usize {
    record
        .iter()
        .filter(|line| line["event"] == "decision")
        .count()
}

fn text(output: &Output) -> String {
    format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// Acceptance 1: a run of three evaluations with no `--records` leaves one decision, the
/// latest, numbered 0003; a resumed run reads it and goes on from its number. The resume keeps
/// the decision the earlier run kept, since it wrote none of it, and leaves only its own latest
/// (story:a-run-keeps-every-decision-only-on-a-trial). `takt approve` reads the kept latest
/// decision.
#[test]
fn default_keeps_the_latest_decision() {
    let world = World::new("default");
    let output = world.run(&[]);
    let said = text(&output);
    assert_eq!(evaluated(&world.record()), 3, "{said}");
    assert_eq!(world.decisions(), ["0003.json"], "{said}");
    let case = world
        .case()
        .file_name()
        .expect("a name")
        .to_string_lossy()
        .into_owned();
    let resumed = world.resume(&case);
    let said = text(&resumed);
    let kept = world.decisions();
    assert_eq!(
        kept.len(),
        2,
        "story:a-run-keeps-every-decision-only-on-a-trial: {kept:?}\n{said}"
    );
    assert_eq!(
        kept[0], "0003.json",
        "story:a-run-keeps-every-decision-only-on-a-trial: an earlier run's decision was removed: {kept:?}\n{said}"
    );
    assert!(kept[1].as_str() > "0003.json", "{kept:?}\n{said}");
    let approved = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args(["approve", "--repo"])
        .arg(world.repo())
        .args([&case, "repository.merge", "--by", "human:operator"])
        .env("PATH", world.path())
        .env("XDG_STATE_HOME", world.0.join("xdg-state"))
        .output()
        .expect("takt approve runs");
    let said = text(&approved);
    assert_eq!(approved.status.code(), Some(1), "{said}");
    assert!(
        said.contains("its latest decision finds repository.merge blocked, not approval-required"),
        "takt approve reads the kept latest decision and names its status: {said}"
    );
}

/// Acceptance 2: the same run with `--records trial` leaves every decision, 0001 to 0003.
#[test]
fn trial_keeps_every_decision() {
    let world = World::new("trial");
    let output = world.run(&["--records", "trial"]);
    let said = text(&output);
    assert_eq!(evaluated(&world.record()), 3, "{said}");
    assert_eq!(
        world.decisions(),
        ["0001.json", "0002.json", "0003.json"],
        "{said}"
    );
}

/// Acceptance 3: `records = "trial"` in the run configuration keeps every decision; the flag
/// `--records default` wins over it; any other value is refused before the case opens, by the
/// file and by the flag, naming what it takes.
#[test]
fn config_key_and_flag() {
    let world = World::new("config-trial");
    let config = world.config("run.toml", "records = \"trial\"\n");
    let output = world.run(&["--config", config.to_str().expect("utf-8")]);
    let said = text(&output);
    assert_eq!(
        world.decisions(),
        ["0001.json", "0002.json", "0003.json"],
        "{said}"
    );

    let world = World::new("config-flag");
    let config = world.config("run.toml", "records = \"trial\"\n");
    let output = world.run(&[
        "--config",
        config.to_str().expect("utf-8"),
        "--records",
        "default",
    ]);
    let said = text(&output);
    assert_eq!(world.decisions(), ["0003.json"], "{said}");

    let world = World::new("config-bad");
    let config = world.config("run.toml", "records = \"sometimes\"\n");
    let output = world.run(&["--config", config.to_str().expect("utf-8")]);
    let said = text(&output);
    assert_eq!(output.status.code(), Some(1), "{said}");
    assert!(said.contains(&config.display().to_string()), "{said}");
    assert!(said.contains("`records`"), "{said}");
    assert!(!said.contains("sometimes"), "{said}");
    assert!(
        !world.repo().join(".takt/cases").exists(),
        "no case was opened\n{said}"
    );

    let world = World::new("flag-bad");
    let output = world.run(&["--records", "sometimes"]);
    let said = text(&output);
    assert_eq!(output.status.code(), Some(2), "{said}");
    assert!(
        !world.repo().join(".takt/cases").exists(),
        "no case was opened\n{said}"
    );
}

/// Acceptance 4, the scenario run: `takt scenario run` keeps every decision whatever
/// `--records default` says, and so does a `loop.improvement/1` case, which is an improvement
/// case and reads every decision for its scorecard. A `self.improvement/1` turn keeps them by
/// the same rule, which the unit test `improvement_cases_keep_every_decision_whatever_is_asked`
/// in `crates/takt/src/case.rs` covers: a turn's CLI run needs the install root and scorecard
/// fixture of `self_improvement_cli.rs`, so no turn is run here.
#[test]
fn scenario_and_improvement_cases_keep_every_decision() {
    let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("records-profile-scenario-{}", std::process::id()));
    if outer.exists() {
        fs::remove_dir_all(&outer).expect("a stale scratch tree is removed");
    }
    let repo = outer.join("repo");
    let copy = Path::new(FIXTURES).join("one-ready");
    copy_dir(&copy, &repo);
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
    let bin = outer.join("bin");
    fs::create_dir_all(&bin).expect("a directory");
    executable(&bin.join("claude"), CLAUDE);
    let path = format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args(["scenario", "run", "--repo"])
        .arg(&repo)
        .args([
            "1",
            "--harness",
            "claude",
            "--forge",
            "none",
            "--iterations",
            "2",
            "--test-cmd",
            "true",
            "--records",
            "default",
        ])
        .env("PATH", &path)
        .env("XDG_STATE_HOME", outer.join("xdg-state"))
        .output()
        .expect("takt runs");
    let said = text(&output);
    let scenario = only_case(&repo);
    let record = events_of(&scenario);
    let evaluated_here = evaluated(&record);
    assert!(evaluated_here >= 2, "{said}");
    assert_eq!(decisions_of(&scenario).len(), evaluated_here, "{said}");
    assert_eq!(opened_line(&record)["records"], "trial", "{said}");

    let turns = outer.join("loop-repo");
    fs::create_dir_all(turns.join("protocols/loop-improvement")).expect("a directory");
    fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../protocols/loop-improvement/metrics.yaml"),
        turns.join("protocols/loop-improvement/metrics.yaml"),
    )
    .expect("the metric definitions");
    git(&turns, &["init", "-q", "-b", "main"]);
    git(&turns, &["add", "-A"]);
    git(&turns, &["commit", "-q", "-m", "start"]);
    let head = git(&turns, &["rev-parse", "HEAD"]);
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args([
            "run",
            "--repo",
            turns.to_str().expect("utf-8"),
            "--protocol",
            "loop.improvement/1",
            "--intent",
            "raise the loop's accepted share",
            "--case-id",
            "loop-1",
            "--harness",
            "none",
            "--forge",
            "none",
            "--test-cmd",
            "true",
            "--iterations",
            "1",
            "--records",
            "default",
            "--input",
            &format!("loop_baseline={head}"),
        ])
        .env("XDG_STATE_HOME", outer.join("xdg-state"))
        .output()
        .expect("takt runs");
    let said = text(&output);
    let turn = turns.join(".takt/cases/loop-1");
    let evaluated_there = evaluated(&events_of(&turn));
    assert!(evaluated_there >= 1, "{said}");
    assert_eq!(decisions_of(&turn).len(), evaluated_there, "{said}");
    let _ = fs::remove_dir_all(&outer);
}

/// The one case directory under `repo`.
fn only_case(repo: &Path) -> PathBuf {
    let cases: Vec<PathBuf> = fs::read_dir(repo.join(".takt/cases"))
        .expect("the cases directory")
        .map(|entry| entry.expect("an entry").path())
        .collect();
    assert_eq!(cases.len(), 1, "{cases:?}");
    cases[0].clone()
}

/// Copies the directory `from` to `to`, every file under it.
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

/// Acceptance 5: the `opened` line carries `records`, the profile the case keeps: `default` for
/// a default run, `trial` for a trial run, and `trial` for a scenario run whatever the flag.
#[test]
fn opened_names_the_profile() {
    let world = World::new("opened-default");
    world.run(&[]);
    let opened = opened_line(&world.record());
    assert_eq!(opened["records"], "default", "{opened}");

    let world = World::new("opened-trial");
    world.run(&["--records", "trial"]);
    let opened = opened_line(&world.record());
    assert_eq!(opened["records"], "trial", "{opened}");
}

/// The first `opened` line of a run record.
fn opened_line(record: &[serde_json::Value]) -> serde_json::Value {
    record
        .iter()
        .find(|line| line["event"] == "opened")
        .cloned()
        .expect("an opened line")
}

/// Acceptance 6: a replay of a step whose decision a `default` run dropped exits non-zero, and
/// names the profile and `--records trial`.
#[test]
fn replay_of_a_dropped_decision_is_refused() {
    let world = World::new("replay");
    let output = world.run(&[]);
    let said = text(&output);
    assert!(world.decisions().len() == 1, "{said}");
    let case = world.case();
    let replay = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args([
            "replay",
            "--case",
            case.to_str().expect("utf-8"),
            "--step",
            "2",
        ])
        .env("PATH", world.path())
        .env("XDG_STATE_HOME", world.0.join("xdg-state"))
        .output()
        .expect("takt replay runs");
    let said = text(&replay);
    assert_ne!(replay.status.code(), Some(0), "{said}");
    assert!(said.contains("the default records profile"), "{said}");
    assert!(said.contains("--records trial"), "{said}");
}
