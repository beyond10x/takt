//! Adversary pass 1 on story:scan-and-forge-are-adapters at the binary's edge. These runs reach
//! no network and start no harness: HOME is a scratch directory, every run stops before its
//! first step, and a `codex` double on the program path records whether it was started.

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

/// A scratch repository with one commit, an empty HOME, a `config` directory and a `codex`
/// double that leaves `started` behind when it runs; removed on drop.
struct World(PathBuf);

impl Drop for World {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

impl World {
    fn new(name: &str) -> Self {
        let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("scan-adapter-adv1-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&outer);
        let world = Self(outer);
        for dir in [world.repo(), world.home(), world.bin(), world.config_dir()] {
            fs::create_dir_all(dir).expect("a directory");
        }
        fs::write(world.repo().join("README.md"), "a story\n").expect("a file");
        git(&world.repo(), &["init", "-q", "-b", "main"]);
        git(&world.repo(), &["add", "-A"]);
        git(&world.repo(), &["commit", "-q", "-m", "start"]);
        let double = world.bin().join("codex");
        fs::write(
            &double,
            format!("#!/bin/sh\ntouch '{}'\nexit 1\n", world.started().display()),
        )
        .expect("a double");
        fs::set_permissions(&double, fs::Permissions::from_mode(0o755)).expect("exec bit");
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

    fn config(&self, name: &str, text: &str) -> PathBuf {
        let path = self.config_dir().join(name);
        fs::write(&path, text).expect("a run configuration file");
        path
    }

    /// `takt run` on the repository with an intent, no forge and `args`, HOME the scratch one,
    /// the double first on the program path.
    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_takt"))
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
            .env_remove("ANTHROPIC_API_KEY")
            .output()
            .expect("takt runs")
    }
}

fn text(output: &Output) -> (String, String) {
    (
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// Acceptance 4 end to end, under the harness Acceptance 1 guards: a `--harness codex` run with
/// no scan adapter declared whose TOML file holds `scan = { none = {} }`, a table and not
/// `none`, is refused before its case opens, naming the file, exit 1. It must not be taken as
/// the declaration `scan: none` and open a case that accepts what the session proposes
/// unscanned.
#[test]
fn a_codex_run_whose_scan_is_a_mapping_is_refused_before_its_case_opens() {
    let world = World::new("mapping");
    let config = world.config("run.toml", "iterations = 0\nscan = { none = {} }\n");
    let output = world.run(&[
        "--harness",
        "codex",
        "--config",
        config.to_str().expect("utf-8"),
    ]);
    let (stdout, stderr) = text(&output);
    assert!(
        !stdout.contains("scan: none (declared in"),
        "a mapping was taken as the declaration `scan: none`:\n{stdout}\n{stderr}"
    );
    assert_eq!(output.status.code(), Some(1), "{stdout}\n{stderr}");
    assert!(
        stderr.contains(&config.display().to_string()),
        "the refusal names the file: {stderr}"
    );
    assert!(!world.cases().exists(), "no case directory");
    assert!(!world.started().exists(), "no harness was started");
}

/// The reference page of the run configuration file (`website/docs/reference/configuration.md`)
/// read as the contract it is for a user writing one: its table of fields lists `scan`, the
/// field this story adds, and the refusal it quotes for an unknown key, "run for real", is the
/// one the binary prints now. The story's Design names the configuration page among those the
/// change updates.
#[test]
fn the_configuration_reference_documents_scan_as_the_binary_reads_it() {
    let page_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../website/docs/reference/configuration.md");
    let page = fs::read_to_string(&page_path).expect("the configuration reference");
    let mut drift = Vec::new();

    let rows: Vec<&str> = page
        .lines()
        .skip_while(|line| line.trim() != "| field | meaning |")
        .skip(2)
        .take_while(|line| line.starts_with('|'))
        .collect();
    assert!(!rows.is_empty(), "the page has its table of fields");
    if !rows.iter().any(|row| row.starts_with("| `scan` |")) {
        drift.push(format!(
            "the table of fields has no `scan` row: {}",
            rows.iter()
                .filter_map(|row| row.split('`').nth(1))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    let marker = "is refused at line 3: ";
    let quoted = page
        .lines()
        .find(|line| {
            line.starts_with("takt: the run configuration file unknown.toml")
                && line.contains(marker)
        })
        .and_then(|line| line.split_once(marker))
        .map(|(_, shape)| shape.to_owned())
        .expect("the page quotes the refusal of unknown.toml");
    let world = World::new("reference");
    let config = world.config(
        "unknown.toml",
        "iterations = 1\nharness = \"none\"\nunknown = 1\n",
    );
    let output = world.run(&["--config", config.to_str().expect("utf-8")]);
    let (stdout, stderr) = text(&output);
    assert_eq!(output.status.code(), Some(1), "{stdout}\n{stderr}");
    let printed = stderr
        .lines()
        .find_map(|line| line.split_once(marker))
        .map(|(_, shape)| shape.to_owned())
        .unwrap_or_else(|| panic!("the binary refuses at line 3: {stderr}"));
    if printed != quoted {
        drift.push(format!(
            "the page quotes\n  {quoted}\nthe binary prints\n  {printed}"
        ));
    }

    assert!(
        drift.is_empty(),
        "{} drifts from the binary:\n{}",
        page_path.display(),
        drift.join("\n")
    );
}
