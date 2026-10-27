//! story:model-names-an-llm-route: with an `llm` section and `--harness harness`, `--model` names a
//! route or alias of that document, and llm resolves it to the endpoint, wire and upstream model.
//! The refusals here exit 1 before any case opens: an unknown name, a route on a protocol takt does
//! not serve, an `llm` section under another harness, an endpoint source beside the section, and a
//! section with no `model`. The route's key is a file this test writes, whose marker must reach no
//! output. No refused run reaches a network, and HOME is a scratch directory.

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use takt::harness::{Credential, RouteEndpoint, RouteSecret};
use takt::run_config::Routes;

/// The key the route's file holds: a fixed marker, which no output or case file may carry.
const KEY: &str = "MARKER-route-file-key";

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
/// commit, an empty HOME, a `config` directory of run files and a key file of mode 0600.
struct World(PathBuf);

impl Drop for World {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

impl World {
    fn new(name: &str) -> Self {
        let outer = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
            "run-config-llm-route-{name}-{}",
            std::process::id()
        ));
        if outer.exists() {
            fs::remove_dir_all(&outer).expect("a stale scratch tree is removed");
        }
        let world = Self(outer);
        for dir in [world.repo(), world.home(), world.config_dir()] {
            fs::create_dir_all(dir).expect("a directory");
        }
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

    fn config_dir(&self) -> PathBuf {
        self.0.join("config")
    }

    fn cases(&self) -> PathBuf {
        self.repo().join(takt::case::CASES_DIR)
    }

    /// The absolute path of the key file, written once with [`KEY`] at mode 0600.
    fn key_file(&self) -> PathBuf {
        let path = self.0.join("route-key");
        fs::write(&path, KEY).expect("a key file");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).expect("mode 0600");
        path
    }

    /// Writes `text` as `config/<name>` and returns its path.
    fn file(&self, name: &str, text: &str) -> PathBuf {
        let path = self.config_dir().join(name);
        fs::write(&path, text).expect("a file beside the run file");
        path
    }

    /// `takt run` on the repository with an intent, `--config <config>` and `extra` arguments.
    fn run(&self, config: &Path, extra: &[&str]) -> Output {
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
            .args(extra)
            .env("HOME", self.home())
            .env("XDG_STATE_HOME", self.0.join("xdg-state"))
            .env_remove("DBUS_SESSION_BUS_ADDRESS")
            .env_remove("XDG_RUNTIME_DIR")
            .env_remove("B10X_GATES_POLICY")
            .output()
            .expect("takt runs")
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

/// The routes document, as JSON, which is YAML: an endpoint on the `responses` protocol whose key is
/// the file at `key`, and the route `small`, with the alias `fast`, on its upstream model.
fn document(key: &Path) -> serde_json::Value {
    serde_json::json!({
        "format": "llm.routes/1",
        "endpoints": {
            "e": {
                "base_url": "http://127.0.0.1:9/v1",
                "protocol": "responses",
                "auth": {"kind": "bearer", "secret": format!("file:{}", key.display())}
            }
        },
        "routes": {"small": {"model": "e/upstream-small", "aliases": ["fast"]}}
    })
}

/// A run file on `harness: harness` with `model` and the `llm` section `llm`.
fn llm_file(model: &str, llm: &serde_json::Value) -> String {
    format!("iterations: 0\nharness: harness\nforge: none\nmodel: {model}\nllm: {llm}\n")
}

/// The refusal of a run exits 1, names `needles` and opens no case, and the key's marker is in no
/// output or file the run wrote.
fn assert_refused(world: &World, output: &Output, needles: &[&str]) {
    let all = text(output);
    assert_eq!(output.status.code(), Some(1), "{all}");
    for needle in needles {
        assert!(all.contains(needle), "{needle} is named: {all}");
    }
    assert!(!all.contains(KEY), "{all}");
    assert!(!world.cases().exists(), "no case was opened");
    for file in files_under(&world.0) {
        if file == world.0.join("route-key") {
            continue;
        }
        let written = fs::read(&file).expect("a file the run wrote");
        assert!(
            !written
                .windows(KEY.len())
                .any(|window| window == KEY.as_bytes()),
            "{} holds the key",
            file.display()
        );
    }
}

/// Acceptance 2: a `model` that is no route, alias or `<endpoint>/<model-id>` exits 1 before any case
/// opens, with llm's own message.
#[test]
fn unknown_model_name_refused() {
    let world = World::new("unknown");
    let key = world.key_file();
    let config = world.file("run.yaml", &llm_file("nosuch", &document(&key)));
    let output = world.run(&config, &[]);
    assert_refused(&world, &output, &["nosuch", "neither a route nor"]);
}

/// Acceptance 3: a route on `chat-completions` exits 1 before any case opens, naming the route and the
/// protocol.
#[test]
fn chat_completions_route_refused() {
    let world = World::new("chat");
    let mut document = document(&world.key_file());
    document["endpoints"]["e"] = serde_json::json!({
        "base_url": "http://127.0.0.1:9/v1",
        "protocol": "chat-completions",
        "auth": {"kind": "anonymous"}
    });
    let config = world.file("run.yaml", &llm_file("small", &document));
    let output = world.run(&config, &[]);
    assert_refused(&world, &output, &["small", "chat-completions"]);
}

/// Acceptance 5: an `llm` section with `--harness codex` exits 1 before any case opens, naming codex.
#[test]
fn llm_section_with_codex_refused() {
    let world = World::new("codex");
    let config = world.file(
        "run.yaml",
        &format!(
            "iterations: 0\nharness: codex\nforge: none\nllm: {}\n",
            document(&world.key_file())
        ),
    );
    let output = world.run(&config, &[]);
    assert_refused(&world, &output, &["codex"]);
}

/// Acceptance 7, the run file's `endpoint_file`: an `llm` section beside it exits 1, naming both.
#[test]
fn llm_section_beside_file_endpoint_refused() {
    let world = World::new("file-key");
    world.file(
        "e.toml",
        "base_url = \"http://127.0.0.1:9/v1\"\nwire = \"openai-responses\"\n",
    );
    let config = world.file(
        "run.yaml",
        &format!(
            "endpoint_file: e.toml\n{}",
            llm_file("small", &document(&world.key_file()))
        ),
    );
    let output = world.run(&config, &[]);
    assert_refused(&world, &output, &["llm", "endpoint_file"]);
}

/// Acceptance 7, `--endpoint-file e.toml` on the command line beside the `llm` section.
#[test]
fn llm_section_beside_endpoint_file_flag_refused() {
    let world = World::new("flag-file");
    world.file(
        "e.toml",
        "base_url = \"http://127.0.0.1:9/v1\"\nwire = \"openai-responses\"\n",
    );
    let config = world.file("run.yaml", &llm_file("small", &document(&world.key_file())));
    let endpoint = world.config_dir().join("e.toml");
    let output = world.run(
        &config,
        &["--endpoint-file", endpoint.to_str().expect("utf-8")],
    );
    assert_refused(&world, &output, &["llm", "--endpoint-file"]);
}

/// Acceptance 7, `--endpoint-base-url <url>` on the command line beside the `llm` section.
#[test]
fn llm_section_beside_endpoint_base_url_flag_refused() {
    let world = World::new("flag-url");
    let config = world.file("run.yaml", &llm_file("small", &document(&world.key_file())));
    let output = world.run(&config, &["--endpoint-base-url", "http://127.0.0.1:9/v1"]);
    assert_refused(&world, &output, &["llm", "--endpoint-base-url"]);
}

/// An `llm` section with `harness: harness` and no `model`, from the flag or the file, exits 1: the
/// section names no route then.
#[test]
fn llm_section_without_model_refused() {
    let world = World::new("no-model");
    let config = world.file(
        "run.yaml",
        &format!(
            "iterations: 0\nharness: harness\nforge: none\nllm: {}\n",
            document(&world.key_file())
        ),
    );
    let output = world.run(&config, &[]);
    assert_refused(&world, &output, &["model"]);
}

/// Acceptance 1, at the function level: the alias `fast` of route `small` on endpoint `e` resolves to
/// `e`'s base URL, the `openai-responses` wire its `responses` protocol maps to, and the upstream
/// model, with the credential by reference: the key read from its file, never recorded.
#[test]
fn route_resolves_to_endpoint_values() {
    let world = World::new("resolve");
    let routes = Routes::parse(&document(&world.key_file()).to_string()).expect("llm parses");
    let route = RouteEndpoint::resolve(&routes, "fast", None).expect("the alias resolves");
    assert_eq!(
        route.config.base_url, "http://127.0.0.1:9/v1/",
        "llm writes the base URL with its trailing slash"
    );
    assert_eq!(route.config.wire, "openai-responses");
    assert_eq!(route.config.model, "upstream-small");
    assert_eq!(
        route.config.credential,
        Credential::Route {
            endpoint: "e".to_owned(),
            secret: RouteSecret::File,
        }
    );
    let recorded = serde_json::to_string(&route.config).expect("the endpoint records");
    assert!(!recorded.contains(KEY), "{recorded}");
    assert!(recorded.contains("\"kind\":\"route\""), "{recorded}");
    let direct =
        RouteEndpoint::resolve(&routes, "e/upstream-small", None).expect("a direct name resolves");
    assert_eq!(direct.config.model, "upstream-small");
}

/// A `messages` route's key is refused: a staged key is read by b10x-harness as a subscription token
/// on the Messages wire, so only the `openai-responses` wire takes one.
#[test]
fn messages_route_with_a_staged_key_refused() {
    let world = World::new("messages");
    let mut document = document(&world.key_file());
    document["endpoints"]["e"]["protocol"] = serde_json::json!("messages");
    let routes = Routes::parse(&document.to_string()).expect("llm parses");
    let error = RouteEndpoint::resolve(&routes, "small", None)
        .err()
        .expect("refused");
    let message = format!("{error:#}");
    assert!(message.contains("small"), "{message}");
    assert!(message.contains("anthropic-messages"), "{message}");
    assert!(!message.contains(KEY), "{message}");
}
