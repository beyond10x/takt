//! story:scenario-run-takes-config: `takt scenario run` takes `--config <FILE>` as `takt run`
//! does: the same TOML or YAML file, a `loom.agent/1` document's `takt` member included, a flag
//! on the command line winning over the file and the file over the flag's default; a key
//! `takt run --config` refuses, and `story`, which the scenario names, are refused before
//! anything is cut or opened. The repository has no `origin`, so nothing is fetched or pushed;
//! no harness runs.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// The fixture stores of the library crate, one directory each.
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../takt/tests/fixtures");

/// A Loom agent document as a `LoomAgent`'s `spec.agent` carries it, before its `takt` member.
const LOOM_AGENT: &str =
    "format: loom.agent/1\nagent:\n  id: takt-a\n  name: takt A\n  brief: delivers stories\n";

fn git(tree: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .current_dir(tree)
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?}");
}

fn git_out(tree: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(tree)
        .output()
        .expect("git runs");
    assert!(output.status.success(), "git {args:?}");
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
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

/// A scratch world under this crate's build directory, removed on drop: a repository whose seed
/// holds the fixture store with `story:ready-one` `proposed` and whose `main` declares scenario
/// `1` on it, and a `config` directory beside it.
struct World(PathBuf);

impl Drop for World {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

impl World {
    fn new(name: &str) -> Self {
        let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("scenario-config-{name}-{}", std::process::id()));
        if outer.exists() {
            fs::remove_dir_all(&outer).expect("a stale scratch tree is removed");
        }
        let world = Self(outer);
        let repo = world.repo();
        copy_dir(&Path::new(FIXTURES).join("one-ready"), &repo);
        fs::create_dir_all(world.config_dir()).expect("a directory");
        git(&repo, &["init", "-q", "-b", "main"]);
        git(&repo, &["add", "-A"]);
        git(&repo, &["commit", "-q", "-m", "seed"]);
        let seed = git_out(&repo, &["rev-parse", "HEAD"]);
        fs::write(
            repo.join(".engineering/scenarios.conf"),
            format!("[scenario \"1\"]\n\tstory = story:ready-one\n\tseed = {seed}\n"),
        )
        .expect("the declaration");
        git(&repo, &["add", "-A"]);
        git(&repo, &["commit", "-q", "-m", "declare the scenarios"]);
        world
    }

    fn repo(&self) -> PathBuf {
        self.0.join("repo")
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

    /// `takt scenario run --repo <repo> 1 --config <config>` with `args` and a passing test
    /// command.
    fn run(&self, config: &Path, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_takt"))
            .args(["scenario", "run", "--repo"])
            .arg(self.repo())
            .args(["1", "--config"])
            .arg(config)
            .args(["--test-cmd", "true"])
            .args(args)
            .env("XDG_STATE_HOME", self.0.join("xdg-state"))
            .env_remove("B10X_GATES_POLICY")
            .env_remove("DBUS_SESSION_BUS_ADDRESS")
            .output()
            .expect("takt runs")
    }

    /// The case directories, none when there is no cases directory.
    fn cases(&self) -> Vec<PathBuf> {
        fs::read_dir(self.repo().join(".engineering/cases"))
            .map(|entries| entries.flatten().map(|entry| entry.path()).collect())
            .unwrap_or_default()
    }

    /// The local branches.
    fn branches(&self) -> String {
        git_out(&self.repo(), &["branch", "--format=%(refname:short)"])
    }
}

fn text(output: &Output) -> String {
    format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

#[test]
fn scenario_run_help_names_config() {
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args(["scenario", "run", "--help"])
        .output()
        .expect("takt runs");
    assert_eq!(output.status.code(), Some(0), "{}", text(&output));
    assert!(
        text(&output).contains("--config <FILE>"),
        "{}",
        text(&output)
    );
}

/// Acceptance 1: `iterations: 0`, `harness: none` and `forge: none` in a `loom.agent/1`
/// document's `takt` member: the trial's case opens and stops `Iterations (0)`, exit 3.
#[test]
fn a_loom_agent_documents_takt_member_sets_the_scenario_run() {
    let world = World::new("loom");
    let config = world.config(
        "agent.yaml",
        &format!("{LOOM_AGENT}takt:\n  iterations: 0\n  harness: none\n  forge: none\n"),
    );
    let output = world.run(&config, &[]);
    let said = text(&output);
    assert_eq!(output.status.code(), Some(3), "{said}");
    assert!(said.contains("stopped: Iterations (0)"), "{said}");
    assert_eq!(world.cases().len(), 1, "{said}");
}

/// Acceptance 2: a flag beside `--config` wins over the file's key of that name: the file's
/// `iterations: 5` under `harness: claude` would start a harness, `--iterations 0 --harness
/// none` stops the trial before any step.
#[test]
fn a_flag_wins_over_the_files_key() {
    let world = World::new("flag");
    let config = world.config("run.yaml", "iterations: 5\nharness: claude\nforge: none\n");
    let output = world.run(&config, &["--iterations", "0", "--harness", "none"]);
    let said = text(&output);
    assert_eq!(output.status.code(), Some(3), "{said}");
    assert!(said.contains("stopped: Iterations (0)"), "{said}");
}

/// Acceptance 2: a key `takt run --config` refuses is refused before anything is cut or opened,
/// exit 1, naming the file and the line and repeating no value.
#[test]
fn a_key_takt_run_refuses_is_refused_before_the_case_opens() {
    let world = World::new("unknown");
    let config = world.config(
        "run.toml",
        "iterations = 0\napi_key = \"MARKER-scenario-config\"\n",
    );
    let branches = world.branches();
    let output = world.run(&config, &["--harness", "none", "--forge", "none"]);
    let said = text(&output);
    assert_eq!(output.status.code(), Some(1), "{said}");
    assert!(said.contains(&config.display().to_string()), "{said}");
    assert!(said.contains("line 2"), "{said}");
    assert!(!said.contains("MARKER-scenario-config"), "{said}");
    assert!(world.cases().is_empty(), "no case was opened");
    assert_eq!(world.branches(), branches, "no trial branch was cut");
}

/// A `story` in the file is refused before anything is cut or opened, exit 1, naming the file
/// and the key: the scenario names its story.
#[test]
fn a_story_in_the_file_is_refused_naming_the_file_and_the_key() {
    let world = World::new("story");
    let config = world.config(
        "run.toml",
        "iterations = 0\nharness = \"none\"\nforge = \"none\"\nstory = \"story:ready-one\"\n",
    );
    let branches = world.branches();
    let output = world.run(&config, &[]);
    let said = text(&output);
    assert_eq!(output.status.code(), Some(1), "{said}");
    assert!(said.contains(&config.display().to_string()), "{said}");
    assert!(said.contains("`story`"), "{said}");
    assert!(world.cases().is_empty(), "no case was opened");
    assert_eq!(world.branches(), branches, "no trial branch was cut");
}

/// story:scan-and-forge-are-adapters: a scenario run scans with `gates` only, so a file that
/// declares `scan: none` is refused before anything is cut or opened, exit 1, naming the file and
/// `scan: none`; `scan: gates` is read as any other field.
#[test]
fn a_declared_scan_none_is_refused_for_a_scenario_run() {
    let world = World::new("scan-none");
    let config = world.config(
        "run.toml",
        "iterations = 0\nharness = \"none\"\nforge = \"none\"\nscan = \"none\"\n",
    );
    let branches = world.branches();
    let output = world.run(&config, &[]);
    let said = text(&output);
    assert_eq!(output.status.code(), Some(1), "{said}");
    assert!(said.contains(&config.display().to_string()), "{said}");
    assert!(said.contains("`scan: none`"), "{said}");
    assert!(world.cases().is_empty(), "no case was opened");
    assert_eq!(world.branches(), branches, "no trial branch was cut");
}
