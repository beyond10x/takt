//! Adversary pass 2 on `scripts/check-finding`, after correction 1. Its header says the scenario
//! declaration is read "with `git config --no-includes --blob`, as `takt scenario run` reads
//! it". Each case declares scenario 1 in a way `takt scenario run` refuses, shows the takt built
//! from this workspace refusing it, and asks `check-finding` for a result on it: a scenario takt
//! does not run gives no result. Scratch stores and a scratch takt only; nothing reaches a
//! network or a real credential.

#[path = "../../takt/tests/support/own_home.rs"]
mod own_home;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const SCRIPTS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../scripts");
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../takt/tests/fixtures");
const SOURCE: &str = "docs/runs/trial-1.md";

struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            eprintln!("remove the scratch tree {}: {error}", self.0.display());
        }
    }
}

fn scratch(name: &str) -> Scratch {
    let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("finding-adv2-{}-{name}", std::process::id()));
    if outer.exists() {
        fs::remove_dir_all(&outer).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(&outer).expect("a scratch directory");
    Scratch(outer)
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

fn git_out(tree: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(tree)
        .output()
        .expect("git runs");
    assert!(output.status.success(), "git {args:?}");
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().expect("a parent")).expect("a directory");
    fs::write(path, text).expect("a file");
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

fn shown(output: &Output) -> String {
    format!(
        "exit {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn script(name: &str, cwd: &Path, args: &[&str], env: &[(&str, &str)]) -> Output {
    Command::new(Path::new(SCRIPTS).join(name))
        .args(args)
        .current_dir(cwd)
        .envs(env.iter().copied())
        .output()
        .unwrap_or_else(|error| panic!("scripts/{name} runs: {error}"))
}

/// The scratch takt's `main.rs`: `run --repo <dir>` reports the case it would open on
/// `software.change/1` when `<dir>` holds the scenario's story, exit 3; else exit 5.
const TAKT_MAIN: &str = r#"use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let repo = args
        .iter()
        .position(|arg| arg == "--repo")
        .and_then(|at| args.get(at + 1))
        .cloned()
        .unwrap_or_default();
    let story = std::path::Path::new(&repo).join(".engineering/planning/story/ready-one.md");
    if !story.is_file() {
        eprintln!("takt: nothing ready in {repo}");
        return ExitCode::from(5);
    }
    println!("case story-ready-one on software.change/1 in {repo}");
    ExitCode::from(3)
}
"#;

/// A scratch takt source holding the finding `story:default-protocol`, filed through
/// `scripts/file-finding`; a scenario repository whose seed holds `story:ready-one` and whose
/// `main` declares scenario 1 with `section` as its section body, `{seed}` replaced by the seed.
struct World {
    outer: Scratch,
    takt: PathBuf,
    scenarios: PathBuf,
    defect: String,
}

fn world(name: &str, section: &str) -> World {
    let outer = scratch(name);
    let takt = outer.0.join("takt");
    fs::create_dir_all(takt.join(".engineering")).expect("a directory");
    fs::copy(
        Path::new(FIXTURES).join("one-ready/.engineering/project.yaml"),
        takt.join(".engineering/project.yaml"),
    )
    .expect("the project file");
    write(
        &takt.join(SOURCE),
        "# Trial 1\n\nThe story ran on the wrong protocol.\n",
    );
    write(
        &takt.join("Cargo.toml"),
        "[workspace]\nresolver = \"2\"\nmembers = [\"crates/*\"]\n",
    );
    write(
        &takt.join("crates/takt-cli/Cargo.toml"),
        "[package]\nname = \"b10x-takt-cli\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[[bin]]\nname = \"takt\"\npath = \"src/main.rs\"\n",
    );
    write(
        &takt.join("Cargo.lock"),
        "# This file is automatically @generated by Cargo.\n# It is not intended for manual editing.\nversion = 4\n\n[[package]]\nname = \"b10x-takt-cli\"\nversion = \"0.1.0\"\n",
    );
    write(&takt.join(".gitignore"), "/target\n");
    write(&takt.join("crates/takt-cli/src/main.rs"), TAKT_MAIN);
    git(&takt, &["init", "-q", "-b", "main"]);
    git(&takt, &["add", "-A"]);
    git(&takt, &["commit", "-q", "-m", "defect"]);
    let defect = git_out(&takt, &["rev-parse", "HEAD"]);

    let body = outer.0.join("body.md");
    write(
        &body,
        &format!(
            "# The default protocol takes a store story\n\n```expected_before\nscenario: 1\ncommand: \"$TAKT\" run --repo . --harness none\nexit: 3\noutput: on software.change/1\nsource: {SOURCE}\n```\n"
        ),
    );
    let filed = script(
        "file-finding",
        &takt,
        &[
            "--key",
            "default-protocol",
            "--title",
            "A finding",
            "--from",
            body.to_str().expect("UTF-8"),
        ],
        &[],
    );
    assert_eq!(
        String::from_utf8_lossy(&filed.stdout),
        "filed\n",
        "{}",
        shown(&filed)
    );

    let scenarios = outer.0.join("scenarios");
    copy_dir(&Path::new(FIXTURES).join("one-ready"), &scenarios);
    git(&scenarios, &["init", "-q", "-b", "main"]);
    git(&scenarios, &["add", "-A"]);
    git(&scenarios, &["commit", "-q", "-m", "seed"]);
    let seed = git_out(&scenarios, &["rev-parse", "HEAD"]);
    write(
        &scenarios.join(".engineering/scenarios.conf"),
        &format!("[scenario \"1\"]\n{}", section.replace("{seed}", &seed)),
    );
    git(&scenarios, &["add", "-A"]);
    git(&scenarios, &["commit", "-q", "-m", "declare scenario 1"]);
    World {
        outer,
        takt,
        scenarios,
        defect,
    }
}

impl World {
    /// `takt scenario run` of this workspace on scenario 1: it reads the declaration first,
    /// before anything is fetched, checked out or pushed.
    fn takt_scenario_run(&self) -> Output {
        Command::new(env!("CARGO_BIN_EXE_takt"))
            .args([
                "scenario",
                "run",
                "--repo",
                self.scenarios.to_str().expect("UTF-8"),
                "1",
                "--harness",
                "none",
                "--forge",
                "none",
                "--iterations",
                "1",
                "--test-cmd",
                "true",
            ])
            .env_remove("GH_TOKEN")
            .env_remove("GITHUB_TOKEN")
            .output()
            .expect("takt runs")
    }

    fn check(&self) -> Output {
        let cache = self.outer.0.join("cache");
        script(
            "check-finding",
            &self.takt,
            &[
                "--scenarios",
                self.scenarios.to_str().expect("UTF-8"),
                "story:default-protocol",
                &self.defect,
            ],
            &[("XDG_CACHE_HOME", cache.to_str().expect("UTF-8"))],
        )
    }
}

/// `takt scenario run` refuses a declaration giving `seed` twice
/// (crates/takt-cli/src/main.rs:736-740); `git config --get` at scripts/check-finding:129 takes
/// the last value and runs the predicate on it.
#[test]
fn a_scenario_giving_its_seed_twice_gives_no_result() {
    let world = world(
        "seed-twice",
        "\tstory = story:ready-one\n\tseed = {seed}\n\tseed = {seed}\n",
    );
    let takt = world.takt_scenario_run();
    assert!(
        !takt.status.success() && String::from_utf8_lossy(&takt.stderr).contains("twice"),
        "takt refuses the declaration: {}",
        shown(&takt)
    );
    let output = world.check();
    assert!(
        !output.status.success() && output.stdout.is_empty(),
        "a scenario takt scenario run refuses gives no result: {}",
        shown(&output)
    );
}

/// `takt scenario run` refuses a declaration giving a key it does not read
/// (crates/takt-cli/src/main.rs:724-729); scripts/check-finding:129-132 reads `seed` and `story`
/// only and runs the predicate.
#[test]
fn a_scenario_giving_a_key_takt_does_not_read_gives_no_result() {
    let world = world(
        "unknown-key",
        "\tstory = story:ready-one\n\tseed = {seed}\n\tbase = main\n",
    );
    let takt = world.takt_scenario_run();
    assert!(
        !takt.status.success()
            && String::from_utf8_lossy(&takt.stderr).contains("which takt does not read"),
        "takt refuses the declaration: {}",
        shown(&takt)
    );
    let output = world.check();
    assert!(
        !output.status.success() && output.stdout.is_empty(),
        "a scenario takt scenario run refuses gives no result: {}",
        shown(&output)
    );
}
