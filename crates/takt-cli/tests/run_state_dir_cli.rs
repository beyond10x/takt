//! story:run-state-directory-is-configurable: the directory takt keeps a run's state in,
//! `$XDG_STATE_HOME/takt` by default, is named by the run configuration's `state_dir` or by
//! `--state-dir`, so a container whose `HOME` is not kept puts the session transcripts on a
//! volume that is. Each harness step's transcript is then kept under `<state_dir>/transcripts`,
//! the step's line naming it relative to that directory, and nothing under `XDG_STATE_HOME`.
//! The `claude` is a double; nothing reaches a model.

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

fn executable(path: &Path, text: &str) {
    fs::write(path, text).expect("a script");
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("exec bit");
}

/// A scratch world under this crate's build directory, removed on drop: a repository with one
/// commit, a program directory holding a `claude` double that writes a file and answers
/// `tests.write`, a `config` directory and an `XDG_STATE_HOME` of its own.
struct World(PathBuf);

impl Drop for World {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

impl World {
    fn new(name: &str) -> Self {
        let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("run-state-dir-{name}-{}", std::process::id()));
        if outer.exists() {
            fs::remove_dir_all(&outer).expect("a stale scratch tree is removed");
        }
        let world = Self(outer);
        for dir in [world.repo(), world.bin(), world.config_dir(), world.xdg()] {
            fs::create_dir_all(dir).expect("a directory");
        }
        fs::write(world.repo().join("README.md"), "a story\n").expect("a file");
        git(&world.repo(), &["init", "-q", "-b", "main"]);
        git(&world.repo(), &["add", "-A"]);
        git(&world.repo(), &["commit", "-q", "-m", "start"]);
        executable(
            &world.bin().join("claude"),
            "#!/bin/sh\nmkdir -p tests\necho work >> tests/work.txt\nprintf '%s\\n' '{\"type\":\"result\",\"subtype\":\"success\",\"result\":\"{\\\"action\\\": \\\"tests.write\\\", \\\"summary\\\": \\\"wrote a test\\\"}\",\"usage\":{\"input_tokens\":3,\"output_tokens\":4}}'\n",
        );
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

    fn xdg(&self) -> PathBuf {
        self.0.join("xdg-state")
    }

    /// Writes `text` as `config/run.toml` and returns its path.
    fn config(&self, text: &str) -> PathBuf {
        let path = self.config_dir().join("run.toml");
        fs::write(&path, text).expect("a run configuration file");
        path
    }

    /// `takt run --harness claude` on the repository with an intent, two steps, a passing test
    /// command and `args`, the double first on the program path.
    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_takt"))
            .args([
                "run",
                "--repo",
                self.repo().to_str().expect("utf-8"),
                "--intent",
                "probe",
                "--harness",
                "claude",
                "--forge",
                "none",
                "--iterations",
                "2",
                "--test-cmd",
                "true",
            ])
            .args(args)
            .env(
                "PATH",
                format!(
                    "{}:{}",
                    self.bin().display(),
                    std::env::var("PATH").unwrap_or_default()
                ),
            )
            .env("XDG_STATE_HOME", self.xdg())
            .env_remove("B10X_GATES_POLICY")
            .env_remove("DBUS_SESSION_BUS_ADDRESS")
            .output()
            .expect("takt runs")
    }

    /// The transcripts the case's run record names, relative to the transcripts directory.
    fn named(&self) -> Vec<String> {
        let cases: Vec<PathBuf> = fs::read_dir(self.repo().join(".takt/cases"))
            .expect("the cases directory")
            .map(|entry| entry.expect("an entry").path())
            .collect();
        assert_eq!(cases.len(), 1, "{cases:?}");
        fs::read_to_string(cases[0].join("events.jsonl"))
            .expect("the run record")
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("a JSON line"))
            .filter_map(|line| line["transcript"].as_str().map(str::to_owned))
            .collect()
    }
}

/// Every regular file under `dir`.
fn files_under(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return found;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            found.extend(files_under(&path));
        } else {
            found.push(path);
        }
    }
    found
}

fn text(output: &Output) -> String {
    format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// Asserts the run kept a transcript under `state/transcripts`, that each the record names
/// lies there, and that nothing was kept under `XDG_STATE_HOME`.
fn kept_under(world: &World, state: &Path, said: &str) {
    let transcripts = state.join("transcripts");
    let kept = files_under(&transcripts);
    assert!(
        !kept.is_empty(),
        "a transcript under {}\n{said}",
        transcripts.display()
    );
    let named = world.named();
    assert!(!named.is_empty(), "the record names a transcript\n{said}");
    for name in &named {
        assert!(
            transcripts.join(name).is_file(),
            "{name} lies under {}\n{said}",
            transcripts.display()
        );
    }
    assert_eq!(
        files_under(&world.xdg()),
        Vec::<PathBuf>::new(),
        "nothing kept under XDG_STATE_HOME"
    );
}

#[test]
fn run_help_names_the_state_dir_flag() {
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args(["run", "--help"])
        .output()
        .expect("takt runs");
    assert_eq!(output.status.code(), Some(0), "{}", text(&output));
    assert!(
        text(&output).contains("--state-dir <DIR>"),
        "{}",
        text(&output)
    );
}

/// Acceptance 1, by the configuration: a relative `state_dir`, read beside the file, holds the
/// transcripts, and the case's record names them there.
#[test]
fn the_configurations_state_dir_holds_the_transcripts() {
    let world = World::new("config");
    let config = world.config("state_dir = \"state\"\n");
    let output = world.run(&["--config", config.to_str().expect("utf-8")]);
    let said = text(&output);
    kept_under(&world, &world.config_dir().join("state"), &said);
}

/// Acceptance 1, by the flag, which wins over the file's.
#[test]
fn the_state_dir_flag_wins_over_the_configurations() {
    let world = World::new("flag");
    let config = world.config("state_dir = \"state\"\n");
    let flag = world.0.join("volume");
    let output = world.run(&[
        "--config",
        config.to_str().expect("utf-8"),
        "--state-dir",
        flag.to_str().expect("utf-8"),
    ]);
    let said = text(&output);
    kept_under(&world, &flag, &said);
    assert!(
        !world.config_dir().join("state").exists(),
        "the file's state directory is not made\n{said}"
    );
}

/// A state directory named explicitly that cannot be used is refused before the case opens,
/// exit 1, naming it and why: inside the tree, a file on its way, and (unless this runs as a
/// user who writes anywhere) one that is not writable. No step runs and nothing is kept.
#[test]
fn an_unusable_named_state_dir_is_refused_before_the_case_opens() {
    use std::os::unix::fs::PermissionsExt as _;
    let world = World::new("unusable");
    let file = world.0.join("a-file");
    fs::write(&file, "x\n").expect("a file");
    let locked = world.0.join("locked");
    fs::create_dir_all(&locked).expect("a directory");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o500)).expect("read-only");
    let held = world.0.join("held");
    fs::create_dir_all(held.join("transcripts")).expect("a directory");
    fs::set_permissions(held.join("transcripts"), fs::Permissions::from_mode(0o500))
        .expect("read-only");
    let writes_anyway = fs::write(locked.join("probe"), "").is_ok();
    let mut cases = vec![
        (world.repo().join("state"), "inside the tree"),
        (file.join("state"), "is not a directory"),
    ];
    if !writes_anyway {
        cases.push((locked.clone(), "Permission denied"));
        cases.push((held.clone(), "is not writable"));
    }
    for (dir, why) in &cases {
        let config = world.config(&format!("state_dir = \"{}\"\n", dir.display()));
        for args in [
            vec!["--config", config.to_str().expect("utf-8")],
            vec!["--state-dir", dir.to_str().expect("utf-8")],
        ] {
            let output = world.run(&args);
            let said = text(&output);
            assert_eq!(output.status.code(), Some(1), "{args:?}\n{said}");
            // story:the-state-directory-holds-every-host-record: the refusal names the directory
            // as `takt::git::shown` writes it (control characters escaped, the home directory as `~`).
            assert!(
                said.contains(&format!(
                    "the state directory {}",
                    takt::git::shown(dir.as_os_str())
                )),
                "{args:?}\n{said}"
            );
            assert!(said.contains(why), "{why}: {args:?}\n{said}");
            assert!(
                !world.repo().join(".takt/cases").exists(),
                "no case was opened: {args:?}\n{said}"
            );
        }
    }
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o700)).expect("writable again");
    fs::set_permissions(held.join("transcripts"), fs::Permissions::from_mode(0o700))
        .expect("writable again");
}

/// Acceptance 2: without one, the transcripts are where they were,
/// `$XDG_STATE_HOME/takt/transcripts`.
#[test]
fn without_a_state_dir_the_transcripts_are_under_the_state_home() {
    let world = World::new("default");
    let output = world.run(&[]);
    let said = text(&output);
    let transcripts = world.xdg().join("takt/transcripts");
    let named = world.named();
    assert!(!named.is_empty(), "the record names a transcript\n{said}");
    for name in &named {
        assert!(
            transcripts.join(name).is_file(),
            "{name} lies under {}\n{said}",
            transcripts.display()
        );
    }
}
