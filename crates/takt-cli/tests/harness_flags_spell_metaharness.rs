//! story:harness-flags-spell-metaharness-names: `takt run` and `takt scenario run` take the five
//! session settings of the harness mapping as flags under the names `metaharness run` takes; each
//! flag reaches the session as the same key in the run file does, and wins over that key; a
//! setting metaharness refuses for the harness, such as `max-turns` under codex, is refused before
//! any case opens with metaharness's own refusal. The harness programs here are doubles on the
//! program path that leave the argv they were started with, and the Codex configuration they were
//! given, behind. No run reaches a network: each run stops at its first step, the double exiting
//! before it does anything.

#[path = "../../takt/tests/support/own_home.rs"]
mod own_home;

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const FLAGS: [&str; 5] = [
    "--effort",
    "--max-turns",
    "--max-budget-usd",
    "--strict-version",
    "--context",
];

/// The fixture stores of the library crate, one directory each.
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../takt/tests/fixtures");

/// A gates document whose one assertion is true in a repository with a manifest: without one a
/// scenario run's delivery stops `Unperformable (gate.green)` before any session step.
const GATES: &str = "format: engineering-gates/1\nlanguage: canon-expr/1\ngates:\n  - id: manifest\n    \
                     text: The repository has its manifest.\n    assertions:\n      \
                     - 'fs.file.exists(\"Cargo.toml\")'\n";

/// The version the doubles report: one of metaharness 0.23.0's Claude Code pins
/// (`metaharness-claude` `PINNED_VERSIONS`: 2.1.304, 2.1.305, 2.1.309), and outside its Codex
/// pins (`metaharness-codex` `PINNED_VERSIONS`: 0.153.4, 0.160.0).
const DOUBLE_VERSION: &str = "2.1.309";

/// The text of `notes.md`, the context file the repositories here commit.
const NOTE: &str = "MARKER-note: the session reads this note before it starts\n";

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

fn text(output: &Output) -> String {
    format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// A run file of one step, no forge, no scan adapter and a test command that passes, with the
/// given `harness` member written as it is.
fn run_file(harness: &str) -> String {
    format!("iterations: 2\nforge: none\ntest_cmd: \"true\"\nscan: none\n{harness}")
}

/// The value written after `flag` in `argv`, when it is there.
fn value_after(argv: &[String], flag: &str) -> Option<String> {
    let at = argv.iter().position(|arg| arg == flag)?;
    argv.get(at + 1).cloned()
}

/// A scratch tree under this crate's build directory, removed on drop: a repository, an empty
/// HOME whose `.local/bin` holds the codex double, a program directory holding the claude double,
/// and a config directory.
struct World(PathBuf);

impl Drop for World {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

impl World {
    /// The world for `takt run`: one commit holding `README.md` and `notes.md`, the doubles
    /// reporting [`DOUBLE_VERSION`].
    fn new(name: &str) -> Self {
        Self::bare(name).committed()
    }

    /// This world's repository given one commit holding `README.md` and `notes.md`.
    fn committed(self) -> Self {
        fs::write(self.repo().join("README.md"), "a story\n").expect("a file");
        fs::write(self.repo().join("notes.md"), NOTE).expect("a file");
        git(&self.repo(), &["init", "-q", "-b", "main"]);
        git(&self.repo(), &["add", "-A"]);
        git(&self.repo(), &["commit", "-q", "-m", "start"]);
        self
    }

    /// The world for `takt scenario run`: the seed holds the fixture store with
    /// `story:ready-one` `proposed`, a manifest, a gates document that holds for it ([`GATES`])
    /// and `notes.md`; `main` moves on past it with the declaration of scenario `1` on that story
    /// and seed. The doubles report [`DOUBLE_VERSION`].
    fn scenario(name: &str) -> Self {
        let world = Self::bare(name);
        let repo = world.repo();
        copy_dir(&Path::new(FIXTURES).join("one-ready"), &repo);
        fs::write(repo.join("Cargo.toml"), "[package]\nname = \"scratch\"\n").expect("a file");
        fs::write(repo.join(".engineering/gates.yaml"), GATES).expect("a file");
        fs::write(repo.join("notes.md"), NOTE).expect("a file");
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
        git(&repo, &["commit", "-q", "-m", "declare the scenario"]);
        world
    }

    /// The directories and the doubles, the repository directory empty. Each double answers
    /// `--version` with [`DOUBLE_VERSION`]; started for a session, it copies the `config.toml` of
    /// the `CODEX_HOME` it was given beside its argv file, writes its argv one argument a line, and
    /// exits 1.
    fn bare(name: &str) -> Self {
        let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("harness-flags-{name}-{}", std::process::id()));
        if outer.exists() {
            fs::remove_dir_all(&outer).expect("a stale scratch tree is removed");
        }
        let world = Self(outer);
        for dir in [world.repo(), world.home(), world.bin(), world.config_dir()] {
            fs::create_dir_all(dir).expect("a directory");
        }
        for program in ["claude", "codex"] {
            let dir = if program == "codex" {
                world.home().join(".local/bin")
            } else {
                world.bin()
            };
            fs::create_dir_all(&dir).expect("a directory");
            let double = dir.join(program);
            fs::write(
                &double,
                format!(
                    "#!/bin/sh\ncase $1 in --version) echo '{DOUBLE_VERSION} (Claude Code)'; exit 0 ;; esac\nif [ -n \"$CODEX_HOME\" ] && [ -f \"$CODEX_HOME/config.toml\" ]; then cp \"$CODEX_HOME/config.toml\" '{}'; fi\nfor arg in \"$@\"; do printf '%s\\n' \"$arg\"; done > '{}'\nexit 1\n",
                    world.config_copy(program).display(),
                    world.started(program).display()
                ),
            )
            .expect("a double");
            fs::set_permissions(&double, fs::Permissions::from_mode(0o755)).expect("exec bit");
        }
        world
    }

    /// Where `program`'s double copies the `config.toml` of the CODEX_HOME it was given.
    fn config_copy(&self, program: &str) -> PathBuf {
        self.0.join(format!("started-{program}-config.toml"))
    }

    fn repo(&self) -> PathBuf {
        self.0.join("repo")
    }

    fn home(&self) -> PathBuf {
        self.0.join("home")
    }

    fn bin(&self) -> PathBuf {
        self.0.join("bin")
    }

    fn config_dir(&self) -> PathBuf {
        self.0.join("config")
    }

    fn started(&self, program: &str) -> PathBuf {
        self.0.join(format!("started-{program}"))
    }

    fn cases(&self) -> PathBuf {
        self.repo().join(".takt/cases")
    }

    /// Writes `text` as `config/<name>` and returns its path.
    fn file(&self, name: &str, text: &str) -> PathBuf {
        let path = self.config_dir().join(name);
        fs::write(&path, text).expect("a file beside the run file");
        path
    }

    /// `takt run` on the repository with an intent and `--config <config>`, `flags` after them,
    /// HOME the scratch one and the doubles first on the program path.
    fn run(&self, config: &Path, flags: &[&str]) -> Output {
        let repo = self.repo();
        let mut args = vec![
            "run",
            "--repo",
            repo.to_str().expect("utf-8"),
            "--intent",
            "probe",
            "--config",
            config.to_str().expect("utf-8"),
        ];
        args.extend_from_slice(flags);
        self.takt(&args)
    }

    /// `takt scenario run` of scenario `1` with `--config <config>`, a passing test command, no
    /// forge (nothing fetched or pushed) and four steps, enough to reach the first session step;
    /// `flags` after them.
    fn scenario_run(&self, config: &Path, flags: &[&str]) -> Output {
        let repo = self.repo();
        let mut args = vec![
            "scenario",
            "run",
            "--repo",
            repo.to_str().expect("utf-8"),
            "1",
            "--config",
            config.to_str().expect("utf-8"),
            "--test-cmd",
            "true",
            "--forge",
            "none",
            "--iterations",
            "4",
        ];
        args.extend_from_slice(flags);
        self.takt(&args)
    }

    /// `takt` with `args`, HOME the scratch one and the doubles first on the program path.
    fn takt(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_takt"))
            .args(args)
            .env("HOME", self.home())
            .env(
                "PATH",
                format!(
                    "{}:{}",
                    self.bin().display(),
                    std::env::var("PATH").unwrap_or_default()
                ),
            )
            .env("XDG_STATE_HOME", self.0.join("xdg-state"))
            .env_remove("DBUS_SESSION_BUS_ADDRESS")
            .env_remove("XDG_RUNTIME_DIR")
            .env_remove("ANTHROPIC_API_KEY")
            .output()
            .expect("takt runs")
    }

    /// The argv `program`'s double was started with, one argument a line, when it was started.
    fn argv(&self, program: &str) -> Option<Vec<String>> {
        fs::read_to_string(self.started(program))
            .ok()
            .map(|text| text.lines().map(str::to_owned).collect())
    }

    /// Asserts the refusal of a run: exit 1, no case opened, no double started. Returns what the
    /// run said.
    fn assert_refused(&self, output: &Output) -> String {
        let said = text(output);
        assert_eq!(output.status.code(), Some(1), "{said}");
        assert!(!self.cases().exists(), "no case was opened: {said}");
        assert!(
            self.argv("claude").is_none() && self.argv("codex").is_none(),
            "no harness was started: {said}"
        );
        said
    }
}

/// `--max-turns` under claude is the session's ceiling: the claude double is started with
/// `--max-turns 60` and not with the default 40.
#[test]
fn the_max_turns_flag_is_the_claude_ceiling() {
    let world = World::new("max-turns-flag");
    let config = world.file("run.yaml", &run_file(""));
    let output = world.run(&config, &["--max-turns", "60"]);
    let argv = world
        .argv("claude")
        .unwrap_or_else(|| panic!("claude was not started: {}", text(&output)));
    assert_eq!(
        value_after(&argv, "--max-turns").as_deref(),
        Some("60"),
        "{argv:?}"
    );
    assert!(!argv.iter().any(|arg| arg == "40"), "{argv:?}");
}

/// The flag wins over the same key in the run file: `--max-turns 50` over a file's 60 starts the
/// claude double with 50.
#[test]
fn a_flag_wins_over_the_files_max_turns() {
    let world = World::new("flag-over-key");
    let config = world.file(
        "run.yaml",
        &run_file("harness: {kind: claude, max-turns: 60}\n"),
    );
    let output = world.run(&config, &["--max-turns", "50"]);
    let argv = world
        .argv("claude")
        .unwrap_or_else(|| panic!("claude was not started: {}", text(&output)));
    assert_eq!(
        value_after(&argv, "--max-turns").as_deref(),
        Some("50"),
        "{argv:?}"
    );
    assert!(!argv.iter().any(|arg| arg == "60"), "{argv:?}");
}

/// Codex's launch takes no turn ceiling: `--max-turns` with `--harness codex` is refused, naming
/// the flag's key, before any case opens and with no double started.
#[test]
fn max_turns_under_codex_is_refused_before_any_case_opens() {
    let world = World::new("kind-check");
    let config = world.file("run.yaml", &run_file(""));
    let output = world.run(&config, &["--harness", "codex", "--max-turns", "60"]);
    let said = world.assert_refused(&output);
    assert!(said.contains("the command line's --max-turns"), "{said}");
    assert!(said.contains("codex"), "{said}");
    assert!(!said.contains("in its `harness` mapping"), "{said}");
    assert!(
        said.contains(
            "the run asked for --max-turns and the pinned codex exec has no way to express it"
        ),
        "metaharness's own refusal: {said}"
    );
}

/// A session setting given under `--harness none` would start no session and be dropped, so it is
/// refused before any case opens, naming the setting.
#[test]
fn a_session_flag_under_harness_none_is_refused_before_any_case_opens() {
    let world = World::new("none-session-flag");
    let config = world.file("run.yaml", &run_file(""));
    let output = world.run(&config, &["--harness", "none", "--max-turns", "60"]);
    let said = world.assert_refused(&output);
    assert!(said.contains("max-turns"), "{said}");
    assert!(said.contains("--harness none"), "{said}");
}

/// The same refusal when the file's kind is codex and the flag gives the turns.
#[test]
fn max_turns_flag_over_a_codex_file_is_refused_before_any_case_opens() {
    let world = World::new("kind-check-file");
    let config = world.file("run.yaml", &run_file("harness: {kind: codex}\n"));
    let output = world.run(&config, &["--max-turns", "60"]);
    let said = world.assert_refused(&output);
    assert!(said.contains("the command line's --max-turns"), "{said}");
    assert!(!said.contains("in its `harness` mapping"), "{said}");
}

/// Each of the four flags once refused as not carried by takt's launch is carried now: the run is
/// not refused for it, and a case opens.
#[test]
fn a_flag_once_not_carried_is_carried_and_a_case_opens() {
    let cases: [(&str, &[&str]); 4] = [
        ("effort", &["--effort", "high"]),
        ("max-budget-usd", &["--max-budget-usd", "2"]),
        ("strict-version", &["--strict-version"]),
        ("context", &["--context", "README.md"]),
    ];
    for (key, flags) in cases {
        let world = World::new(&format!("carried-{key}"));
        let config = world.file("run.yaml", &run_file(""));
        let output = world.run(&config, flags);
        let said = text(&output);
        assert!(!said.contains("does not carry"), "{key}: {said}");
        assert!(world.cases().exists(), "{key}: a case was opened: {said}");
    }
}

/// `takt scenario run` takes the same flags through the same check, so a flag metaharness refuses
/// for the harness is refused before the scenario is read.
#[test]
fn scenario_run_refuses_a_flag_metaharness_refuses_before_anything_opens() {
    let world = World::new("scenario-kind-check");
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args(["scenario", "run", "--repo"])
        .arg(world.repo())
        .args(["1", "--harness", "codex", "--max-budget-usd", "2"])
        .env("XDG_STATE_HOME", world.0.join("xdg-state"))
        .output()
        .expect("takt runs");
    let said = world.assert_refused(&output);
    assert!(
        said.contains("the command line's --max-budget-usd"),
        "{said}"
    );
    assert!(
        said.contains("the run asked for --max-budget-usd and the pinned codex exec"),
        "metaharness's own refusal: {said}"
    );
}

/// `takt run --help` and `takt scenario run --help` list all five flags by name.
#[test]
fn the_help_of_both_run_commands_lists_the_five_flags() {
    for args in [&["run", "--help"][..], &["scenario", "run", "--help"][..]] {
        let output = Command::new(env!("CARGO_BIN_EXE_takt"))
            .args(args)
            .output()
            .expect("takt runs");
        let help = text(&output);
        assert!(output.status.success(), "{args:?}: {help}");
        for flag in FLAGS {
            assert!(help.contains(flag), "{args:?} does not list {flag}: {help}");
        }
    }
}

/// The two commands the acceptance is observed on.
#[derive(Clone, Copy, Debug)]
enum On {
    Run,
    ScenarioRun,
}

/// What one run left behind: what it said with its case records, and each double's record.
struct Seen {
    said: String,
    claude: Option<Vec<String>>,
    codex: Option<Vec<String>>,
    codex_config: Option<String>,
}

/// Runs `on` in a world of its own: `harness` written as the run file's `harness` member,
/// `flags` on the command line. A scenario run's file declares `scan: none`, which a session that
/// may read outside its copy needs; `takt run`'s file ([`run_file`]) declares it already.
fn seen(on: On, name: &str, harness: &str, flags: &[&str]) -> Seen {
    let name = format!("{on:?}-{name}");
    let world = match on {
        On::Run => World::new(&name),
        On::ScenarioRun => World::scenario(&name),
    };
    let output = match on {
        On::Run => world.run(&world.file("run.yaml", &run_file(harness)), flags),
        On::ScenarioRun => world.scenario_run(
            &world.file("run.yaml", &format!("scan: none\n{harness}")),
            flags,
        ),
    };
    let mut said = text(&output);
    for case in fs::read_dir(world.cases()).into_iter().flatten().flatten() {
        for record in fs::read_dir(case.path()).into_iter().flatten().flatten() {
            if let Ok(record) = fs::read_to_string(record.path()) {
                said.push('\n');
                said.push_str(&record);
            }
        }
    }
    Seen {
        said,
        claude: world.argv("claude"),
        codex: world.argv("codex"),
        codex_config: fs::read_to_string(world.config_copy("codex")).ok(),
    }
}

/// One flag of harness-flags/each-flag-observed on one command: three runs under `--harness
/// <kind>` or the file's `kind`, one with `flag` on the command line, one whose run file's
/// `harness` mapping sets `key` alone, one with neither. `observe` reads the same observable from
/// each: the first two give `expected`, the third does not.
fn each_flag_observed<T: PartialEq + std::fmt::Debug>(
    on: On,
    kind: &str,
    flag: &[&str],
    key: &str,
    expected: &T,
    observe: impl Fn(&Seen) -> T,
) {
    let tag = flag[0].trim_start_matches('-');
    let by_flag = seen(
        on,
        &format!("{tag}-flag"),
        "",
        &[&["--harness", kind][..], flag].concat(),
    );
    let by_key = seen(
        on,
        &format!("{tag}-key"),
        &format!("harness: {{kind: {kind}, {key}}}\n"),
        &[],
    );
    let neither = seen(on, &format!("{tag}-none"), "", &["--harness", kind]);
    assert_eq!(
        &observe(&by_flag),
        expected,
        "{on:?} with {flag:?}: {}",
        by_flag.said
    );
    assert_eq!(
        &observe(&by_key),
        expected,
        "{on:?} with the file's {key}: {}",
        by_key.said
    );
    assert_ne!(
        &observe(&neither),
        expected,
        "{on:?} without {flag:?}: {}",
        neither.said
    );
}

/// The `model_reasoning_effort` line of the Codex configuration the codex double was given.
fn codex_effort(seen: &Seen) -> Option<String> {
    seen.codex_config.as_deref().and_then(|config| {
        config
            .lines()
            .map(str::trim)
            .find(|line| line.starts_with("model_reasoning_effort"))
            .map(str::to_owned)
    })
}

/// The value after `flag` in the argv the claude double was started with.
fn claude_value(seen: &Seen, flag: &str) -> Option<String> {
    seen.claude
        .as_deref()
        .and_then(|argv| value_after(argv, flag))
}

/// Whether the claude double was started with [`NOTE`]'s text somewhere in its argv: the prompt
/// metaharness hands it.
fn claude_was_handed_the_note(seen: &Seen) -> bool {
    seen.claude
        .as_deref()
        .is_some_and(|argv| argv.iter().any(|arg| arg.contains(NOTE.trim_end())))
}

/// metaharness's refusal of `--strict-version` for a binary off the pin: its H9 verdict on the
/// codex double, whose [`DOUBLE_VERSION`] is outside metaharness 0.23.0's Codex pins. takt asks
/// metaharness to qualify the binary before the first session (`SessionHarness::preflight`),
/// and under `--strict-version` that verdict stops the run
/// `HarnessUnqualified` before the launch, so metaharness's launch-time text (`--strict-version
/// requires one of …`) is never reached. Codex, not Claude Code: Claude Code runs under the
/// strict hermetic mode, where a version off its pin has no memory canary and H11 stops the run
/// with or without the flag, so no run could show the flag's difference.
const STRICT_VERSION_REFUSAL: &str =
    "H9 gap: the vendor is 2.1.309 and the adapter is pinned to [\"0.153.4\", \"0.160.0\"]";

/// Whether the run started the codex session, and whether it stopped `HarnessUnqualified` on
/// metaharness's version verdict.
fn strict_version_seen(seen: &Seen) -> (bool, bool) {
    let refused = seen
        .said
        .lines()
        .find(|line| line.starts_with("stopped: HarnessUnqualified"))
        .is_some_and(|line| line.contains(STRICT_VERSION_REFUSAL));
    (seen.codex.is_some(), refused)
}

mod each_flag_observed {
    use super::*;

    fn effort(on: On) {
        each_flag_observed(
            on,
            "codex",
            &["--effort", "high"],
            "effort: high",
            &Some("model_reasoning_effort = \"high\"".to_owned()),
            codex_effort,
        );
    }

    fn max_turns(on: On) {
        each_flag_observed(
            on,
            "claude",
            &["--max-turns", "60"],
            "max-turns: 60",
            &Some("60".to_owned()),
            |seen| claude_value(seen, "--max-turns"),
        );
    }

    fn max_budget_usd(on: On) {
        each_flag_observed(
            on,
            "claude",
            &["--max-budget-usd", "2"],
            "max-budget-usd: 2",
            &Some("2".to_owned()),
            |seen| claude_value(seen, "--max-budget-usd"),
        );
    }

    fn strict_version(on: On) {
        each_flag_observed(
            on,
            "codex",
            &["--strict-version"],
            "strict-version: true",
            &(false, true),
            strict_version_seen,
        );
    }

    fn context(on: On) {
        each_flag_observed(
            on,
            "claude",
            &["--context", "notes.md"],
            "context: [notes.md]",
            &true,
            claude_was_handed_the_note,
        );
    }

    /// `--effort high` under codex: the Codex configuration metaharness generated holds
    /// `model_reasoning_effort = "high"`.
    #[test]
    fn effort_on_takt_run() {
        effort(On::Run);
    }

    #[test]
    fn effort_on_takt_scenario_run() {
        effort(On::ScenarioRun);
    }

    /// `--max-turns 60` under claude: the claude double is started with `--max-turns 60`.
    #[test]
    fn max_turns_on_takt_run() {
        max_turns(On::Run);
    }

    #[test]
    fn max_turns_on_takt_scenario_run() {
        max_turns(On::ScenarioRun);
    }

    /// `--max-budget-usd 2` under claude: the claude double is started with
    /// `--max-budget-usd 2`.
    #[test]
    fn max_budget_usd_on_takt_run() {
        max_budget_usd(On::Run);
    }

    #[test]
    fn max_budget_usd_on_takt_scenario_run() {
        max_budget_usd(On::ScenarioRun);
    }

    /// `--strict-version` under codex, the double off metaharness's Codex pin: the run stops with
    /// metaharness's version verdict and no session starts; without it the session starts.
    #[test]
    fn strict_version_on_takt_run() {
        strict_version(On::Run);
    }

    #[test]
    fn strict_version_on_takt_scenario_run() {
        strict_version(On::ScenarioRun);
    }

    /// `--context notes.md`: the note's text is in what the claude double was handed.
    #[test]
    fn context_on_takt_run() {
        context(On::Run);
    }

    #[test]
    fn context_on_takt_scenario_run() {
        context(On::ScenarioRun);
    }
}

/// harness-flags/kind-check-applies: `--harness codex --max-turns 60` exits 1 before any case
/// opens, on both commands, with metaharness's own refusal of the setting for codex; no trial
/// branch is cut for the scenario run.
#[test]
fn kind_check_applies() {
    for on in [On::Run, On::ScenarioRun] {
        let name = format!("{on:?}-kind-check-applies");
        let world = match on {
            On::Run => World::new(&name),
            On::ScenarioRun => World::scenario(&name),
        };
        let flags = ["--harness", "codex", "--max-turns", "60"];
        let output = match on {
            On::Run => world.run(&world.file("run.yaml", &run_file("")), &flags),
            On::ScenarioRun => world.scenario_run(&world.file("run.yaml", "scan: none\n"), &flags),
        };
        let said = world.assert_refused(&output);
        assert!(
            said.contains("the command line's --max-turns"),
            "{on:?}: {said}"
        );
        assert!(
            said.contains(
                "the run asked for --max-turns and the pinned codex exec has no way to express it"
            ),
            "{on:?}: metaharness's own refusal: {said}"
        );
        assert!(
            !git_out(&world.repo(), &["branch", "--format=%(refname:short)"]).contains("trial/"),
            "{on:?}: no trial branch was cut: {said}"
        );
    }
}
