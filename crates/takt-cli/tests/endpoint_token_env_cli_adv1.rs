//! Adversary, story:run-configuration-names-the-story-and-token-variable: the run
//! configuration's `endpoint_token_env` is taken "only when no credential flag is given", and a
//! flag on the command line wins over the file. `--endpoint-file` on the command line names a
//! credential too (its `[credential]` table); the configuration file's variable must not win
//! over it. These runs reach no network: the endpoint is `.invalid`, `--iterations 0` stops the
//! run before any step, and the variable's value is a marker.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const VARIABLE: &str = "TAKT_TEST_ENDPOINT_KEY_ADV1";
const MARKER: &str = "MARKER-adv1-not-a-real-key";

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

/// A scratch world under this crate's build directory, removed on drop: a repository with one
/// commit, an empty HOME and a runtime directory of mode 0700.
struct World(PathBuf);

impl Drop for World {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

impl World {
    fn new(name: &str) -> Self {
        use std::os::unix::fs::PermissionsExt as _;
        let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("endpoint-env-adv1-{name}-{}", std::process::id()));
        if outer.exists() {
            fs::remove_dir_all(&outer).expect("a stale scratch tree is removed");
        }
        let world = Self(outer);
        for dir in [world.repo(), world.home(), world.runtime()] {
            fs::create_dir_all(&dir).expect("a directory");
        }
        fs::set_permissions(world.runtime(), fs::Permissions::from_mode(0o700))
            .expect("a private runtime directory");
        fs::write(world.repo().join("README.md"), "a story\n").expect("a file");
        git(&world.repo(), &["init", "-q", "-b", "main"]);
        git(&world.repo(), &["add", "-A"]);
        git(&world.repo(), &["commit", "-q", "-m", "start"]);
        world
    }

    fn repo(&self) -> PathBuf {
        self.0.join("repo")
    }

    fn home(&self) -> PathBuf {
        self.0.join("home")
    }

    fn runtime(&self) -> PathBuf {
        self.0.join("runtime")
    }

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
            .env("XDG_STATE_HOME", self.0.join("xdg-state"))
            .env("XDG_RUNTIME_DIR", self.runtime())
            .env_remove("DBUS_SESSION_BUS_ADDRESS")
            .env_remove("B10X_GATES_POLICY")
            .env_remove("ANTHROPIC_API_KEY")
            .env(VARIABLE, MARKER)
            .output()
            .expect("takt runs")
    }

    /// The `opened` lines of the one case, each as JSON.
    fn opened(&self) -> Vec<serde_json::Value> {
        let cases = self.repo().join(".engineering/cases");
        let case: Vec<PathBuf> = fs::read_dir(&cases)
            .expect("the cases directory")
            .map(|entry| entry.expect("an entry").path())
            .collect();
        assert_eq!(case.len(), 1, "{case:?}");
        fs::read_to_string(case[0].join("events.jsonl"))
            .expect("the run record")
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("a JSON line"))
            .filter(|line| line["event"] == "opened")
            .collect()
    }
}

/// `--endpoint-file` given on the command line, its `[credential]` a secret's name, beside a
/// `--config` file whose `endpoint_token_env` names a variable: the command line's endpoint
/// file names the credential, so the run records the secret, not the file's variable. Today
/// the configuration's variable is put in the flag's place (`LoopArgs::configured`, which
/// checks only `--endpoint-token-env`, `--endpoint-token-file` and `--endpoint-secret`), and
/// a flag's credential wins over any endpoint file's (`EndpointConfig::of`), so the file beats
/// the command line.
#[test]
fn a_command_line_endpoint_files_credential_wins_over_the_configurations_variable() {
    let world = World::new("cli-endpoint-file");
    let endpoint = world.0.join("endpoint.toml");
    fs::write(
        &endpoint,
        "base_url = \"https://endpoint.invalid/v1\"\nwire = \"openai-responses\"\nmodel = \"m\"\n\n[credential]\nsecret = \"llm/key\"\n",
    )
    .expect("an endpoint file");
    let config = world.0.join("run.toml");
    fs::write(
        &config,
        format!("harness = \"harness\"\nendpoint_token_env = \"{VARIABLE}\"\n"),
    )
    .expect("a run configuration file");
    let output = world.run(&[
        "--config",
        config.to_str().expect("utf-8"),
        "--endpoint-file",
        endpoint.to_str().expect("utf-8"),
        "--iterations",
        "0",
    ]);
    let said = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.status.code(), Some(3), "{said}");
    assert_eq!(
        world.opened()[0]["endpoint"]["credential"],
        serde_json::json!({"kind": "secret", "namespace": "llm", "name": "key"}),
        "the command line's endpoint file names the credential\n{said}"
    );
}

/// `--harness claude` given on the command line at its default value still wins over the
/// file's `harness = "harness"`: `unset` must read where the value came from, not whether it
/// equals the default. Were the file's harness taken, its `endpoint_token_env` would name an
/// unset variable and the run would be refused, exit 1; with the flag's, the run opens and
/// stops `Iterations (0)`, exit 3. Kills the mutant "given = differs from the default", which
/// every existing precedence test (each flag at a non-default value) lets through.
#[test]
fn a_flag_given_at_its_default_value_still_wins_over_the_file() {
    let world = World::new("default-flag");
    let config = world.0.join("run.toml");
    fs::write(
        &config,
        "harness = \"harness\"\nendpoint_token_env = \"TAKT_TEST_UNSET_ADV1\"\n",
    )
    .expect("a run configuration file");
    let output = world.run(&[
        "--config",
        config.to_str().expect("utf-8"),
        "--harness",
        "claude",
        "--iterations",
        "0",
    ]);
    let said = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.status.code(), Some(3), "{said}");
    assert!(world.opened()[0]["endpoint"].is_null(), "{said}");
}
