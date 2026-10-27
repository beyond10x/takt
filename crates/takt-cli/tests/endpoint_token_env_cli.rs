//! story:endpoint-credential-from-environment: `takt run --harness harness` takes the endpoint's
//! credential from an environment variable it names (`--endpoint-token-env <NAME>`, or the
//! endpoint file's `[credential] env`), stages its value as a protected token file under the
//! runtime directory, hands that file to the session, and lets the variable reach no process it
//! starts. These runs reach no network and no real credential: the value is a marker, HOME and
//! the runtime directory are scratch directories, and a `b10x-harness` double under HOME's
//! `.local/bin`, where the session executor finds it, records its environment and arguments
//! and reads the staged token back.

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const VARIABLE: &str = "TAKT_TEST_ENDPOINT_KEY";
const MARKER: &str = "MARKER-5d27c1-not-a-real-key";

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

/// A scratch world: a repository with one commit and a test command that records its own
/// environment, a HOME holding only a `b10x-harness` double that records its environment, its
/// arguments and the token files staged under the runtime directory, and a runtime directory of
/// mode 0700.
struct World {
    scratch: Scratch,
}

impl World {
    fn new(name: &str) -> Self {
        let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("endpoint-env-{name}-{}", std::process::id()));
        if outer.exists() {
            fs::remove_dir_all(&outer).expect("a stale scratch tree is removed");
        }
        let world = Self {
            scratch: Scratch(outer),
        };
        for dir in [world.repo(), world.home(), world.runtime()] {
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
        // Where the session executor finds it: the PATH metaharness gives its child starts at
        // `$HOME/.local/bin`.
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

    fn outer(&self) -> &Path {
        &self.scratch.0
    }

    fn repo(&self) -> PathBuf {
        self.outer().join("repo")
    }

    fn home(&self) -> PathBuf {
        self.outer().join("home")
    }

    /// The runtime directory takt is given as `XDG_RUNTIME_DIR`, its first staging candidate.
    fn runtime(&self) -> PathBuf {
        self.outer().join("runtime")
    }

    /// What the test command saw, in the repository's git directory, out of the work tree.
    fn seen(&self) -> PathBuf {
        self.repo().join(".git/seen")
    }

    fn started(&self) -> PathBuf {
        self.outer().join("started")
    }

    fn cases(&self) -> PathBuf {
        self.repo().join(".engineering/cases")
    }

    /// `takt run` on the repository with `args` after the intent, HOME the scratch one, the
    /// runtime directory the scratch one, no session bus, and the variable set to `value` when
    /// given.
    fn run(&self, value: Option<&str>, args: &[&str]) -> Output {
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
            ])
            .args(args)
            .env("HOME", self.home())
            .env("XDG_STATE_HOME", self.outer().join("xdg-state"))
            .env("XDG_RUNTIME_DIR", self.runtime())
            .env_remove("DBUS_SESSION_BUS_ADDRESS")
            .env_remove("B10X_GATES_POLICY")
            .env_remove("ANTHROPIC_API_KEY")
            .env_remove(VARIABLE);
        if let Some(value) = value {
            command.env(VARIABLE, value);
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

/// Asserts `output` is a refusal before any case: exit 1, no case opened, b10x-harness not
/// started, nothing staged, every one of `parts` in what it said and the marker in none of it.
fn refused_before_any_case(world: &World, output: &Output, parts: &[&str]) {
    let (stdout, stderr) = text(output);
    assert_eq!(output.status.code(), Some(1), "{stdout}\n{stderr}");
    for part in parts {
        assert!(stderr.contains(part), "{part}: {stderr}");
    }
    assert!(
        !stdout.contains(MARKER) && !stderr.contains(MARKER),
        "the value leaked: {stdout}\n{stderr}"
    );
    assert!(!world.cases().exists(), "no case was opened");
    assert!(!world.started().exists(), "b10x-harness was not started");
    assert_eq!(
        world.staged(),
        Vec::<PathBuf>::new(),
        "nothing is left staged"
    );
}

#[test]
fn run_help_names_the_token_env_flag() {
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args(["run", "--help"])
        .output()
        .expect("takt runs");
    let (help, _) = text(&output);
    assert_eq!(output.status.code(), Some(0), "{help}");
    assert!(help.contains("--endpoint-token-env"), "{help}");
}

/// Acceptance 1: the flag with the token file or the secret is refused before any case.
#[test]
fn the_token_env_with_another_credential_is_refused_before_any_case() {
    for (name, other, value) in [
        (
            "env-file",
            "--endpoint-token-file",
            "/nonexistent/token.json",
        ),
        ("env-secret", "--endpoint-secret", "llm/key"),
    ] {
        let world = World::new(name);
        let output = world.run(
            Some(MARKER),
            &[
                "--harness",
                "harness",
                "--endpoint-token-env",
                VARIABLE,
                other,
                value,
            ],
        );
        refused_before_any_case(&world, &output, &["--endpoint-token-env", other]);
    }
}

/// Acceptance 1: a name that is no portable variable name is refused before any case.
#[test]
fn a_name_that_is_no_variable_name_is_refused_before_any_case() {
    for (index, name) in ["1KEY", "A-KEY", "A=B", ""].into_iter().enumerate() {
        let world = World::new(&format!("bad-name-{index}"));
        let output = world.run(
            Some(MARKER),
            &[
                "--harness",
                "harness",
                "--endpoint-base-url",
                "https://endpoint.invalid/v1",
                "--endpoint-wire",
                "openai-responses",
                "--endpoint-token-env",
                name,
            ],
        );
        refused_before_any_case(&world, &output, &["no environment variable's name"]);
    }
}

/// Acceptance 2: an unset or an empty variable is refused before any case, naming it.
#[test]
fn an_unset_or_empty_variable_is_refused_before_any_case_naming_it() {
    for (name, value, why) in [
        ("unset", None, "is not set"),
        ("empty", Some(""), "is empty"),
    ] {
        let world = World::new(name);
        let output = world.run(
            value,
            &[
                "--harness",
                "harness",
                "--endpoint-base-url",
                "https://endpoint.invalid/v1",
                "--endpoint-wire",
                "openai-responses",
                "--endpoint-token-env",
                VARIABLE,
            ],
        );
        refused_before_any_case(&world, &output, &[VARIABLE, why]);
    }
}

/// An environment credential on the `anthropic-messages` wire, given or by default, by flag or
/// by the endpoint file, is refused before any case, naming the variable and the wire: the
/// session executor hands the staged file to b10x-harness as a subscription OAuth token file.
#[test]
fn the_token_env_on_the_messages_wire_is_refused_before_any_case() {
    let world = World::new("messages-flag");
    let output = world.run(
        Some(MARKER),
        &[
            "--harness",
            "harness",
            "--endpoint-base-url",
            "https://endpoint.invalid/v1",
            "--endpoint-token-env",
            VARIABLE,
        ],
    );
    refused_before_any_case(
        &world,
        &output,
        &[VARIABLE, "`anthropic-messages`", "only openai-responses"],
    );
    let world = World::new("messages-file");
    let file = world.outer().join("endpoint.toml");
    fs::write(
        &file,
        format!(
            "base_url = \"https://endpoint.invalid/v1\"\nwire = \"anthropic-messages\"\nmodel = \"m\"\n\n[credential]\nenv = \"{VARIABLE}\"\n"
        ),
    )
    .expect("an endpoint file");
    let output = world.run(
        Some(MARKER),
        &[
            "--harness",
            "harness",
            "--endpoint-file",
            file.to_str().expect("utf-8"),
        ],
    );
    refused_before_any_case(
        &world,
        &output,
        &[VARIABLE, "`anthropic-messages`", "only openai-responses"],
    );
}

/// Acceptance 3 and 5, from the endpoint file's `[credential] env`: the case opens on
/// `--iterations 0`, its `opened` line records the credential as `kind: environment` with the
/// variable's name, nothing of the value reaches the case or the output, and the staged
/// directory is gone once the run has ended.
#[test]
fn the_endpoint_file_names_the_variable_and_the_case_records_its_name() {
    let world = World::new("file");
    let file = world.outer().join("endpoint.toml");
    fs::write(
        &file,
        format!(
            "base_url = \"https://endpoint.invalid/v1\"\nwire = \"openai-responses\"\nmodel = \"m\"\n\n[credential]\nenv = \"{VARIABLE}\"\n"
        ),
    )
    .expect("an endpoint file");
    let output = world.run(
        Some(MARKER),
        &[
            "--harness",
            "harness",
            "--endpoint-file",
            file.to_str().expect("utf-8"),
            "--iterations",
            "0",
        ],
    );
    let (stdout, stderr) = text(&output);
    assert_eq!(output.status.code(), Some(3), "{stdout}\n{stderr}");
    assert!(
        !stdout.contains(MARKER) && !stderr.contains(MARKER),
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
            "credential": {"kind": "environment", "name": VARIABLE},
        })
    );
    for path in files_under(&world.cases()) {
        let bytes = fs::read(&path).expect("a case file");
        assert!(
            !String::from_utf8_lossy(&bytes).contains(MARKER),
            "the value reached {}",
            path.display()
        );
    }
    assert_eq!(
        world.staged(),
        Vec::<PathBuf>::new(),
        "the staged directory is gone"
    );
}

/// Acceptance 3, 4 and 5 by flag through two steps: the test command (step 1) sees neither the
/// variable nor its value; b10x-harness (step 2) sees neither in its environment and is handed
/// the staged file under the runtime directory, which holds the value; no case file and no
/// output holds the value; and once the run has ended, on the harness's error, the staged
/// directory is gone. The double names no action, so the step is tried
/// [`takt::run::NO_ACTION_TRIES`] times (steps 2 to 4) before the run ends on the error.
#[test]
fn the_variable_reaches_no_child_and_the_staged_file_is_removed_after_the_run() {
    let world = World::new("flag");
    let output = world.run(
        Some(MARKER),
        &[
            "--harness",
            "harness",
            "--endpoint-base-url",
            "https://endpoint.invalid/v1",
            "--endpoint-wire",
            "openai-responses",
            "--endpoint-token-env",
            VARIABLE,
            "--iterations",
            "4",
            "--test-cmd",
            "sh probe.sh",
        ],
    );
    let (stdout, stderr) = text(&output);
    let seen = fs::read_to_string(world.seen())
        .unwrap_or_else(|error| panic!("the probe ran ({error}): {stdout}\n{stderr}"));
    assert!(
        !seen.contains(VARIABLE) && !seen.contains(MARKER),
        "the test command saw the variable:\n{seen}"
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
        !environment.contains(VARIABLE) && !environment.contains(MARKER),
        "b10x-harness saw the variable:\n{environment}"
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
    assert!(!args.contains(MARKER), "the value is no argument:\n{args}");
    assert_eq!(
        staged.lines().next(),
        Some(MARKER),
        "the staged file holds the value"
    );
    assert_eq!(output.status.code(), Some(5), "{stdout}\n{stderr}");
    assert!(
        !stdout.contains(MARKER) && !stderr.contains(MARKER),
        "{stdout}\n{stderr}"
    );
    for path in files_under(&world.cases()) {
        let bytes = fs::read(&path).expect("a case file");
        assert!(
            !String::from_utf8_lossy(&bytes).contains(MARKER),
            "the value reached {}",
            path.display()
        );
    }
    let opened = opened_lines(&world.cases());
    assert_eq!(
        opened[0]["endpoint"]["credential"],
        serde_json::json!({"kind": "environment", "name": VARIABLE})
    );
    assert_eq!(
        world.staged(),
        Vec::<PathBuf>::new(),
        "the staged directory is gone"
    );
}

// story:run-configuration-names-the-story-and-token-variable: the run configuration file's
// `endpoint_token_env` names the variable as `--endpoint-token-env` does.

/// A run configuration file in the world, out of its repository, holding `text`.
fn config(world: &World, text: &str) -> String {
    let path = world.outer().join("run.toml");
    fs::write(&path, text).expect("a run configuration file");
    path.to_str().expect("utf-8").to_owned()
}

/// The endpoint flags of these runs but the credential's.
const ENDPOINT: [&str; 6] = [
    "--harness",
    "harness",
    "--endpoint-base-url",
    "https://endpoint.invalid/v1",
    "--endpoint-wire",
    "openai-responses",
];

/// Acceptance 2: `endpoint_token_env` in `--config`'s file stages the variable as the flag
/// does: the test command and b10x-harness see neither the variable nor its value, b10x-harness
/// is handed the staged file, no case file and no output holds the value, the `opened` line
/// names the variable, and the staged directory is gone once the run has ended.
#[test]
fn the_configurations_token_variable_is_staged_as_the_flags_is() {
    let world = World::new("config-env");
    let config = config(&world, &format!("endpoint_token_env = \"{VARIABLE}\"\n"));
    let mut args = ENDPOINT.to_vec();
    args.extend([
        "--config",
        &config,
        "--iterations",
        "3",
        "--test-cmd",
        "sh probe.sh",
    ]);
    let output = world.run(Some(MARKER), &args);
    let (stdout, stderr) = text(&output);
    let seen = fs::read_to_string(world.seen())
        .unwrap_or_else(|error| panic!("the probe ran ({error}): {stdout}\n{stderr}"));
    assert!(
        !seen.contains(VARIABLE) && !seen.contains(MARKER),
        "the test command saw the variable:\n{seen}"
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
        !environment.contains(VARIABLE) && !environment.contains(MARKER),
        "b10x-harness saw the variable:\n{environment}"
    );
    assert!(
        args.contains("--oauth-token-file"),
        "b10x-harness was handed a token file:\n{args}"
    );
    assert!(!args.contains(MARKER), "the value is no argument:\n{args}");
    assert_eq!(
        staged.lines().next(),
        Some(MARKER),
        "the staged file holds the value"
    );
    assert!(
        !stdout.contains(MARKER) && !stderr.contains(MARKER),
        "{stdout}\n{stderr}"
    );
    for path in files_under(&world.cases()) {
        let bytes = fs::read(&path).expect("a case file");
        assert!(
            !String::from_utf8_lossy(&bytes).contains(MARKER),
            "the value reached {}",
            path.display()
        );
    }
    assert_eq!(
        opened_lines(&world.cases())[0]["endpoint"]["credential"],
        serde_json::json!({"kind": "environment", "name": VARIABLE})
    );
    assert_eq!(
        world.staged(),
        Vec::<PathBuf>::new(),
        "the staged directory is gone"
    );
}

/// Acceptance 2: the file's variable is refused as the flag's is: unset, before any case,
/// naming it; and a name that is no variable's.
#[test]
fn the_configurations_token_variable_is_refused_as_the_flags_is() {
    let world = World::new("config-env-unset");
    let config_path = config(&world, &format!("endpoint_token_env = \"{VARIABLE}\"\n"));
    let mut args = ENDPOINT.to_vec();
    args.extend(["--config", &config_path]);
    let output = world.run(None, &args);
    refused_before_any_case(&world, &output, &[VARIABLE, "is not set"]);

    let world = World::new("config-env-bad-name");
    let config_path = config(&world, "endpoint_token_env = \"A-KEY\"\n");
    let mut args = ENDPOINT.to_vec();
    args.extend(["--config", &config_path]);
    let output = world.run(Some(MARKER), &args);
    refused_before_any_case(&world, &output, &["no environment variable's name"]);
}

/// An `--endpoint-file` on the command line that names no credential names no credential
/// source: the configuration file's variable is the credential.
#[test]
fn a_command_line_endpoint_file_without_a_credential_takes_the_configurations_variable() {
    let world = World::new("config-env-bare-endpoint");
    let endpoint = world.outer().join("endpoint.toml");
    fs::write(
        &endpoint,
        "base_url = \"https://endpoint.invalid/v1\"\nwire = \"openai-responses\"\nmodel = \"m\"\n",
    )
    .expect("an endpoint file");
    let config_path = config(
        &world,
        &format!("harness = \"harness\"\nendpoint_token_env = \"{VARIABLE}\"\n"),
    );
    let output = world.run(
        Some(MARKER),
        &[
            "--config",
            &config_path,
            "--endpoint-file",
            endpoint.to_str().expect("utf-8"),
            "--iterations",
            "0",
        ],
    );
    let (stdout, stderr) = text(&output);
    assert_eq!(output.status.code(), Some(3), "{stdout}\n{stderr}");
    assert_eq!(
        opened_lines(&world.cases())[0]["endpoint"]["credential"],
        serde_json::json!({"kind": "environment", "name": VARIABLE})
    );
}

/// Acceptance 2: a credential flag on the command line wins over the file's variable: the
/// flag's variable is the one named, and a secret flag is taken where the file's variable would
/// have been refused beside it.
#[test]
fn a_credential_flag_wins_over_the_configurations_token_variable() {
    let world = World::new("config-env-flag");
    let config_path = config(&world, "endpoint_token_env = \"TAKT_TEST_UNSET_OTHER\"\n");
    let mut args = ENDPOINT.to_vec();
    args.extend([
        "--config",
        &config_path,
        "--endpoint-token-env",
        VARIABLE,
        "--iterations",
        "0",
    ]);
    let output = world.run(Some(MARKER), &args);
    let (stdout, stderr) = text(&output);
    assert_eq!(output.status.code(), Some(3), "{stdout}\n{stderr}");
    assert_eq!(
        opened_lines(&world.cases())[0]["endpoint"]["credential"],
        serde_json::json!({"kind": "environment", "name": VARIABLE})
    );

    let world = World::new("config-env-secret");
    let config_path = config(&world, &format!("endpoint_token_env = \"{VARIABLE}\"\n"));
    let mut args = ENDPOINT.to_vec();
    args.extend([
        "--config",
        &config_path,
        "--endpoint-secret",
        "llm/key",
        "--iterations",
        "0",
    ]);
    let output = world.run(Some(MARKER), &args);
    let (stdout, stderr) = text(&output);
    assert_eq!(output.status.code(), Some(3), "{stdout}\n{stderr}");
    assert_eq!(
        opened_lines(&world.cases())[0]["endpoint"]["credential"],
        serde_json::json!({"kind": "secret", "namespace": "llm", "name": "key"})
    );
}
