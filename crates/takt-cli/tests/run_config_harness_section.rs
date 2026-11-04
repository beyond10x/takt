//! story:harness-section-is-metaharness-session-request: a run file's `harness` mapping is
//! metaharness's session request. Its admitted keys take effect; any other key is refused before
//! any case opens, naming the key and whose it is, and no harness is started. The harness programs
//! here are doubles on the program path that leave the argv they were started with behind. No run
//! reaches a network: each run stops at its one step, the double exiting before it does anything.

#[path = "../../takt/tests/support/own_home.rs"]
mod own_home;

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

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
    String::from_utf8_lossy(&output.stdout).into_owned()
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

/// A scratch tree under this crate's build directory, removed on drop: a repository with one
/// commit, an empty HOME whose `.local/bin` holds the codex double metaharness finds first, a program
/// directory holding the claude double, and a
/// config directory.
struct World(PathBuf);

impl Drop for World {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

impl World {
    fn new(name: &str) -> Self {
        let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("harness-section-{name}-{}", std::process::id()));
        if outer.exists() {
            fs::remove_dir_all(&outer).expect("a stale scratch tree is removed");
        }
        let world = Self(outer);
        for dir in [world.repo(), world.home(), world.bin(), world.config_dir()] {
            fs::create_dir_all(dir).expect("a directory");
        }
        fs::write(world.repo().join("README.md"), "a story\n").expect("a file");
        git(&world.repo(), &["init", "-q", "-b", "main"]);
        git(&world.repo(), &["add", "-A"]);
        git(&world.repo(), &["commit", "-q", "-m", "start"]);
        for program in ["claude", "codex"] {
            let dir = if program == "codex" {
                world.home().join(".local/bin")
            } else {
                world.bin()
            };
            fs::create_dir_all(&dir).expect("a directory");
            let double = dir.join(program);
            // The Codex configuration metaharness generated for the session, copied beside the
            // argv file when the child is given a CODEX_HOME.
            fs::write(
                &double,
                format!(
                    "#!/bin/sh\ncase $1 in --version) echo '2.1.309 (Claude Code)'; exit 0 ;; esac\nif [ -n \"$CODEX_HOME\" ] && [ -f \"$CODEX_HOME/config.toml\" ]; then cp \"$CODEX_HOME/config.toml\" '{}'; fi\nfor arg in \"$@\"; do printf '%s\\n' \"$arg\"; done > '{}'\nexit 1\n",
                    world.config_copy(program).display(),
                    world.started(program).display()
                ),
            )
            .expect("a double");
            fs::set_permissions(&double, fs::Permissions::from_mode(0o755)).expect("exec bit");
        }
        world
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

    /// Where `program`'s double copies the `config.toml` of the CODEX_HOME it was given.
    fn config_copy(&self, program: &str) -> PathBuf {
        self.0.join(format!("started-{program}-config.toml"))
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

    /// Clears the cases and the record of each double, so the next run is this run's alone.
    /// Clears the cases, the record of each double and the case branches, so the next run is this
    /// run's alone, on `main`.
    fn fresh(&self) {
        if self.cases().exists() {
            fs::remove_dir_all(self.cases()).expect("an earlier case is removed");
        }
        for program in ["claude", "codex"] {
            let _ = fs::remove_file(self.started(program));
            let _ = fs::remove_file(self.config_copy(program));
        }
        git(&self.repo(), &["worktree", "prune"]);
        git(&self.repo(), &["checkout", "-q", "-f", "main"]);
        let branches = git_out(&self.repo(), &["branch", "--format=%(refname:short)"]);
        for branch in branches
            .lines()
            .filter(|branch| branch.starts_with("takt/"))
        {
            git(&self.repo(), &["branch", "-D", branch]);
        }
    }

    /// `takt run` on the repository with an intent and `--config <config>`, `flags` after them,
    /// HOME the scratch one and the doubles first on the program path.
    fn run(&self, config: &Path, flags: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_takt"))
            .args([
                "run",
                "--repo",
                self.repo().to_str().expect("utf-8"),
                "--intent",
                "probe",
                "--config",
                config.to_str().expect("utf-8"),
            ])
            .args(flags)
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

    /// Runs `harness` as a run file's `harness` member and asserts the refusal: exit 1, no case
    /// opened, no double started. Returns what the run said.
    fn refused(&self, harness: &str) -> String {
        self.fresh();
        let config = self.file("run.yaml", &run_file(harness));
        let output = self.run(&config, &[]);
        let said = text(&output);
        assert_eq!(output.status.code(), Some(1), "{said}");
        assert!(!self.cases().exists(), "no case was opened: {said}");
        assert!(
            self.argv("claude").is_none() && self.argv("codex").is_none(),
            "no harness was started: {said}"
        );
        said
    }
}

/// A key outside the admitted six is refused before any case opens, named as not admitted, and
/// its value is never printed.
#[test]
fn an_unadmitted_key_is_refused_naming_the_key() {
    let world = World::new("unadmitted");
    let said = world.refused("harness: {kind: claude, retain-dir: MARKER-retain-value}\n");
    assert!(said.contains("`retain-dir`"), "{said}");
    assert!(said.contains("not admitted"), "{said}");
    assert!(!said.contains("MARKER-retain-value"), "{said}");
}

/// `hermetic` is takt's confinement, refused as takt's before any case opens.
#[test]
fn a_takt_owned_key_is_refused_as_takts() {
    let world = World::new("takt-owned");
    let said = world.refused("harness: {kind: claude, hermetic: off}\n");
    assert!(said.contains("`hermetic`"), "{said}");
    assert!(said.contains("takt's"), "{said}");
}

/// An endpoint key belongs to the run's model and llm section, not to the harness mapping.
#[test]
fn an_endpoint_key_is_refused_as_the_runs() {
    let world = World::new("endpoint");
    let said = world.refused("harness: {kind: claude, api-key-secret: MARKER-secret-name}\n");
    assert!(said.contains("`api-key-secret`"), "{said}");
    assert!(said.contains("llm section"), "{said}");
    assert!(!said.contains("MARKER-secret-name"), "{said}");
}

/// `kind` is one of metaharness's three names and is given; a value that is none of them is
/// refused naming the key, never the value.
#[test]
fn kind_is_given_and_is_one_of_three_names() {
    let world = World::new("kind");
    let said = world.refused("harness: {max-turns: 3}\n");
    assert!(said.contains("`kind`"), "{said}");
    let said = world.refused("harness: {kind: MARKER-kind-value}\n");
    assert!(said.contains("`kind`"), "{said}");
    assert!(!said.contains("MARKER-kind-value"), "{said}");
}

/// `max-turns` is a whole number takt can pass to the session; any other value is refused naming
/// the key, never the value.
#[test]
fn max_turns_is_a_whole_number() {
    let world = World::new("max-turns-type");
    let said = world.refused("harness: {kind: claude, max-turns: MARKER-turns-value}\n");
    assert!(said.contains("`max-turns`"), "{said}");
    assert!(!said.contains("MARKER-turns-value"), "{said}");
}

/// harness-section/codex-effort-passed: `effort` reaches the Codex session. The configuration
/// metaharness generated for the child, which the double copies beside its argv file, holds
/// `model_reasoning_effort = "high"`.
#[test]
fn codex_effort_passed() {
    let world = World::new("codex-effort");
    world.fresh();
    let config = world.file(
        "run.yaml",
        &run_file("harness: {kind: codex, effort: high}\n"),
    );
    let output = world.run(&config, &[]);
    let said = text(&output);
    assert!(
        world.argv("codex").is_some(),
        "codex was not started: {said}"
    );
    let generated = fs::read_to_string(world.config_copy("codex"))
        .unwrap_or_else(|error| panic!("no Codex configuration was copied ({error}): {said}"));
    assert!(
        generated
            .lines()
            .any(|line| line.trim() == "model_reasoning_effort = \"high\""),
        "{generated}"
    );
}

/// harness-section/bad-value-refused-by-metaharness: a value of the wrong type is refused before
/// any case opens with metaharness's own message, read from its session-request document, and no
/// harness is started.
#[test]
fn bad_value_refused_by_metaharness() {
    let world = World::new("bad-value");
    let said = world.refused("harness: {kind: claude, max-turns: sixty}\n");
    assert!(
        said.contains("the run-spec document's `max-turns` takes a whole number"),
        "{said}"
    );
    assert!(!said.contains("sixty"), "{said}");
    let said = world.refused("harness: {kind: claude, strict-version: MARKER-strict}\n");
    assert!(
        said.contains("the run-spec document's `strict-version` takes true or false"),
        "{said}"
    );
    assert!(!said.contains("MARKER-strict"), "{said}");
}

/// harness-section/kind-check-refused: `max-turns` under `kind: codex` is refused before any case
/// opens with metaharness's own refusal of the setting for that kind, and no harness is started.
#[test]
fn kind_check_refused() {
    let world = World::new("kind-check");
    let said = world.refused("harness: {kind: codex, max-turns: 60}\n");
    assert!(said.contains("`kind: codex`"), "{said}");
    assert!(
        said.contains(
            "the run asked for --max-turns and the pinned codex exec has no way to express it"
        ),
        "{said}"
    );
    let said = world.refused("harness: {kind: codex, max-budget-usd: 2}\n");
    assert!(
        said.contains("the run asked for --max-budget-usd and the pinned codex exec"),
        "{said}"
    );
}

/// `max-budget-usd` reaches the Claude Code session as metaharness passes it: read as a number or
/// a decimal string, carried in micro-dollars, and given to the program as its decimal of dollars.
#[test]
fn max_budget_usd_reaches_the_claude_session() {
    let world = World::new("budget");
    for (written, passed) in [("1.5", "1.5"), ("\"2.50\"", "2.5"), ("3", "3")] {
        world.fresh();
        let config = world.file(
            "run.yaml",
            &run_file(&format!(
                "harness: {{kind: claude, max-budget-usd: {written}}}\n"
            )),
        );
        let output = world.run(&config, &[]);
        let argv = world
            .argv("claude")
            .unwrap_or_else(|| panic!("claude was not started: {}", text(&output)));
        let at = argv
            .iter()
            .position(|arg| arg == "--max-budget-usd")
            .unwrap_or_else(|| panic!("no --max-budget-usd in {argv:?}"));
        assert_eq!(
            argv.get(at + 1).map(String::as_str),
            Some(passed),
            "{written}: {argv:?}"
        );
    }
}

/// A `max-budget-usd` finer than one micro-dollar, or no sum of dollars at all, is refused before
/// any case opens naming the key, and its value is never printed.
#[test]
fn a_budget_finer_than_a_micro_dollar_is_refused_naming_the_key() {
    let world = World::new("budget-fine");
    for written in ["0.0000001", "\"1.0000005\"", "\"MARKER-budget\"", "-1"] {
        let said = world.refused(&format!(
            "harness: {{kind: claude, max-budget-usd: {written}}}\n"
        ));
        assert!(said.contains("`max-budget-usd`"), "{written}: {said}");
        assert!(said.contains("micro-dollar"), "{written}: {said}");
        assert!(!said.contains("MARKER-budget"), "{said}");
    }
}

/// `context` names files of the repository the session's copy holds: such a file is handed to the
/// Claude Code session as metaharness renders a context file, and any other path is refused
/// before any case opens, naming it.
#[test]
fn context_names_a_file_the_copy_holds() {
    let world = World::new("context");
    world.fresh();
    let config = world.file(
        "run.yaml",
        &run_file("harness: {kind: claude, context: [README.md]}\n"),
    );
    let output = world.run(&config, &[]);
    let argv = world
        .argv("claude")
        .unwrap_or_else(|| panic!("claude was not started: {}", text(&output)));
    assert!(
        argv.iter()
            .any(|arg| arg.starts_with("<context-file-") && arg.contains("path=\"README.md\"")),
        "{argv:?}"
    );
    for (path, named) in [
        ("missing.md", "missing.md"),
        ("/etc/hostname", "/etc/hostname"),
        ("../outside.md", "../outside.md"),
        ("./README.md", "./README.md"),
    ] {
        let said = world.refused(&format!(
            "harness: {{kind: claude, context: [\"{path}\"]}}\n"
        ));
        assert!(said.contains(named), "{path}: {said}");
    }
}

/// `strict-version` is carried: the run is not refused for it, and the session's version check
/// is the one that decides.
#[test]
fn strict_version_is_carried_not_refused() {
    let world = World::new("strict");
    world.fresh();
    let config = world.file(
        "run.yaml",
        &run_file("harness: {kind: claude, strict-version: true}\n"),
    );
    let output = world.run(&config, &[]);
    let said = text(&output);
    assert!(world.cases().exists(), "a case was opened: {said}");
    assert!(!said.contains("does not carry"), "{said}");
}

/// The string form keeps its meaning: `harness: codex` starts the argv `--harness codex` starts.
/// A session setting in the file's `harness` mapping under `--harness none`, which starts no session,
/// is refused before any case opens, naming the setting, as a flag's is: it would be dropped.
#[test]
fn a_file_session_setting_under_harness_none_is_refused_before_any_case_opens() {
    let world = World::new("none-file-setting");
    world.fresh();
    let config = world.file(
        "run.yaml",
        &run_file("harness: {kind: claude, max-turns: 60}\n"),
    );
    let output = world.run(&config, &["--harness", "none"]);
    let said = text(&output);
    assert_eq!(output.status.code(), Some(1), "{said}");
    assert!(!world.cases().exists(), "no case was opened: {said}");
    assert!(said.contains("max-turns"), "{said}");
    assert!(said.contains("--harness none"), "{said}");
}

/// `max-turns` is a whole number above zero: zero ends no session turn, so it is refused before any
/// case opens, in the file's mapping and on the command line alike.
#[test]
fn max_turns_zero_is_refused_before_any_case_opens() {
    let world = World::new("max-turns-zero");
    let said = world.refused("harness: {kind: claude, max-turns: 0}\n");
    assert!(said.contains("`max-turns`"), "{said}");
    world.fresh();
    let config = world.file("run.yaml", &run_file(""));
    let output = world.run(&config, &["--max-turns", "0"]);
    let said = text(&output);
    assert_eq!(output.status.code(), Some(1), "{said}");
    assert!(!world.cases().exists(), "no case was opened: {said}");
    assert!(said.contains("the command line's --max-turns"), "{said}");
    assert!(
        world.argv("claude").is_none() && world.argv("codex").is_none(),
        "no harness was started: {said}"
    );
}

#[test]
fn the_string_form_starts_the_argv_of_the_flag() {
    let world = World::new("string-form");
    let config = world.file("run.yaml", &run_file("harness: codex\n"));
    let by_flag = world.run(&config, &["--harness", "codex"]);
    let flag_argv = world
        .argv("codex")
        .unwrap_or_else(|| panic!("codex was not started by the flag: {}", text(&by_flag)));
    world.fresh();
    let by_file = world.run(&config, &[]);
    let file_argv = world
        .argv("codex")
        .unwrap_or_else(|| panic!("codex was not started by the file: {}", text(&by_file)));
    assert!(!flag_argv.is_empty(), "{}", text(&by_flag));
    assert_eq!(by_flag.status.code(), by_file.status.code());
    assert_eq!(flag_argv, file_argv);
}

/// `max-turns` replaces the session's default of 40: the claude session is started with
/// `--max-turns 60` and not with 40.
#[test]
fn max_turns_replaces_the_default() {
    let world = World::new("max-turns");
    let config = world.file(
        "run.yaml",
        &run_file("harness: {kind: claude, max-turns: 60}\n"),
    );
    let output = world.run(&config, &[]);
    let argv = world
        .argv("claude")
        .unwrap_or_else(|| panic!("claude was not started: {}", text(&output)));
    let at = argv
        .iter()
        .position(|arg| arg == "--max-turns")
        .unwrap_or_else(|| panic!("no --max-turns in {argv:?}"));
    assert_eq!(argv.get(at + 1).map(String::as_str), Some("60"), "{argv:?}");
    assert!(!argv.iter().any(|arg| arg == "40"), "{argv:?}");
}

/// `--harness` on the command line wins over the mapping's kind.
#[test]
fn the_command_line_harness_wins_over_the_kind() {
    let world = World::new("flag-wins");
    let config = world.file("run.yaml", &run_file("harness: {kind: codex}\n"));
    let output = world.run(&config, &["--harness", "claude"]);
    assert!(
        world.argv("claude").is_some(),
        "claude was started: {}",
        text(&output)
    );
    assert!(world.argv("codex").is_none(), "codex was not started");
}
