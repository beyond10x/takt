//! story:harness-endpoint-is-configured: `takt run --harness harness` takes its endpoint (base
//! URL, wire, model, credential) from `--endpoint-*` flags or an `--endpoint-file`, a flag
//! winning over the file. These runs reach no network and no real credential: every token file
//! is a scratch file, HOME is a scratch directory, and a `b10x-harness` double on the program
//! path records whether it was started.

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

/// A scratch directory under this crate's build directory, removed on drop.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// A scratch tree: a repository with one commit, an empty HOME, and a program directory holding
/// a `b10x-harness` double that leaves `started` behind when it runs.
struct World {
    scratch: Scratch,
}

impl World {
    fn new(name: &str) -> Self {
        let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("harness-endpoint-{name}-{}", std::process::id()));
        if outer.exists() {
            fs::remove_dir_all(&outer).expect("a stale scratch tree is removed");
        }
        let world = Self {
            scratch: Scratch(outer),
        };
        fs::create_dir_all(world.repo()).expect("a repository");
        fs::create_dir_all(world.home()).expect("a HOME");
        fs::create_dir_all(world.bin()).expect("a program directory");
        fs::write(world.repo().join("README.md"), "a story\n").expect("a file");
        git(&world.repo(), &["init", "-q", "-b", "main"]);
        git(&world.repo(), &["add", "-A"]);
        git(&world.repo(), &["commit", "-q", "-m", "start"]);
        let double = world.bin().join("b10x-harness");
        fs::write(
            &double,
            format!("#!/bin/sh\ntouch '{}'\nexit 1\n", world.started().display()),
        )
        .expect("a double");
        fs::set_permissions(&double, fs::Permissions::from_mode(0o755)).expect("exec bit");
        world
    }

    fn outer(&self) -> &Path {
        &self.scratch.0
    }

    fn repo(&self) -> PathBuf {
        self.outer().join("repo")
    }

    fn home(&self) -> PathBuf {
        self.outer().join("home")
    }

    fn bin(&self) -> PathBuf {
        self.outer().join("bin")
    }

    fn started(&self) -> PathBuf {
        self.outer().join("started")
    }

    fn cases(&self) -> PathBuf {
        self.repo().join(".engineering/cases")
    }

    /// `takt run` on the repository with `args` after the intent, HOME the scratch one and the
    /// double first on the program path.
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
            .env("XDG_STATE_HOME", self.outer().join("xdg-state"))
            .env_remove("B10X_GATES_POLICY")
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

/// Every regular file under `dir`, a link never followed.
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

#[test]
fn run_help_names_every_endpoint_flag() {
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args(["run", "--help"])
        .output()
        .expect("takt runs");
    let (help, _) = text(&output);
    assert_eq!(output.status.code(), Some(0), "{help}");
    for flag in [
        "--endpoint-base-url",
        "--endpoint-wire",
        "--endpoint-model",
        "--endpoint-token-file",
        "--endpoint-token-pointer",
        "--endpoint-secret",
        "--endpoint-file",
    ] {
        assert!(help.contains(flag), "takt run --help names {flag}: {help}");
    }
}

#[test]
fn an_endpoint_flag_with_another_harness_is_refused_before_any_case() {
    let world = World::new("other-harness");
    let output = world.run(&["--harness", "none", "--endpoint-model", "m"]);
    let (stdout, stderr) = text(&output);
    assert_eq!(output.status.code(), Some(1), "{stdout}\n{stderr}");
    assert!(
        stderr.contains("--endpoint-model") && stderr.contains("--harness none"),
        "{stderr}"
    );
    assert_eq!(stderr.trim().lines().count(), 1, "one line: {stderr}");
    assert!(!world.cases().exists(), "no case was opened");
}

#[test]
fn a_missing_token_file_fails_before_the_first_step_naming_path_and_pointer() {
    let world = World::new("missing-token");
    let absent = world.outer().join("absent.json");
    let output = world.run(&[
        "--harness",
        "harness",
        "--endpoint-token-file",
        absent.to_str().expect("utf-8"),
        "--endpoint-token-pointer",
        "/claudeAiOauth/accessToken",
    ]);
    let (stdout, stderr) = text(&output);
    assert_eq!(output.status.code(), Some(1), "{stdout}\n{stderr}");
    assert!(
        stderr.contains(&absent.display().to_string())
            && stderr.contains("/claudeAiOauth/accessToken"),
        "{stderr}"
    );
    assert!(!world.cases().exists(), "no case was opened");
    assert!(!world.started().exists(), "b10x-harness was not started");
}

#[test]
fn a_dangling_pointer_fails_before_the_first_step_naming_path_and_pointer() {
    let world = World::new("dangling-pointer");
    let token = world.outer().join("token.json");
    fs::write(&token, "{\"a\":{}}").expect("a token file");
    let output = world.run(&[
        "--harness",
        "harness",
        "--endpoint-token-file",
        token.to_str().expect("utf-8"),
        "--endpoint-token-pointer",
        "/a/b",
    ]);
    let (stdout, stderr) = text(&output);
    assert_eq!(output.status.code(), Some(1), "{stdout}\n{stderr}");
    assert!(
        stderr.contains(&token.display().to_string()) && stderr.contains("/a/b"),
        "{stderr}"
    );
    assert!(!world.cases().exists(), "no case was opened");
    assert!(!world.started().exists(), "b10x-harness was not started");
}

#[test]
fn two_credentials_are_refused_before_any_case() {
    let world = World::new("two-credentials");
    let output = world.run(&[
        "--harness",
        "harness",
        "--endpoint-token-file",
        "/nonexistent/token.json",
        "--endpoint-secret",
        "llm/opencode-go-api-key",
    ]);
    let (stdout, stderr) = text(&output);
    assert_eq!(output.status.code(), Some(1), "{stdout}\n{stderr}");
    assert!(
        stderr.contains("--endpoint-token-file") && stderr.contains("--endpoint-secret"),
        "{stderr}"
    );
    assert!(!world.cases().exists(), "no case was opened");
}

/// Acceptance 4 and 7: the trial arm's endpoint file with the secret `llm` /
/// `opencode-go-api-key`, HOME an empty directory so no store or keychain is reachable through
/// it: takt reads no secret, and since the pinned session executor carries no secret's name,
/// the run is refused before any case, naming it, the endpoint and the secret's parts.
#[test]
fn a_secret_name_is_refused_naming_the_session_executor_and_the_secret() {
    let world = World::new("secret");
    let file = world.outer().join("endpoint.toml");
    fs::write(
        &file,
        "base_url = \"https://opencode.ai/inference/go/anthropic/v1\"\nwire = \"anthropic-messages\"\nmodel = \"claude-haiku-5-5\"\n\n[credential]\nsecret = \"llm/opencode-go-api-key\"\n",
    )
    .expect("an endpoint file");
    let output = world.run(&[
        "--harness",
        "harness",
        "--endpoint-file",
        file.to_str().expect("utf-8"),
    ]);
    let (stdout, stderr) = text(&output);
    assert_eq!(output.status.code(), Some(1), "{stdout}\n{stderr}");
    for part in [
        "loom 0.39.0",
        "namespace llm",
        "name opencode-go-api-key",
        "https://opencode.ai/inference/go/anthropic/v1",
        "anthropic-messages",
        "claude-haiku-5-5",
    ] {
        assert!(stderr.contains(part), "{part}: {stderr}");
    }
    assert!(!world.cases().exists(), "no case was opened");
    assert!(!world.started().exists(), "b10x-harness was not started");
    assert_eq!(
        fs::read_dir(world.home()).expect("HOME").count(),
        0,
        "nothing was written under HOME"
    );
}

/// The `opened` lines of the one case under `cases`, each as JSON.
fn opened_lines(cases: &Path) -> Vec<serde_json::Value> {
    let events: Vec<PathBuf> = files_under(cases)
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

/// Acceptance 5: a marker token in a scratch token file named by an endpoint file reaches no
/// case file, no output and no argument, while the case records the endpoint by reference: its
/// `opened` line names the base URL, the wire, the model, and the token file's path and
/// pointer. The case opens on `--iterations 0` and stops before its first step, b10x-harness
/// never started.
#[test]
fn a_marker_token_reaches_no_case_file() {
    let world = World::new("marker");
    let marker = "MARKER-0c4e9b-not-a-real-token";
    let token = world.outer().join("token.json");
    fs::write(&token, format!("{{\"oauth\":{{\"access\":\"{marker}\"}}}}")).expect("a token");
    let file = world.outer().join("endpoint.toml");
    fs::write(
        &file,
        format!(
            "base_url = \"https://endpoint.invalid/v1\"\nwire = \"openai-responses\"\nmodel = \"m\"\n\n[credential]\ntoken_file = \"{}\"\ntoken_pointer = \"/oauth/access\"\n",
            token.display()
        ),
    )
    .expect("an endpoint file");
    let output = world.run(&[
        "--harness",
        "harness",
        "--endpoint-file",
        file.to_str().expect("utf-8"),
        "--iterations",
        "0",
    ]);
    let (stdout, stderr) = text(&output);
    assert_eq!(output.status.code(), Some(3), "{stdout}\n{stderr}");
    assert!(
        world.cases().is_dir(),
        "the case was opened: {stdout}\n{stderr}"
    );
    assert!(!world.started().exists(), "b10x-harness was not started");
    let files = files_under(&world.cases());
    assert!(!files.is_empty(), "the case holds files");
    let mut naming = Vec::new();
    for path in files {
        let bytes = fs::read(&path).expect("a case file");
        let text = String::from_utf8_lossy(&bytes);
        assert!(
            !text.contains(marker),
            "the token reached {}",
            path.display()
        );
        if text.contains(&token.display().to_string()) && text.contains("/oauth/access") {
            naming.push(path);
        }
    }
    assert!(
        !naming.is_empty(),
        "a case file names the token file and its pointer"
    );
    assert!(
        !stdout.contains(marker) && !stderr.contains(marker),
        "{stdout}\n{stderr}"
    );
    let opened = opened_lines(&world.cases());
    assert_eq!(opened.len(), 1, "{opened:?}");
    assert_eq!(
        opened[0]["endpoint"],
        serde_json::json!({
            "base_url": "https://endpoint.invalid/v1",
            "wire": "openai-responses",
            "model": "m",
            "credential": {
                "kind": "subscription_file",
                "path": token.display().to_string(),
                "pointer": "/oauth/access",
            },
        })
    );
}

/// A run whose harness is pointed at no endpoint records none: `--harness none` opens a case
/// whose `opened` line has no `endpoint` field.
#[test]
fn a_run_without_an_endpoint_records_none() {
    let world = World::new("no-endpoint");
    let output = world.run(&["--harness", "none", "--iterations", "0"]);
    let (stdout, stderr) = text(&output);
    assert_eq!(output.status.code(), Some(3), "{stdout}\n{stderr}");
    let opened = opened_lines(&world.cases());
    assert_eq!(opened.len(), 1, "{opened:?}");
    assert!(opened[0].get("endpoint").is_none(), "{}", opened[0]);
}
