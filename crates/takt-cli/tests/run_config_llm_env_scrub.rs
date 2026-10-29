//! A route's key read from an `env:` reference reaches no process takt starts: not the test command,
//! not the b10x-harness double, not an argument or a case file, and the runtime directory the staged
//! token lived in is empty after the run. The key is a placeholder this test writes; HOME, the
//! runtime directory and the double are scratch directories, and no run reaches a network.

#[path = "../../takt/tests/support/own_home.rs"]
mod own_home;

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// The variable the route's endpoint names, and a second variable holding the same value.
const VARIABLE: &str = "PROBE_ROUTE_KEY";
const COPY: &str = "PROBE_ROUTE_COPY";
/// The placeholder key, which no output, environment, argument or case file may carry.
const VALUE: &str = "PLACEHOLDER-ROUTE-KEY-VALUE";
/// A variable another endpoint of the document names, which no route reaches, and its placeholder.
const OTHER: &str = "PROBE_OTHER";
const OTHER_VALUE: &str = "PLACEHOLDER-OTHER-KEY-VALUE";

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
/// commit and a test command that records its environment, a HOME whose `.local/bin` holds a
/// b10x-harness double that records its environment, its arguments and the token it is handed,
/// a runtime directory of mode 0700, and a run file beside the repository.
struct World(PathBuf);

impl Drop for World {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

impl World {
    fn new(name: &str) -> Self {
        let outer = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
            "run-config-llm-env-scrub-{name}-{}",
            std::process::id()
        ));
        if outer.exists() {
            fs::remove_dir_all(&outer).expect("a stale scratch tree is removed");
        }
        let world = Self(outer);
        for dir in [
            world.repo(),
            world.home(),
            world.runtime(),
            world.config_dir(),
        ] {
            fs::create_dir_all(&dir).expect("a directory");
        }
        fs::set_permissions(world.runtime(), fs::Permissions::from_mode(0o700))
            .expect("a private runtime directory");
        fs::write(world.repo().join("README.md"), "a story\n").expect("a file");
        fs::write(
            world.repo().join("probe.sh"),
            format!(
                "#!/bin/sh\nenv > '{}' 2>&1\nexit 0\n",
                world.seen().display()
            ),
        )
        .expect("a probe");
        git(&world.repo(), &["init", "-q", "-b", "main"]);
        git(&world.repo(), &["add", "-A"]);
        git(&world.repo(), &["commit", "-q", "-m", "start"]);
        let double = world.home().join(".local/bin/b10x-harness");
        fs::create_dir_all(double.parent().expect("a parent")).expect("a program directory");
        fs::write(
            &double,
            format!(
                "#!/bin/sh\nexec > '{started}' 2>&1\nenv\necho '-- args'\nfor arg in \"$@\"; do echo \"$arg\"; done\necho '-- staged'\ncat '{runtime}'/*/token\necho\necho '-- end'\nexit 1\n",
                started = world.started().display(),
                runtime = world.runtime().display(),
            ),
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

    fn config_dir(&self) -> PathBuf {
        self.0.join("config")
    }

    /// The runtime directory takt is given as `XDG_RUNTIME_DIR`, its first staging candidate.
    fn runtime(&self) -> PathBuf {
        self.0.join("runtime")
    }

    /// What the test command saw, in the repository's git directory, out of the work tree.
    fn seen(&self) -> PathBuf {
        self.repo().join(".git/seen")
    }

    fn started(&self) -> PathBuf {
        self.0.join("started")
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

    /// `takt run` on the repository with `config`, `args` after it, HOME and the runtime directory
    /// the scratch ones, no session bus, and each of `vars` set to its value.
    fn run(&self, config: &Path, args: &[&str], vars: &[(&str, &str)]) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_takt"));
        command
            .args([
                "run",
                "--repo",
                self.repo().to_str().expect("utf-8"),
                "--intent",
                "probe",
                "--config",
                config.to_str().expect("utf-8"),
                "--harness",
                "harness",
                "--iterations",
                "5",
            ])
            .args(args)
            .env("HOME", self.home())
            .env("XDG_STATE_HOME", self.0.join("xdg-state"))
            .env("XDG_RUNTIME_DIR", self.runtime())
            .env_remove("DBUS_SESSION_BUS_ADDRESS")
            // A harness run opens only under a policy or a declared `scan: none`; no step here
            // reaches a scan.
            .env("B10X_GATES_POLICY", self.0.join("gates-policy.json"))
            .env_remove("ANTHROPIC_API_KEY")
            .env_remove(VARIABLE)
            .env_remove(COPY)
            .env_remove(OTHER);
        for (name, value) in vars {
            command.env(name, value);
        }
        command.output().expect("takt runs")
    }

    /// The runtime directory's entries.
    fn staged(&self) -> Vec<PathBuf> {
        fs::read_dir(self.runtime())
            .expect("the runtime directory")
            .flatten()
            .map(|entry| entry.path())
            .collect()
    }
}

/// The routes document, as JSON, which is YAML: endpoint `e` on the `responses` protocol, its key
/// the variable [`VARIABLE`], and the route `small` on its upstream model. With `second`, a further
/// endpoint `o` whose key is the variable [`OTHER`], which no route of the document reaches.
fn document(second: bool) -> serde_json::Value {
    let mut document = serde_json::json!({
        "format": "llm.routes/1",
        "endpoints": {
            "e": {
                "base_url": "http://127.0.0.1:9/v1",
                "protocol": "responses",
                "auth": {"kind": "bearer", "secret": format!("env:{VARIABLE}")}
            }
        },
        "routes": {"small": {"model": "e/upstream-small"}}
    });
    if second {
        document["endpoints"]["o"] = serde_json::json!({
            "base_url": "http://127.0.0.1:9/v2",
            "protocol": "responses",
            "auth": {"kind": "bearer", "secret": format!("env:{OTHER}")}
        });
    }
    document
}

/// A run file on the route `model` and the `llm` section `llm`.
fn llm_file(model: &str, llm: &serde_json::Value) -> String {
    format!("forge: none\nmodel: {model}\nllm: {llm}\n")
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

/// A route whose variable is set but empty is refused before any case opens, naming the route and
/// its credential's source, not the endpoint's name as a credential variable's.
#[test]
fn an_empty_route_variable_is_refused_naming_the_route_credential() {
    let world = World::new("empty-route");
    let config = world.file("run.yaml", &llm_file("small", &document(false)));
    let output = world.run(&config, &[], &[(VARIABLE, "")]);
    let (stdout, stderr) = text(&output);
    assert_eq!(output.status.code(), Some(1), "{stdout}\n{stderr}");
    assert!(!world.cases().exists(), "no case was opened: {stderr}");
    assert!(stderr.contains("the route small"), "{stderr}");
    assert!(stderr.contains("from environment"), "{stderr}");
    assert!(stderr.contains("is empty"), "{stderr}");
    assert!(!stderr.contains("credential variable"), "{stderr}");
}

/// The route's key, read from its `env:` reference, reaches no child: the test command saw neither
/// variable nor the value, the double saw neither, no argument holds the value, the double is
/// handed a token file under the runtime directory that holds it, no case file holds it, and the
/// runtime directory is empty after the run. The double names no action, so the run ends on the
/// error, exit 5.
#[test]
fn route_env_key_reaches_no_child_and_the_staged_file_is_removed() {
    let world = World::new("route");
    let config = world.file("run.yaml", &llm_file("small", &document(false)));
    let output = world.run(
        &config,
        &["--test-cmd", "sh probe.sh"],
        &[(VARIABLE, VALUE), (COPY, VALUE)],
    );
    let (stdout, stderr) = text(&output);
    let seen = fs::read_to_string(world.seen())
        .unwrap_or_else(|error| panic!("the probe ran ({error}): {stdout}\n{stderr}"));
    assert!(
        !seen.contains(VARIABLE) && !seen.contains(COPY) && !seen.contains(VALUE),
        "the test command saw the key:\n{seen}"
    );
    let harness = fs::read_to_string(world.started())
        .unwrap_or_else(|error| panic!("b10x-harness ran ({error}): {stdout}\n{stderr}"));
    let (environment, rest) = harness
        .split_once("-- args\n")
        .expect("the double listed its arguments");
    let (args, staged) = rest
        .split_once("-- staged\n")
        .expect("the double read the runtime directory");
    assert!(
        !environment.contains(VARIABLE)
            && !environment.contains(COPY)
            && !environment.contains(VALUE),
        "b10x-harness saw the key:\n{environment}"
    );
    let runtime = world.runtime().display().to_string();
    let token_file = args
        .lines()
        .skip_while(|line| *line != "--oauth-token-file")
        .nth(1)
        .unwrap_or_else(|| panic!("b10x-harness was handed a token file:\n{args}"));
    assert!(
        token_file.starts_with(&format!("{runtime}/takt-endpoint-")),
        "the token file is staged under the runtime directory: {token_file}"
    );
    assert!(!args.contains(VALUE), "the key is no argument:\n{args}");
    assert_eq!(
        staged.lines().next(),
        Some(VALUE),
        "the staged file holds the key"
    );
    assert_eq!(
        output.status.code(),
        Some(5),
        "a harness step without an action ends the run on the error, exit 5:\n{stdout}\n{stderr}"
    );
    assert!(
        !stdout.contains(VALUE) && !stderr.contains(VALUE),
        "{stdout}\n{stderr}"
    );
    for path in files_under(&world.cases()) {
        let bytes = fs::read(&path).expect("a case file");
        assert!(
            !String::from_utf8_lossy(&bytes).contains(VALUE),
            "the key reached {}",
            path.display()
        );
    }
    let opened = opened_lines(&world.cases());
    assert_eq!(
        opened[0]["endpoint"]["credential"],
        serde_json::json!({"kind": "route", "endpoint": "e", "secret": "environment"})
    );
    assert_eq!(
        world.staged(),
        Vec::<PathBuf>::new(),
        "the runtime directory is empty after the run"
    );
}

/// The document names a second endpoint's variable, which no route of it reaches and takt never
/// resolves: that variable reaches no child either, and neither does the chosen route's key.
#[test]
fn a_variable_another_endpoint_names_reaches_no_child() {
    let world = World::new("other-endpoint");
    let config = world.file("run.yaml", &llm_file("small", &document(true)));
    let output = world.run(
        &config,
        &["--test-cmd", "sh probe.sh"],
        &[(VARIABLE, VALUE), (OTHER, OTHER_VALUE)],
    );
    let (stdout, stderr) = text(&output);
    let seen = fs::read_to_string(world.seen())
        .unwrap_or_else(|error| panic!("the probe ran ({error}): {stdout}\n{stderr}"));
    for secret in [VARIABLE, VALUE, OTHER, OTHER_VALUE] {
        assert!(
            !seen.contains(secret),
            "the test command saw {secret}:\n{seen}"
        );
    }
    let harness = fs::read_to_string(world.started())
        .unwrap_or_else(|error| panic!("b10x-harness ran ({error}): {stdout}\n{stderr}"));
    let (environment, _) = harness
        .split_once("-- args\n")
        .expect("the double listed its arguments");
    for secret in [VARIABLE, VALUE, OTHER, OTHER_VALUE] {
        assert!(
            !environment.contains(secret),
            "b10x-harness saw {secret}:\n{environment}"
        );
    }
    assert!(
        !stdout.contains(OTHER_VALUE) && !stderr.contains(OTHER_VALUE),
        "{stdout}\n{stderr}"
    );
}
