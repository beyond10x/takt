//! story:harness-flags-spell-metaharness-names: `takt run` and `takt scenario run` take the five
//! session settings of the harness mapping as flags under the names `metaharness run` takes; a
//! flag wins over the same key in the run file; a setting takt's launch does not carry yet, and
//! `max-turns` under codex, is refused before any case opens. The harness programs here are
//! doubles on the program path that leave the argv they were started with behind. No run reaches
//! a network: each run stops at its one step, the double exiting before it does anything.

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

/// A scratch tree under this crate's build directory, removed on drop: a repository with one
/// commit, an empty HOME whose `.local/bin` holds the codex double, a program directory holding
/// the claude double, and a config directory.
struct World(PathBuf);

impl Drop for World {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

impl World {
    fn new(name: &str) -> Self {
        let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("harness-flags-{name}-{}", std::process::id()));
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
            fs::write(
                &double,
                format!(
                    "#!/bin/sh\nfor arg in \"$@\"; do printf '%s\\n' \"$arg\"; done > '{}'\nexit 1\n",
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
            .env_remove("B10X_GATES_POLICY")
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
    assert!(said.contains("`max-turns`"), "{said}");
    assert!(said.contains("`kind: codex`"), "{said}");
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
    assert!(said.contains("`max-turns`"), "{said}");
}

/// Each of the four flags takt's launch does not carry yet is refused as the file's key is, by
/// name, before any case opens.
#[test]
fn a_flag_not_carried_yet_is_refused_before_any_case_opens() {
    let cases: [(&str, &[&str]); 4] = [
        ("effort", &["--effort", "high"]),
        ("max-budget-usd", &["--max-budget-usd", "2"]),
        ("strict-version", &["--strict-version"]),
        ("context", &["--context", "notes.md"]),
    ];
    for (key, flags) in cases {
        let world = World::new(&format!("not-carried-{key}"));
        let config = world.file("run.yaml", &run_file(""));
        let output = world.run(&config, flags);
        let said = world.assert_refused(&output);
        assert!(said.contains(&format!("`{key}`")), "{said}");
        assert!(
            said.contains("refused rather than run without it"),
            "{said}"
        );
    }
}

/// `takt scenario run` takes the same flags, so the refusal of one is the same before the
/// scenario is read.
#[test]
fn scenario_run_refuses_a_flag_not_carried_yet_before_anything_opens() {
    let world = World::new("scenario-not-carried");
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args(["scenario", "run", "--repo"])
        .arg(world.repo())
        .args(["1", "--max-budget-usd", "2"])
        .env("XDG_STATE_HOME", world.0.join("xdg-state"))
        .env_remove("B10X_GATES_POLICY")
        .output()
        .expect("takt runs");
    let said = world.assert_refused(&output);
    assert!(said.contains("`max-budget-usd`"), "{said}");
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
