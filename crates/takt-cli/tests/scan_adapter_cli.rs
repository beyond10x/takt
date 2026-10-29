//! story:scan-and-forge-are-adapters at the binary's edge: the scan a proposal passes is a named
//! adapter, `gates` unless the run configuration declares `scan: none`. A harness whose session
//! may read outside its copy (`codex`, `harness`) under `gates` with no policy is refused before
//! its case opens; a declared `none` opens it and says so; `none` with a policy set, and a value
//! that names no adapter, are refused. These runs reach no network and start no harness: HOME is
//! a scratch directory, every run stops before its first step, and a `codex` and a
//! `b10x-harness` double on the program path record whether they were started.

#[path = "../../takt/tests/support/own_home.rs"]
mod own_home;

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// The variable the `gates` scan adapter reads its policy from.
const POLICY_VAR: &str = takt::scan::gates::POLICY_VAR;

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

/// A scratch tree under this crate's build directory, removed on drop: a repository with one
/// commit, an empty HOME, a `config` directory and a program directory holding a `codex` and a
/// `b10x-harness` double, each leaving `started` behind when it runs.
struct World(PathBuf);

impl Drop for World {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

impl World {
    fn new(name: &str) -> Self {
        let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("scan-adapter-{name}-{}", std::process::id()));
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
        for program in ["codex", "b10x-harness", "claude"] {
            let double = world.bin().join(program);
            fs::write(
                &double,
                format!("#!/bin/sh\ncase $1 in --version) echo '2.1.309 (Claude Code)'; exit 0 ;; esac\ntouch '{}'\nexit 1\n", world.started().display()),
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

    fn started(&self) -> PathBuf {
        self.0.join("started")
    }

    fn cases(&self) -> PathBuf {
        self.repo().join(".takt/cases")
    }

    /// Writes `text` as `config/<name>` and returns its path.
    fn config(&self, name: &str, text: &str) -> PathBuf {
        let path = self.config_dir().join(name);
        fs::write(&path, text).expect("a run configuration file");
        path
    }

    /// `takt run` on the repository with an intent, no forge and `args`, HOME the scratch one,
    /// the doubles first on the program path, no session bus, and the gates policy `policy`
    /// when one is given, else none.
    fn run(&self, args: &[&str], policy: Option<&Path>) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_takt"));
        command
            .args([
                "run",
                "--repo",
                self.repo().to_str().expect("utf-8"),
                "--intent",
                "probe",
                "--forge",
                "none",
                "--test-cmd",
                "true",
            ])
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
            .env_remove("ANTHROPIC_API_KEY");
        match policy {
            Some(policy) => command.env(POLICY_VAR, policy),
            None => command.env_remove(POLICY_VAR),
        };
        command.output().expect("takt runs")
    }

    /// The `opened` lines of the one case record.
    fn opened(&self) -> Vec<serde_json::Value> {
        let events: Vec<PathBuf> = files_under(&self.cases())
            .into_iter()
            .filter(|path| path.file_name().is_some_and(|name| name == "events.jsonl"))
            .collect();
        assert_eq!(events.len(), 1, "one case record: {events:?}");
        fs::read_to_string(&events[0])
            .expect("the run record")
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("a JSON line"))
            .filter(|line| line["event"] == "opened")
            .collect()
    }
}

fn files_under(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return found;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(meta) = fs::symlink_metadata(&path) else {
            continue;
        };
        if meta.is_dir() {
            found.extend(files_under(&path));
        } else if meta.is_file() {
            found.push(path);
        }
    }
    found
}

fn text(output: &Output) -> (String, String) {
    (
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// The flags that point `--harness harness` at an endpoint by a secret's name alone, so the
/// harness is made without a token file and nothing is read under HOME.
const HARNESS_ENDPOINT: [&str; 6] = [
    "--endpoint-base-url",
    "https://models.example/v1",
    "--endpoint-wire",
    "openai-responses",
    "--endpoint-secret",
    "llm/model-key",
];

/// The `--harness` flags of each harness whose session may read outside its copy.
fn outside_readers() -> Vec<(&'static str, Vec<&'static str>)> {
    let mut harness = vec!["--harness", "harness"];
    harness.extend(HARNESS_ENDPOINT);
    vec![("codex", vec!["--harness", "codex"]), ("harness", harness)]
}

/// Acceptance 1: `--harness codex` and `--harness harness` with no gates policy and no
/// declaration are refused before the case opens, exit 1, the refusal naming the scan adapter
/// `gates`, the policy's variable and `scan: none` in `--config`; no case directory is written
/// and no harness is started. A Claude Code run, whose session reads only inside its copy,
/// opens as before.
#[test]
fn an_outside_reader_without_a_policy_or_a_declaration_is_refused_before_its_case_opens() {
    for (name, flags) in outside_readers() {
        let world = World::new(&format!("refused-{name}"));
        let mut args = flags.clone();
        args.extend(["--iterations", "1"]);
        let output = world.run(&args, None);
        let (stdout, stderr) = text(&output);
        assert_eq!(output.status.code(), Some(1), "{name}: {stdout}\n{stderr}");
        for named in ["`gates`", POLICY_VAR, "`scan: none`", "--config"] {
            assert!(
                stderr.contains(named),
                "{name}: the refusal names {named}: {stderr}"
            );
        }
        assert!(
            stderr.contains("nothing was opened"),
            "{name}: the refusal says nothing was opened: {stderr}"
        );
        assert!(!world.cases().exists(), "{name}: no case directory");
        assert!(!world.started().exists(), "{name}: no harness was started");
        assert!(!stdout.contains("case "), "{name}: {stdout}");
    }
    let world = World::new("claude-opens");
    let output = world.run(&["--harness", "claude", "--iterations", "0"], None);
    let (stdout, stderr) = text(&output);
    assert_eq!(output.status.code(), Some(3), "{stdout}\n{stderr}");
    assert_eq!(world.opened()[0]["scan"], "gates", "{stdout}");
}

/// Acceptance 2: the same runs with `scan: none` in their `--config` file, TOML or a
/// `loom.agent/1` document's `takt` member, open their case: a line before the first step names
/// `scan: none` and the file, the first line of the run is as before, and the `opened` event
/// holds `"scan":"none"`.
#[test]
fn a_declared_scan_none_opens_the_case_and_says_so_before_the_first_step() {
    for (name, flags) in outside_readers() {
        for (form, file, text_of) in [
            ("toml", "run.toml", "scan = \"none\"\n"),
            (
                "loom",
                "agent.yaml",
                "format: loom.agent/1\ntakt:\n  scan: none\n",
            ),
        ] {
            let label = format!("{name}-{form}");
            let world = World::new(&format!("declared-{label}"));
            let config = world.config(file, text_of);
            let mut args = flags.clone();
            args.extend(["--iterations", "0", "--config"]);
            args.push(config.to_str().expect("utf-8"));
            let output = world.run(&args, None);
            let (stdout, stderr) = text(&output);
            assert_eq!(output.status.code(), Some(3), "{label}: {stdout}\n{stderr}");
            let said = format!("scan: none (declared in {})", config.display());
            let at = stdout
                .find(&said)
                .unwrap_or_else(|| panic!("{label}: the line names the file: {stdout}"));
            let case = stdout
                .find("case ")
                .unwrap_or_else(|| panic!("{label}: the case opened: {stdout}"));
            assert!(
                at < case,
                "{label}: said before the run's first line: {stdout}"
            );
            assert!(
                stdout.contains("accepted unscanned")
                    && stdout.contains("the shape of a credential"),
                "{label}: {stdout}"
            );
            let first = stdout[case..].lines().next().unwrap_or_default();
            assert!(
                first.contains(&format!(
                    "confinement none: Substrate local, declared insecure: {}; proved: ",
                    takt::confinement::DEFAULT_REASON
                )),
                "{label}: story:confinement-through-substrate-on-linux-and-macos: the run's first line is unchanged but for the adapter `none` runs on, the default without --confinement: {first}"
            );
            // The tree's path may hold any word; what the line says after it does not name the
            // scan.
            let said_after = first
                .rsplit_once("; inputs ")
                .map_or(first, |(_, rest)| rest);
            assert!(!said_after.contains("scan"), "{label}: {first}");
            let opened = world.opened();
            assert_eq!(opened.len(), 1, "{label}: {opened:?}");
            assert_eq!(opened[0]["scan"], "none", "{label}: {}", opened[0]);
            assert!(!world.started().exists(), "{label}: no harness was started");
        }
    }
}

/// Acceptance 3: `scan: none` declared while the gates policy is set is refused before the case
/// opens, exit 1, naming both: one adapter per run.
#[test]
fn a_declared_scan_none_with_a_policy_set_is_refused_before_its_case_opens() {
    for (name, flags) in [
        ("codex", vec!["--harness", "codex"]),
        ("claude", vec!["--harness", "claude"]),
    ] {
        let world = World::new(&format!("both-{name}"));
        let config = world.config("run.toml", "scan = \"none\"\n");
        let policy = world.config_dir().join("policy.json");
        fs::write(&policy, "{}\n").expect("a policy file");
        let mut args = flags;
        args.extend(["--iterations", "0", "--config"]);
        args.push(config.to_str().expect("utf-8"));
        let output = world.run(&args, Some(&policy));
        let (stdout, stderr) = text(&output);
        assert_eq!(output.status.code(), Some(1), "{name}: {stdout}\n{stderr}");
        for named in [
            POLICY_VAR,
            "`scan: none`",
            &config.display().to_string(),
            "nothing was opened",
        ] {
            assert!(stderr.contains(named), "{name}: {named}: {stderr}");
        }
        assert!(!world.cases().exists(), "{name}: no case directory");
        assert!(!world.started().exists(), "{name}: no harness was started");
    }
}

/// Acceptance 4: `scan` with a value that names no adapter is refused before the case opens,
/// naming the file and the line and repeating no value, as any other bad field is; in TOML, in
/// YAML and in a `loom.agent/1` document.
#[test]
fn a_scan_that_names_no_adapter_is_refused_naming_the_file_and_the_line() {
    for (form, file, text_of, line) in [
        (
            "toml",
            "run.toml",
            "iterations = 0\nscan = \"MARKER-scan\"\n",
            "line 2",
        ),
        (
            "yaml",
            "run.yaml",
            "iterations: 0\nforge: none\nscan: MARKER-scan\n",
            "line 3",
        ),
        (
            "loom",
            "agent.yaml",
            "format: loom.agent/1\ntakt:\n  scan: MARKER-scan\n",
            "line 3",
        ),
    ] {
        let world = World::new(&format!("bad-{form}"));
        let config = world.config(file, text_of);
        let output = world.run(
            &[
                "--harness",
                "claude",
                "--config",
                config.to_str().expect("utf-8"),
            ],
            None,
        );
        let (stdout, stderr) = text(&output);
        assert_eq!(output.status.code(), Some(1), "{form}: {stdout}\n{stderr}");
        assert!(
            stderr.contains(&config.display().to_string()),
            "{form}: {stderr}"
        );
        assert!(stderr.contains(line), "{form}: {line}: {stderr}");
        assert!(
            stderr.contains("`scan`"),
            "{form}: the shape names scan: {stderr}"
        );
        assert!(
            !format!("{stdout}{stderr}").contains("MARKER-scan"),
            "{form}: {stderr}"
        );
        assert!(!world.cases().exists(), "{form}: no case directory");
    }
    let world = World::new("gates-declared");
    let config = world.config("run.toml", "scan = \"gates\"\n");
    let output = world.run(
        &[
            "--harness",
            "claude",
            "--iterations",
            "0",
            "--config",
            config.to_str().expect("utf-8"),
        ],
        None,
    );
    let (stdout, stderr) = text(&output);
    assert_eq!(output.status.code(), Some(3), "{stdout}\n{stderr}");
    assert!(!stdout.contains("scan: none"), "{stdout}");
    assert_eq!(world.opened()[0]["scan"], "gates");
}
