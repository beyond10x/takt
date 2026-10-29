//! story:run-configuration-file: `takt run --config <FILE>` takes the loop's budget, the
//! harness, its model and endpoint file, the forge and the test command from one TOML file, a
//! flag given on the command line winning over the file. These runs reach no network and no
//! real credential: HOME is a scratch directory, the endpoint names a secret by name only, and a
//! `b10x-harness` double on the program path records whether it was started.

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

/// A scratch tree under this crate's build directory, removed on drop: a repository with one
/// commit, an empty HOME, a `config` directory and a program directory holding a `b10x-harness`
/// double that leaves `started` behind when it runs.
struct World(PathBuf);

impl Drop for World {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

impl World {
    fn new(name: &str) -> Self {
        let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("run-config-{name}-{}", std::process::id()));
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
        let double = world.bin().join("b10x-harness");
        fs::write(
            &double,
            format!("#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then echo double; exit 0; fi\ntouch '{}'\nexit 1\n", world.started().display()),
        )
        .expect("a double");
        fs::set_permissions(&double, fs::Permissions::from_mode(0o755)).expect("exec bit");
        fs::write(
            world.config_dir().join("endpoint.toml"),
            "base_url = \"https://models.example/v1\"\nwire = \"openai-responses\"\nmodel = \"file-model\"\n\n[credential]\nsecret = \"llm/model-key\"\n",
        )
        .expect("an endpoint file");
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

    /// Writes `text` as `config/run.toml` and returns its path.
    fn config(&self, text: &str) -> PathBuf {
        self.config_named("run.toml", text)
    }

    /// Writes `text` as `config/<name>` and returns its path.
    fn config_named(&self, name: &str, text: &str) -> PathBuf {
        let path = self.config_dir().join(name);
        fs::write(&path, text).expect("a run configuration file");
        path
    }

    /// `takt run` on the repository with an intent, `--config <config>` and `args`, HOME the
    /// scratch one, the double first on the program path, no session bus.
    fn run(&self, config: &Path, args: &[&str]) -> Output {
        self.run_env(config, args, &[])
    }

    /// [`World::run`] with `env` set on top of the scratch environment.
    fn run_env(&self, config: &Path, args: &[&str], env: &[(&str, &str)]) -> Output {
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
            // A harness run opens only under a policy or a declared `scan: none`
            // (story:scan-and-forge-are-adapters); no step here reaches a scan.
            .env("B10X_GATES_POLICY", self.0.join("gates-policy.json"))
            .env_remove("ANTHROPIC_API_KEY")
            .envs(env.iter().copied())
            .output()
            .expect("takt runs")
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

fn text(output: &Output) -> String {
    format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

#[test]
fn run_help_names_config() {
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args(["run", "--help"])
        .output()
        .expect("takt runs");
    assert_eq!(output.status.code(), Some(0), "{}", text(&output));
    assert!(
        text(&output).contains("--config <FILE>"),
        "{}",
        text(&output)
    );
}

/// The harness, the iteration budget, the forge, the model and the endpoint file (relative,
/// read beside the configuration) all come from the file: the case opens on `iterations = 0`,
/// exit 3, before any step, and its `opened` line records the file's endpoint with the file's
/// model over the endpoint file's.
#[test]
fn the_file_sets_harness_budget_forge_model_and_endpoint() {
    let world = World::new("all");
    let config = world.config(
        "iterations = 0\nharness = \"harness\"\nmodel = \"run-model\"\nendpoint_file = \"endpoint.toml\"\nforge = \"none\"\ntest_cmd = \"true\"\n",
    );
    let output = world.run(&config, &[]);
    assert_eq!(output.status.code(), Some(3), "{}", text(&output));
    assert!(!world.started().exists(), "b10x-harness was not started");
    let opened = world.opened();
    assert_eq!(opened.len(), 1, "{opened:?}");
    assert_eq!(
        opened[0]["endpoint"],
        serde_json::json!({
            "base_url": "https://models.example/v1",
            "wire": "openai-responses",
            "model": "run-model",
            "credential": {"kind": "secret", "namespace": "llm", "name": "model-key"},
        })
    );
}

/// A flag on the command line wins over the file: the file's `iterations = 5` would start the
/// harness double, `--iterations 0` opens the case and stops before it.
#[test]
fn a_flag_wins_over_the_file() {
    let world = World::new("flag-wins");
    let config = world.config(
        "iterations = 5\nharness = \"harness\"\nendpoint_file = \"endpoint.toml\"\nforge = \"none\"\n",
    );
    let output = world.run(&config, &["--iterations", "0", "--model", "flag-model"]);
    assert_eq!(output.status.code(), Some(3), "{}", text(&output));
    assert!(!world.started().exists(), "b10x-harness was not started");
    assert_eq!(world.opened()[0]["endpoint"]["model"], "flag-model");
}

/// A key the file may not hold is refused before any case opens, exit 1, naming the file and
/// the line and repeating no value.
#[test]
fn an_unknown_key_is_refused_before_any_case() {
    let world = World::new("unknown");
    let config = world.config("iterations = 0\napi_key = \"MARKER-run-config-value\"\n");
    let output = world.run(&config, &["--forge", "none"]);
    let all = text(&output);
    assert_eq!(output.status.code(), Some(1), "{all}");
    assert!(all.contains(&config.display().to_string()), "{all}");
    assert!(all.contains("line 2"), "{all}");
    assert!(!all.contains("MARKER-run-config-value"), "{all}");
    assert!(!world.cases().exists(), "no case was opened");
}

/// A configuration file that does not exist is refused before any case opens, naming it.
#[test]
fn a_missing_file_is_refused_naming_it() {
    let world = World::new("missing");
    let absent = world.config_dir().join("absent.toml");
    let output = world.run(&absent, &["--forge", "none", "--iterations", "0"]);
    let all = text(&output);
    assert_eq!(output.status.code(), Some(1), "{all}");
    assert!(all.contains(&absent.display().to_string()), "{all}");
    assert!(!world.cases().exists(), "no case was opened");
}

/// An hour budget from the file is checked as the flag's is: `hours = 0` is refused before any
/// case opens.
#[test]
fn an_hour_budget_from_the_file_is_checked() {
    let world = World::new("hours");
    let config = world.config("hours = 0\nharness = \"none\"\nforge = \"none\"\n");
    let output = world.run(&config, &[]);
    let all = text(&output);
    assert_eq!(output.status.code(), Some(1), "{all}");
    assert!(all.contains("--hours is 0"), "{all}");
    assert!(!world.cases().exists(), "no case was opened");
}

/// The forge comes from the file: `forge = "bot"` on a repository with no `origin` is refused
/// naming the bot forge, where the default would have been `none`.
#[test]
fn the_forge_comes_from_the_file() {
    let world = World::new("forge");
    let config = world.config("forge = \"bot\"\nharness = \"none\"\niterations = 0\n");
    let output = world.run(&config, &[]);
    let all = text(&output);
    assert_eq!(output.status.code(), Some(1), "{all}");
    assert!(all.contains("the forge `bot`"), "{all}");
    assert!(!world.cases().exists(), "no case was opened");
}

/// The test command comes from the file: an empty one is refused, where the default would run.
#[test]
fn the_test_command_comes_from_the_file() {
    let world = World::new("test-cmd");
    let config =
        world.config("test_cmd = \"\"\nharness = \"none\"\nforge = \"none\"\niterations = 0\n");
    let output = world.run(&config, &[]);
    let all = text(&output);
    assert_ne!(output.status.code(), Some(3), "{all}");
    assert!(!world.cases().exists(), "no case was opened");
}

/// A harness flag wins over the file's, and the file's endpoint, b10x-harness's, is then left
/// out rather than refused: the case opens on `--harness none`.
#[test]
fn a_harness_flag_over_the_file_leaves_its_endpoint_out() {
    let world = World::new("harness-flag");
    let config = world.config(
        "iterations = 0\nharness = \"harness\"\nendpoint_file = \"endpoint.toml\"\nforge = \"none\"\n",
    );
    let output = world.run(&config, &["--harness", "none"]);
    assert_eq!(output.status.code(), Some(3), "{}", text(&output));
    assert!(!world.started().exists(), "b10x-harness was not started");
    assert!(
        world.opened()[0]
            .get("endpoint")
            .is_none_or(|e| e.is_null()),
        "{:?}",
        world.opened()
    );
}

// story:one-config-contract-with-loom: `--config` reads TOML or YAML by the file's extension, and
// a `loom.agent/1` document, the file Loom mounts at `/config/agent.yaml`, by its `takt` member.

/// A Loom agent document as a `LoomAgent`'s `spec.agent` carries it, with `takt` appended.
const LOOM_AGENT: &str = "format: loom.agent/1\nagent:\n  id: takt-a\n  name: takt A\n  brief: delivers stories\nmodel:\n  routes: routes.yaml\n  roles:\n    - {role: agent, model: luna}\ndeploy:\n  kubernetes:\n    namespace: loom-agents\n    egress:\n      - {name: models, host: models.example, port: 443}\n";

/// The expected `opened` endpoint of a run whose configuration names `endpoint.toml` and the
/// model `run-model`.
fn configured_endpoint() -> serde_json::Value {
    serde_json::json!({
        "base_url": "https://models.example/v1",
        "wire": "openai-responses",
        "model": "run-model",
        "credential": {"kind": "secret", "namespace": "llm", "name": "model-key"},
    })
}

/// A YAML file sets the same run as the TOML one from the same fields.
#[test]
fn a_yaml_file_sets_the_run_as_toml_does() {
    for name in ["run.yaml", "run.yml"] {
        let world = World::new(&format!("yaml-{name}"));
        let config = world.config_named(
            name,
            "iterations: 0\nharness: harness\nmodel: run-model\nendpoint_file: endpoint.toml\nforge: none\ntest_cmd: \"true\"\n",
        );
        let output = world.run(&config, &[]);
        assert_eq!(output.status.code(), Some(3), "{name}: {}", text(&output));
        assert!(!world.started().exists(), "b10x-harness was not started");
        assert_eq!(
            world.opened()[0]["endpoint"],
            configured_endpoint(),
            "{name}"
        );
    }
}

/// A `loom.agent/1` document's `takt` member is the run configuration; Loom's members are not
/// read by takt.
#[test]
fn a_loom_agent_documents_takt_member_sets_the_run() {
    let world = World::new("loom-takt");
    let config = world.config_named(
        "agent.yaml",
        &format!(
            "{LOOM_AGENT}takt:\n  iterations: 0\n  harness: harness\n  model: run-model\n  endpoint_file: endpoint.toml\n  forge: none\n  test_cmd: \"true\"\n"
        ),
    );
    let output = world.run(&config, &[]);
    assert_eq!(output.status.code(), Some(3), "{}", text(&output));
    assert!(!world.started().exists(), "b10x-harness was not started");
    assert_eq!(world.opened()[0]["endpoint"], configured_endpoint());
}

/// A `loom.agent/1` document with no `takt` member sets nothing: the run takes its flags, as a
/// resource's `spec.command.args` give them.
#[test]
fn a_loom_agent_document_without_takt_sets_nothing() {
    let world = World::new("loom-bare");
    let config = world.config_named("agent.yaml", LOOM_AGENT);
    let output = world.run(
        &config,
        &["--iterations", "0", "--harness", "none", "--forge", "none"],
    );
    assert_eq!(output.status.code(), Some(3), "{}", text(&output));
    assert_eq!(world.opened().len(), 1);
    assert!(
        world.opened()[0]
            .get("endpoint")
            .is_none_or(|e| e.is_null()),
        "{:?}",
        world.opened()
    );
}

/// A key the `takt` member may not hold is refused before any case opens, exit 1, naming the
/// file and the line and repeating no value; so is one in a plain YAML file.
#[test]
fn an_unknown_yaml_key_is_refused_naming_file_and_line() {
    let world = World::new("loom-unknown");
    let config = world.config_named(
        "agent.yaml",
        &format!("{LOOM_AGENT}takt:\n  iterations: 0\n  api_key: MARKER-loom-config-value\n"),
    );
    let output = world.run(&config, &["--forge", "none"]);
    let all = text(&output);
    assert_eq!(output.status.code(), Some(1), "{all}");
    assert!(all.contains(&config.display().to_string()), "{all}");
    let line = LOOM_AGENT.lines().count() + 3;
    assert!(all.contains(&format!("line {line}")), "{all}");
    assert!(!all.contains("MARKER-loom-config-value"), "{all}");
    assert!(!world.cases().exists(), "no case was opened");

    let world = World::new("yaml-unknown");
    let config = world.config_named(
        "run.yaml",
        "iterations: 0\napi_key: MARKER-yaml-config-value\n",
    );
    let output = world.run(&config, &["--forge", "none"]);
    let all = text(&output);
    assert_eq!(output.status.code(), Some(1), "{all}");
    assert!(all.contains(&config.display().to_string()), "{all}");
    assert!(all.contains("line 2"), "{all}");
    assert!(!all.contains("MARKER-yaml-config-value"), "{all}");
    assert!(!world.cases().exists(), "no case was opened");
}

/// A file of another extension, or a YAML document of another format, is refused before any case
/// opens, exit 1, naming the file and repeating no value.
#[test]
fn another_extension_or_format_is_refused_naming_the_file() {
    let world = World::new("extension");
    let config = world.config_named("run.json", "{\"iterations\": 0}\n");
    let output = world.run(&config, &["--forge", "none"]);
    let all = text(&output);
    assert_eq!(output.status.code(), Some(1), "{all}");
    assert!(all.contains(&config.display().to_string()), "{all}");
    assert!(!world.cases().exists(), "no case was opened");

    let world = World::new("format");
    let config = world.config_named(
        "agent.yaml",
        "format: MARKER-other-format/1\ntakt:\n  iterations: 0\n",
    );
    let output = world.run(&config, &["--forge", "none"]);
    let all = text(&output);
    assert_eq!(output.status.code(), Some(1), "{all}");
    assert!(all.contains(&config.display().to_string()), "{all}");
    assert!(!all.contains("MARKER-other-format"), "{all}");
    assert!(!world.cases().exists(), "no case was opened");
}

/// The files `examples/loom-agent/takt-agent.yaml` mounts at `/config/`, as `(name, content)`: its
/// `spec.configFiles`, verbatim.
fn example_config_files() -> Vec<(String, String)> {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/loom-agent/takt-agent.yaml");
    let text = fs::read_to_string(path).expect("the example");
    let agent: serde_yaml_ng::Value = serde_yaml_ng::from_str(&text).expect("YAML");
    agent["spec"]["configFiles"]
        .as_sequence()
        .expect("configFiles")
        .iter()
        .map(|file| {
            (
                file["name"].as_str().expect("a name").to_owned(),
                file["content"].as_str().expect("a content").to_owned(),
            )
        })
        .collect()
}

/// Acceptance 1 of story:one-run-file-for-host-and-loom-agent: the example's `takt.yaml` and
/// `llm.yaml`, read from a directory and from the same bytes in a second directory, the `/config/`
/// a LoomAgent mounts, each open a case whose `opened` line names the same llm digest and the same
/// endpoint. The route's key is a marker in the environment, which no case file may carry.
#[test]
fn one_file_host_and_mount_open_the_same_digest_and_endpoint() {
    let marker = "MARKER-one-file-key";
    let files = example_config_files();
    assert_eq!(
        files
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>(),
        ["takt.yaml", "llm.yaml"]
    );
    let llm = &files[1].1;
    let expected = takt::run_config::Routes::parse(llm)
        .expect("llm parses")
        .digest()
        .to_owned();

    let mut opened = Vec::new();
    for mounted in [false, true] {
        let world = World::new(if mounted {
            "one-file-mount"
        } else {
            "one-file-host"
        });
        let dir = if mounted {
            world.0.join("mount")
        } else {
            world.config_dir()
        };
        fs::create_dir_all(&dir).expect("a directory");
        for (name, content) in &files {
            fs::write(dir.join(name), content).expect("a config file");
        }
        let output = world.run_env(
            &dir.join("takt.yaml"),
            &["--iterations", "0"],
            &[("LLM_API_KEY", marker)],
        );
        let all = text(&output);
        assert_eq!(output.status.code(), Some(3), "{all}");
        assert!(!all.contains(marker), "{all}");
        assert!(!world.started().exists(), "b10x-harness was not started");
        let lines = world.opened();
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert_eq!(lines[0]["llm_digest"], expected.as_str(), "{lines:?}");
        opened.push(lines[0].clone());
        for file in files_under(&world.0) {
            let written = fs::read(&file).expect("a file the run wrote");
            assert!(
                !written
                    .windows(marker.len())
                    .any(|window| window == marker.as_bytes()),
                "{} holds the key",
                file.display()
            );
        }
    }
    assert_eq!(opened[0]["endpoint"], opened[1]["endpoint"]);
    assert_eq!(
        opened[0]["endpoint"]["base_url"],
        "https://llm.example.com/v1/"
    );
    assert_eq!(opened[0]["endpoint"]["wire"], "openai-responses");
    assert_eq!(opened[0]["endpoint"]["model"], "example-model");
}

/// Acceptance 3 of story:one-run-file-for-host-and-loom-agent, takt's side: a literal `auth.secret`
/// in the mounted `llm.yaml` exits 1 before any case opens, naming `auth.secret` and repeating the
/// literal in no output and in no file the run wrote. Loom's own `SecretLiteral` refusal of the
/// same file is not run here.
#[test]
fn one_file_literal_secret_in_llm_yaml_is_refused_before_any_case() {
    let world = World::new("one-file-literal");
    let literal = "MARKER-one-file-literal-key";
    for (name, content) in example_config_files() {
        let content = content.replace("secret: env:LLM_API_KEY", &format!("secret: {literal}"));
        world.config_named(&name, &content);
    }
    let config = world.config_dir().join("takt.yaml");
    let output = world.run(&config, &["--iterations", "0"]);
    let all = text(&output);
    assert_eq!(output.status.code(), Some(1), "{all}");
    assert!(all.contains("auth.secret"), "{all}");
    assert!(!all.contains(literal), "{all}");
    assert!(!world.cases().exists(), "no case was opened");
    for file in files_under(&world.0) {
        if file.starts_with(world.config_dir()) {
            continue;
        }
        let written = fs::read(&file).expect("a file the run wrote");
        assert!(
            !written
                .windows(literal.len())
                .any(|window| window == literal.as_bytes()),
            "{} holds the literal",
            file.display()
        );
    }
}
