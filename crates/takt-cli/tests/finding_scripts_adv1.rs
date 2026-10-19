//! Adversary pass 1 on `scripts/file-finding` and `scripts/check-finding`. Each case drives a
//! script from what its own header, the operator profile's Intake section or the story's Checks
//! say, against scratch stores and a scratch takt; nothing reaches a network or a real credential.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

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
        .join(format!("finding-adv1-{}-{name}", std::process::id()));
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

fn store(at: &Path) -> PathBuf {
    let repo = at.to_path_buf();
    fs::create_dir_all(repo.join(".engineering")).expect("a directory");
    fs::copy(
        Path::new(FIXTURES).join("one-ready/.engineering/project.yaml"),
        repo.join(".engineering/project.yaml"),
    )
    .expect("the project file");
    write(
        &repo.join(SOURCE),
        "# Trial 1\n\nThe story ran on the wrong protocol.\n",
    );
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "seed"]);
    repo
}

fn body(predicate: Option<&str>) -> String {
    let mut text = "# The default protocol takes a store story\n\nA story taken from the store \
                    runs on the wrong protocol.\n"
        .to_owned();
    if let Some(predicate) = predicate {
        text.push_str(&format!("\n```expected_before\n{predicate}```\n"));
    }
    text
}

fn predicate() -> String {
    format!(
        "scenario: 1\ncommand: \"$TAKT\" run --repo . --harness none\nexit: 3\noutput: on software.change/1\nsource: {SOURCE}\n"
    )
}

fn command(name: &str, cwd: &Path, args: &[&str], env: &[(&str, &str)]) -> Command {
    let mut command = Command::new(Path::new(SCRIPTS).join(name));
    command
        .args(args)
        .current_dir(cwd)
        .envs(env.iter().copied());
    command
}

fn script(name: &str, cwd: &Path, args: &[&str], env: &[(&str, &str)]) -> Output {
    command(name, cwd, args, env)
        .output()
        .unwrap_or_else(|error| panic!("scripts/{name} runs: {error}"))
}

fn shown(output: &Output) -> String {
    format!(
        "exit {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn file(repo: &Path, key: &str, text: &str, extra: &[&str]) -> Output {
    let path = repo
        .parent()
        .expect("a parent")
        .join(format!("body-{key}.md"));
    fs::write(&path, text).expect("the body file");
    let from = path.to_str().expect("a UTF-8 path");
    let mut args = vec!["--key", key, "--title", "A finding", "--from", from];
    args.extend_from_slice(extra);
    script("file-finding", repo, &args, &[])
}

fn listed(repo: &Path, key: &str) -> Vec<String> {
    let output = Command::new("aep")
        .args([
            "plan",
            "artifact",
            "list",
            "--ref",
            &format!("finding:{key}"),
        ])
        .current_dir(repo)
        .output()
        .expect("aep runs");
    assert!(output.status.success(), "aep list: {}", shown(&output));
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(str::to_owned)
        .collect()
}

fn show(repo: &Path, id: &str) -> String {
    let output = Command::new("aep")
        .args(["plan", "artifact", "show", id])
        .current_dir(repo)
        .output()
        .expect("aep runs");
    assert!(output.status.success(), "aep show {id}: {}", shown(&output));
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn takt_main(protocol: &str) -> String {
    format!(
        r#"use std::process::ExitCode;

fn main() -> ExitCode {{
    let args: Vec<String> = std::env::args().collect();
    let repo = args
        .iter()
        .position(|arg| arg == "--repo")
        .and_then(|at| args.get(at + 1))
        .cloned()
        .unwrap_or_default();
    let story = std::path::Path::new(&repo).join(".engineering/planning/story/ready-one.md");
    if !story.is_file() {{
        eprintln!("takt: nothing ready in {{repo}}");
        return ExitCode::from(5);
    }}
    println!("case story-ready-one on {protocol} in {{repo}}");
    ExitCode::from(3)
}}
"#
    )
}

struct World {
    outer: Scratch,
    takt: PathBuf,
    scenarios: PathBuf,
    seed: String,
    defect: String,
}

fn world(name: &str) -> World {
    let outer = scratch(name);
    let takt = store(&outer.0.join("takt"));
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
    write(
        &takt.join("crates/takt-cli/src/main.rs"),
        &takt_main("software.change/1"),
    );
    git(&takt, &["add", "-A"]);
    git(&takt, &["commit", "-q", "-m", "defect"]);
    let defect = git_out(&takt, &["rev-parse", "HEAD"]);

    let scenarios = outer.0.join("scenarios");
    copy_dir(&Path::new(FIXTURES).join("one-ready"), &scenarios);
    git(&scenarios, &["init", "-q", "-b", "main"]);
    git(&scenarios, &["add", "-A"]);
    git(&scenarios, &["commit", "-q", "-m", "seed"]);
    let seed = git_out(&scenarios, &["rev-parse", "HEAD"]);
    fs::remove_file(scenarios.join(".engineering/planning/story/ready-one.md"))
        .expect("the story is removed past the seed");
    write(
        &scenarios.join(".engineering/scenarios.conf"),
        &format!("[scenario \"1\"]\n\tstory = story:ready-one\n\tseed = {seed}\n"),
    );
    git(&scenarios, &["add", "-A"]);
    git(&scenarios, &["commit", "-q", "-m", "declare scenario 1"]);
    World {
        outer,
        takt,
        scenarios,
        seed,
        defect,
    }
}

impl World {
    fn cache(&self) -> PathBuf {
        self.outer.0.join("cache")
    }

    fn check_command(&self, finding: &str, env: &[(&str, &str)]) -> Command {
        let cache = self.cache();
        let mut all = vec![("XDG_CACHE_HOME", cache.to_str().expect("UTF-8"))];
        all.extend_from_slice(env);
        command(
            "check-finding",
            &self.takt,
            &[
                "--scenarios",
                self.scenarios.to_str().expect("UTF-8"),
                finding,
                &self.defect,
            ],
            &all,
        )
    }

    fn check(&self, finding: &str, env: &[(&str, &str)]) -> Output {
        self.check_command(finding, env)
            .output()
            .expect("check-finding runs")
    }

    fn file(&self, key: &str, predicate: &str) {
        let filed = file(&self.takt, key, &body(Some(predicate)), &[]);
        assert!(filed.status.success(), "{}", shown(&filed));
    }

    /// Replaces the declaration on the scenario repository's `main` with `text`.
    fn declare(&self, text: &str) {
        write(&self.scenarios.join(".engineering/scenarios.conf"), text);
        git(&self.scenarios, &["add", "-A"]);
        git(&self.scenarios, &["commit", "-q", "-m", "declare again"]);
    }
}

/// Lead 5/9: `takt scenario run` reads `.engineering/scenarios.conf` with `--no-includes`
/// (crates/takt/src/git.rs:190-204), so a scenario declared only through an `include.path` is no
/// scenario to it. `check-finding` reads the same file with `git config --blob --get`
/// (scripts/check-finding:110-112), which follows the include.
#[test]
fn a_scenario_declared_only_through_an_include_gives_no_result() {
    let world = world("include");
    world.file("default-protocol", &predicate());
    let outside = world.outer.0.join("outside.conf");
    write(
        &outside,
        &format!(
            "[scenario \"1\"]\n\tstory = story:ready-one\n\tseed = {}\n",
            world.seed
        ),
    );
    world.declare(&format!(
        "[include]\n\tpath = {}\n",
        outside.to_str().expect("UTF-8")
    ));
    let output = world.check("story:default-protocol", &[]);
    assert!(
        !output.status.success() && output.stdout.is_empty(),
        "takt scenario run declares no scenario 1 here, so the check has no scenario: {}",
        shown(&output)
    );
}

/// Lead 3: the timeout wraps only the predicate. An `include.path` naming a FIFO blocks
/// `git config --blob` at scripts/check-finding:110 with no bound.
#[test]
fn a_scenario_include_naming_a_fifo_does_not_hang_the_check() {
    let world = world("fifo");
    world.file("default-protocol", &predicate());
    let fifo = world.outer.0.join("declared.fifo");
    let made = Command::new("mkfifo")
        .arg(&fifo)
        .status()
        .expect("mkfifo runs");
    assert!(made.success());
    world.declare(&format!(
        "[include]\n\tpath = {}\n",
        fifo.to_str().expect("UTF-8")
    ));
    let mut child: Child = world
        .check_command("story:default-protocol", &[("CHECK_FINDING_TIMEOUT", "5")])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("check-finding starts");
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut finished = None;
    while Instant::now() < deadline {
        if let Some(status) = child.try_wait().expect("wait") {
            finished = Some(status);
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    if finished.is_none() {
        child.kill().expect("kill check-finding");
        child.wait().expect("reap check-finding");
        // Opening the write end releases the git process blocked on the read end.
        drop(
            fs::OpenOptions::new()
                .write(true)
                .open(&fifo)
                .expect("open the FIFO"),
        );
    } else {
        // Already reaped by `try_wait`; `wait` returns the status it kept.
        child.wait().expect("reap check-finding");
    }
    assert!(
        finished.is_some(),
        "check-finding still ran 20 s after it started, blocked on the declaration's include"
    );
}

/// Lead 2: scripts/check-finding:24 says the environment holds only PATH, HOME, LANG, TAKT and
/// SCENARIO, "so no credential of the caller reaches it"; the profile's Intake table says the
/// same. HOME is the caller's, so the caller's git credential helper answers the predicate.
/// Without a credential `git credential fill` dies with 128, the predicate's healthy exit.
#[test]
fn the_predicate_reaches_no_credential_under_home() {
    let world = world("home");
    let real_home = std::env::var("HOME").expect("HOME");
    let rustup = std::env::var("RUSTUP_HOME").unwrap_or_else(|_| format!("{real_home}/.rustup"));
    let cargo = std::env::var("CARGO_HOME").unwrap_or_else(|_| format!("{real_home}/.cargo"));
    let home = world.outer.0.join("home");
    write(&home.join(".gitconfig"), "[credential]\n\thelper = store\n");
    write(
        &home.join(".git-credentials"),
        "https://operator:PLANTED-SECRET@example.invalid\n",
    );
    world.file(
        "credential",
        &format!(
            r#"scenario: 1
command: "$TAKT" --version >/dev/null; printf 'protocol=https\nhost=example.invalid\n\n' | GIT_TERMINAL_PROMPT=0 git credential fill
exit: 0
healthy_exit: 128
output: PLANTED-SECRET
source: {SOURCE}
"#
        ),
    );
    let output = world.check(
        "story:credential",
        &[
            ("HOME", home.to_str().expect("UTF-8")),
            ("RUSTUP_HOME", &rustup),
            ("CARGO_HOME", &cargo),
        ],
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "not_reproduced\n",
        "the caller's stored git credential reached the predicate: {}",
        shown(&output)
    );
}

/// Lead 6: `takt scenario run` cuts `trial/0` in the scratch checkout and leaves it checked out
/// (crates/takt-cli/src/main.rs:869, crates/takt/src/git.rs:150). The profile passes that same
/// checkout as `--scenarios`; its clone then has a local `trial/0` already, and
/// scripts/check-finding:142 `checkout -b trial/0` fails.
#[test]
fn a_scenario_checkout_left_on_trial_0_still_gives_a_result() {
    let world = world("trial-0");
    world.file("default-protocol", &predicate());
    git(
        &world.scenarios,
        &["checkout", "-q", "-b", "trial/0", &world.seed],
    );
    let output = world.check("story:default-protocol", &[]);
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "reproduced\n",
        "{}",
        shown(&output)
    );
}

/// Lead 3: scripts/file-finding:97-100 accepts any exit 0 to 255, and check-finding:155 reads
/// 124 and 137 as a timeout, so a predicate expecting 124 is filed and never gives a result.
#[test]
fn a_predicate_expecting_exit_124_is_checked_or_refused_at_filing() {
    let world = world("exit-124");
    let filed = file(
        &world.takt,
        "exit-124",
        &body(Some(&format!(
            "scenario: 1\ncommand: exit 124\nexit: 124\nsource: {SOURCE}\n"
        ))),
        &[],
    );
    if String::from_utf8_lossy(&filed.stdout) == "malformed\n" {
        return;
    }
    assert!(filed.status.success(), "{}", shown(&filed));
    let output = world.check("story:exit-124", &[]);
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "reproduced\n",
        "a filed predicate whose command exits as it expects: {}",
        shown(&output)
    );
}

/// Lead 7: the header (check-finding:32-35) says a check that cannot run its predicate prints
/// nothing. A CHECK_FINDING_TIMEOUT `timeout` refuses makes `timeout` exit 125 without running
/// the command, and that status is read as the predicate's.
#[test]
fn a_timeout_that_is_no_duration_gives_no_result() {
    let world = world("bad-timeout");
    world.file("default-protocol", &predicate());
    let output = world.check(
        "story:default-protocol",
        &[("CHECK_FINDING_TIMEOUT", "ten")],
    );
    assert!(
        !output.status.success() && output.stdout.is_empty(),
        "the command never ran: {}",
        shown(&output)
    );
}

/// Lead 5: file-finding checks `--from` relative to the caller's directory
/// (scripts/file-finding:139, :147) and then hands the same relative path to
/// `aep plan artifact new` after `cd "$repo"` (:160, :171).
#[test]
fn a_relative_body_is_filed_from_a_subdirectory_of_the_repository() {
    let outer = scratch("relative");
    let repo = store(&outer.0.join("repo"));
    let here = repo.join("docs/runs");
    write(&here.join("finding.md"), &body(Some(&predicate())));
    let output = script(
        "file-finding",
        &here,
        &[
            "--key",
            "default-protocol",
            "--title",
            "A finding",
            "--from",
            "finding.md",
        ],
        &[],
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "filed\n",
        "{}",
        shown(&output)
    );
    assert_eq!(listed(&repo, "default-protocol").len(), 1);
}

/// Lead 5, the same defect where it breaks the story's check "A finding without an
/// `expected_before` block is refused at filing": the predicate parsed is the one in the
/// caller's directory, the body filed is the same-named file at the top of the repository.
#[test]
fn the_body_filed_is_the_body_whose_predicate_was_checked() {
    let outer = scratch("two-bodies");
    let repo = store(&outer.0.join("repo"));
    let here = repo.join("docs/runs");
    write(&here.join("finding.md"), &body(Some(&predicate())));
    write(&repo.join("finding.md"), &body(None));
    let output = script(
        "file-finding",
        &here,
        &[
            "--key",
            "default-protocol",
            "--title",
            "A finding",
            "--from",
            "finding.md",
        ],
        &[],
    );
    if String::from_utf8_lossy(&output.stdout) == "filed\n" {
        let filed = show(&repo, "story:default-protocol");
        assert!(
            filed.contains("```expected_before\n"),
            "filed without a predicate:\n{filed}\n{}",
            shown(&output)
        );
    }
}

/// Lead 1: the refusal is a `list` followed by a `new` with nothing held between them
/// (scripts/file-finding:163-171). A PATH stand-in for `aep` that answers `list` a second late
/// widens that window; two filings of one key, as a story and as a blocker, half a second apart.
#[test]
fn two_filings_of_one_key_a_moment_apart_leave_one_artifact() {
    let outer = scratch("race");
    let repo = store(&outer.0.join("repo"));
    let real = Command::new("sh")
        .args(["-c", "command -v aep"])
        .output()
        .expect("sh runs");
    let real = String::from_utf8_lossy(&real.stdout).trim().to_owned();
    assert!(!real.is_empty(), "aep on PATH");
    let bin = outer.0.join("bin");
    write(
        &bin.join("aep"),
        &format!(
            "#!/bin/sh\n'{real}' \"$@\"\nstatus=$?\nif [ \"$1 $2 $3\" = 'plan artifact list' ]; then sleep 1; fi\nexit $status\n"
        ),
    );
    let made = Command::new("chmod")
        .args(["+x", bin.join("aep").to_str().expect("UTF-8")])
        .status()
        .expect("chmod runs");
    assert!(made.success());
    let path = format!(
        "{}:{}",
        bin.to_str().expect("UTF-8"),
        std::env::var("PATH").expect("PATH")
    );
    let from = outer.0.join("body.md");
    write(&from, &body(Some(&predicate())));
    let from = from.to_str().expect("UTF-8");
    let spawn = |kind: &str| {
        command(
            "file-finding",
            &repo,
            &[
                "--key",
                "default-protocol",
                "--title",
                "A finding",
                "--from",
                from,
                "--kind",
                kind,
            ],
            &[("PATH", &path)],
        )
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("file-finding starts")
    };
    let first = spawn("story");
    std::thread::sleep(Duration::from_millis(500));
    let second = spawn("blocker");
    let first = first.wait_with_output().expect("first filing");
    let second = second.wait_with_output().expect("second filing");
    let held = listed(&repo, "default-protocol");
    assert_eq!(
        held.len(),
        1,
        "one key, one artifact: {held:?}\nfirst: {}\nsecond: {}",
        shown(&first),
        shown(&second)
    );
}
