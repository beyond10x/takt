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

    /// `takt run` on the repository with `args` after the intent, HOME the scratch one, the
    /// double first on the program path, and no session bus, so no secrets store or keychain is
    /// reachable.
    fn run(&self, args: &[&str]) -> Output {
        self.run_in(None, args)
    }

    /// [`Self::run`] from the working directory `cwd`, else the test's own.
    fn run_in(&self, cwd: Option<&Path>, args: &[&str]) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_takt"));
        if let Some(cwd) = cwd {
            command.current_dir(cwd);
        }
        command
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
            .env_remove("DBUS_SESSION_BUS_ADDRESS")
            .env_remove("XDG_RUNTIME_DIR")
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

/// Asserts `output` opened a case on `--iterations 0` (exit 3) without starting b10x-harness or
/// writing under HOME, and that its `opened` line records the endpoint `base_url`, `wire` and
/// `model` with the secret `llm/opencode-go-api-key` by its namespace and name.
fn opened_on_the_secret(world: &World, output: &Output, base_url: &str, wire: &str, model: &str) {
    let (stdout, stderr) = text(output);
    assert_eq!(output.status.code(), Some(3), "{stdout}\n{stderr}");
    assert!(!world.started().exists(), "b10x-harness was not started");
    assert_eq!(
        fs::read_dir(world.home()).expect("HOME").count(),
        0,
        "nothing was written under HOME"
    );
    let opened = opened_lines(&world.cases());
    assert_eq!(opened.len(), 1, "{opened:?}");
    assert_eq!(
        opened[0]["endpoint"],
        serde_json::json!({
            "base_url": base_url,
            "wire": wire,
            "model": model,
            "credential": {
                "kind": "secret",
                "namespace": "llm",
                "name": "opencode-go-api-key",
            },
        })
    );
}

/// Acceptance 4 and 7: the trial arm's two endpoint files, each with the secret `llm` /
/// `opencode-go-api-key`, HOME an empty directory and no session bus, so no store or keychain is
/// reachable: takt reads no secret, the case opens and records the secret by name, and nothing
/// is written under HOME.
#[test]
fn the_trial_arm_endpoints_open_a_case_naming_the_secret() {
    for (name, wire, base_url, model) in [
        (
            "secret-anthropic",
            "anthropic-messages",
            "https://opencode.ai/inference/go/anthropic/v1",
            "claude-haiku-5-5",
        ),
        (
            "secret-openai",
            "openai-responses",
            "https://opencode.ai/inference/go/openai/v1",
            "gpt-6-luna",
        ),
    ] {
        let world = World::new(name);
        let file = world.outer().join("endpoint.toml");
        fs::write(
            &file,
            format!(
                "base_url = \"{base_url}\"\nwire = \"{wire}\"\nmodel = \"{model}\"\n\n[credential]\nsecret = \"llm/opencode-go-api-key\"\n"
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
        opened_on_the_secret(&world, &output, base_url, wire, model);
    }
}

/// `--endpoint-secret llm/opencode-go-api-key` with another base URL reaches a case: the secret
/// is the endpoint's own credential, so F2's refusal of the default credential to another host
/// does not apply, and the case records the secret by name.
#[test]
fn an_endpoint_secret_by_flag_reaches_a_case() {
    let world = World::new("secret-flag");
    let output = world.run(&[
        "--harness",
        "harness",
        "--endpoint-base-url",
        "https://opencode.ai/inference/go/anthropic/v1",
        "--endpoint-model",
        "claude-haiku-5-5",
        "--endpoint-secret",
        "llm/opencode-go-api-key",
        "--iterations",
        "0",
    ]);
    opened_on_the_secret(
        &world,
        &output,
        "https://opencode.ai/inference/go/anthropic/v1",
        "anthropic-messages",
        "claude-haiku-5-5",
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

/// Asserts `output` is a refusal before any case: exit 1, no case opened, b10x-harness not
/// started, and every one of `parts` in what it said.
fn refused_before_any_case(world: &World, output: &Output, parts: &[&str]) -> String {
    let (stdout, stderr) = text(output);
    assert_eq!(output.status.code(), Some(1), "{stdout}\n{stderr}");
    for part in parts {
        assert!(stderr.contains(part), "{part}: {stderr}");
    }
    assert!(!world.cases().exists(), "no case was opened");
    assert!(!world.started().exists(), "b10x-harness was not started");
    format!("{stdout}\n{stderr}")
}

/// F2: a base URL other than the default with no credential given is refused before any case:
/// the default credential, the user's claude.ai token, is not sent to another host.
#[test]
fn another_base_url_with_the_default_credential_is_refused_before_any_case() {
    let world = World::new("default-credential");
    let output = world.run(&[
        "--harness",
        "harness",
        "--endpoint-base-url",
        "https://endpoint.invalid/v1",
    ]);
    refused_before_any_case(
        &world,
        &output,
        &["https://endpoint.invalid/v1", "not sent to another host"],
    );
}

/// F2: `http://` to a host that is not loopback is refused before any case, a credential
/// given or not.
#[test]
fn an_http_base_url_to_another_host_is_refused_before_any_case() {
    let world = World::new("http");
    let token = world.outer().join("token.json");
    fs::write(&token, "{\"oauth\":{\"access\":\"t\"}}").expect("a token file");
    let output = world.run(&[
        "--harness",
        "harness",
        "--endpoint-base-url",
        "http://endpoint.invalid/v1",
        "--endpoint-token-file",
        token.to_str().expect("utf-8"),
    ]);
    refused_before_any_case(
        &world,
        &output,
        &["http://endpoint.invalid/v1", "no loopback host"],
    );
}

/// F3: a base URL with a userinfo and a query is refused before any case, naming both parts
/// and repeating nothing written in either.
#[test]
fn a_base_url_with_userinfo_and_a_query_is_refused_holding_neither() {
    let world = World::new("userinfo");
    let marker = "MARKER-a61f03-not-a-real-key";
    let token = world.outer().join("token.json");
    fs::write(&token, "{\"oauth\":{\"access\":\"t\"}}").expect("a token file");
    let base_url = format!("https://u:{marker}@endpoint.invalid/v1?key={marker}");
    let output = world.run(&[
        "--harness",
        "harness",
        "--endpoint-base-url",
        &base_url,
        "--endpoint-token-file",
        token.to_str().expect("utf-8"),
    ]);
    let said = refused_before_any_case(
        &world,
        &output,
        &["https://endpoint.invalid/v1", "userinfo", "query"],
    );
    assert!(!said.contains(marker), "the key leaked: {said}");
}

/// F7: a relative token file, by flag or in an endpoint file named by a relative path, is
/// read against the current directory and recorded absolute, the path the harness is given
/// though it runs in a scratch copy of the tree.
#[test]
fn a_relative_token_file_is_read_against_the_current_directory_and_recorded_absolute() {
    for (name, by_file) in [("relative-flag", false), ("relative-file", true)] {
        let world = World::new(name);
        let token = world.outer().join("token.json");
        fs::write(&token, "{\"oauth\":{\"access\":\"t\"}}").expect("a token file");
        fs::write(
            world.outer().join("endpoint.toml"),
            "base_url = \"https://endpoint.invalid/v1\"\n\n[credential]\ntoken_file = \"token.json\"\ntoken_pointer = \"/oauth/access\"\n",
        )
        .expect("an endpoint file");
        let mut args = vec!["--harness", "harness", "--iterations", "0"];
        if by_file {
            args.extend(["--endpoint-file", "endpoint.toml"]);
        } else {
            args.extend([
                "--endpoint-base-url",
                "https://endpoint.invalid/v1",
                "--endpoint-token-file",
                "token.json",
                "--endpoint-token-pointer",
                "/oauth/access",
            ]);
        }
        let output = world.run_in(Some(world.outer()), &args);
        let (stdout, stderr) = text(&output);
        assert_eq!(output.status.code(), Some(3), "{name}: {stdout}\n{stderr}");
        let opened = opened_lines(&world.cases());
        assert_eq!(opened.len(), 1, "{opened:?}");
        assert_eq!(
            opened[0]["endpoint"]["credential"],
            serde_json::json!({
                "kind": "subscription_file",
                "path": token.display().to_string(),
                "pointer": "/oauth/access",
            }),
            "{name}"
        );
    }
}

/// F10: with nothing but `--harness harness`, the default endpoint on the default token file
/// under HOME (a scratch one holding a marker token): the case opens, its `opened` line names
/// the token file from `~`, and no case file holds the marker or HOME's own path.
#[test]
fn the_default_token_file_is_recorded_from_tilde() {
    let world = World::new("tilde");
    let marker = "MARKER-e83d50-not-a-real-token";
    let dir = world.home().join(".claude");
    fs::create_dir_all(&dir).expect("a .claude directory");
    fs::write(
        dir.join(".credentials.json"),
        format!("{{\"claudeAiOauth\":{{\"accessToken\":\"{marker}\"}}}}"),
    )
    .expect("a token file");
    let output = world.run(&["--harness", "harness", "--iterations", "0"]);
    let (stdout, stderr) = text(&output);
    assert_eq!(output.status.code(), Some(3), "{stdout}\n{stderr}");
    assert!(!world.started().exists(), "b10x-harness was not started");
    let opened = opened_lines(&world.cases());
    assert_eq!(opened.len(), 1, "{opened:?}");
    assert_eq!(
        opened[0]["endpoint"],
        serde_json::json!({
            "base_url": "https://api.anthropic.com/v1",
            "wire": "anthropic-messages",
            "model": "claude-opus-5",
            "credential": {
                "kind": "subscription_file",
                "path": "~/.claude/.credentials.json",
                "pointer": "/claudeAiOauth/accessToken",
            },
        })
    );
    let home = world.home().display().to_string();
    for path in files_under(&world.cases()) {
        let text = String::from_utf8_lossy(&fs::read(&path).expect("a case file")).into_owned();
        assert!(
            !text.contains(marker),
            "the token reached {}",
            path.display()
        );
        assert!(!text.contains(&home), "HOME reached {}", path.display());
    }
    assert!(
        !stdout.contains(marker) && !stderr.contains(marker),
        "{stdout}\n{stderr}"
    );
}
