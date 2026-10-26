//! Adversarial cases for story:endpoint-credential-from-environment. Each drives `takt run
//! --harness harness --endpoint-token-env <NAME>` against a `b10x-harness` double under a scratch
//! HOME, as `endpoint_token_env_cli.rs` does, and asserts what the story promises where the
//! unit's own suite does not look: the key presented as the key it is on the Messages wire, the
//! value unreadable from takt's own process by a process takt starts, and the staged file gone
//! when the run is ended by a signal other than SIGKILL.

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

const VARIABLE: &str = "TAKT_ADV1_ENDPOINT_KEY";
const MARKER: &str = "MARKER-adv1-91c4e7-not-a-real-key";

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

struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// A repository whose test command is `probe`, a HOME holding a `b10x-harness` double whose
/// body is `double`, and a private runtime directory.
struct World {
    scratch: Scratch,
}

impl World {
    fn new(name: &str, probe: &str, double: &str) -> Self {
        let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("endpoint-env-adv1-{name}-{}", std::process::id()));
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
        fs::write(world.repo().join("probe.sh"), probe).expect("a probe");
        git(&world.repo(), &["init", "-q", "-b", "main"]);
        git(&world.repo(), &["add", "-A"]);
        git(&world.repo(), &["commit", "-q", "-m", "start"]);
        let program = world.home().join(".local/bin/b10x-harness");
        fs::create_dir_all(program.parent().expect("a parent")).expect("a program directory");
        fs::write(&program, double).expect("a double");
        fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).expect("exec bit");
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

    fn runtime(&self) -> PathBuf {
        self.outer().join("runtime")
    }

    fn command(&self, args: &[&str]) -> Command {
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
            .env(VARIABLE, MARKER);
        command
    }

    fn staged(&self) -> Vec<PathBuf> {
        fs::read_dir(self.runtime())
            .expect("the runtime directory")
            .flatten()
            .map(|entry| entry.path())
            .collect()
    }
}

fn text(output: &Output) -> String {
    format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// A double that records its arguments into `out` and fails, so the run ends at once.
fn recording_double(out: &Path) -> String {
    format!(
        "#!/bin/sh\nexec > '{}' 2>&1\nfor arg in \"$@\"; do echo \"$arg\"; done\nexit 1\n",
        out.display()
    )
}

/// The story's outcome is that the key "reaches b10x-harness". On the `anthropic-messages`
/// wire, the default one, a file handed as `--oauth-token-file` is a subscription token to
/// b10x-harness 0.20.0 (as 0.18.0): `Source::file` presents it as `Presentation::Subscription`, which the
/// Messages binding sends as `AuthKind::SubscriptionOauth` (an `Authorization: Bearer` with the
/// OAuth beta header and the client preamble) and bills as `Subscription`, not as the
/// `x-api-key` an API key is sent in (harness 0.20.0, `crates/harness-llm/src/lib.rs:395-404`).
/// An API key in an environment variable (a `LoomAgent`'s `spec.secretRef`) is an API key, so
/// on this wire it must not be handed as a subscription token.
///
/// Changed in the fix round by the dispatcher's decision F1: takt refuses an environment
/// credential on this wire before any case opens (exit 1, naming the variable and the wire and
/// nothing of the value), so the double must not run at all.
#[test]
fn an_api_key_on_the_messages_wire_is_not_handed_as_a_subscription_oauth_token() {
    let out_name = "args";
    let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("endpoint-env-adv1-messages-{}", std::process::id()));
    let world = World::new(
        "messages",
        "#!/bin/sh\nexit 0\n",
        &recording_double(&outer.join(out_name)),
    );
    let output = world
        .command(&[
            "--harness",
            "harness",
            "--endpoint-base-url",
            "https://endpoint.invalid/v1",
            "--endpoint-wire",
            "anthropic-messages",
            "--endpoint-token-env",
            VARIABLE,
            "--iterations",
            "3",
            "--test-cmd",
            "sh probe.sh",
        ])
        .output()
        .expect("takt runs");
    let said = text(&output);
    assert_eq!(output.status.code(), Some(1), "{said}");
    assert!(
        !outer.join(out_name).exists(),
        "an API key from {VARIABLE} on the anthropic-messages wire reached b10x-harness, which takes a token file as a subscription OAuth token:\n{said}"
    );
    assert!(
        said.contains(VARIABLE) && said.contains("anthropic-messages"),
        "the refusal names the variable and the wire:\n{said}"
    );
    assert!(!said.contains(MARKER), "{said}");
    assert!(
        !world.repo().join(".engineering/cases").exists(),
        "no case was opened"
    );
    assert!(world.staged().is_empty(), "nothing was staged");
}

/// The flag's help: "takt takes it out of its own environment before it starts anything, so no
/// process it starts sees it". `std::env::remove_var` edits libc's `environ` array; the
/// variable's bytes stay in takt's initial environment block, which `/proc/<takt>/environ`
/// reads back to any process of the same user, the test command takt starts among them. The
/// probe walks from its parent up and reads each ancestor's `/proc/<pid>/environ`.
/// Linux only: `/proc` and a test command allowed to write outside the tree.
#[cfg(target_os = "linux")]
#[test]
fn the_test_command_cannot_read_the_value_back_from_takts_proc_environ() {
    let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("endpoint-env-adv1-environ-{}", std::process::id()));
    let seen = outer.join("seen");
    let probe = format!(
        "#!/bin/sh\np=$PPID\nwhile [ \"$p\" -gt 1 ]; do\n  echo \"== $p $(cat /proc/$p/comm)\"\n  tr '\\0' '\\n' < /proc/$p/environ\n  p=$(sed 's/.*) //' /proc/$p/stat | cut -d' ' -f2)\ndone > '{}' 2>&1\nexit 0\n",
        seen.display()
    );
    let world = World::new("environ", &probe, &recording_double(&outer.join("args")));
    let output = world
        .command(&[
            "--harness",
            "harness",
            "--endpoint-base-url",
            "https://endpoint.invalid/v1",
            "--endpoint-wire",
            "openai-responses",
            "--endpoint-token-env",
            VARIABLE,
            "--iterations",
            "3",
            "--test-cmd",
            "sh probe.sh",
        ])
        .output()
        .expect("takt runs");
    let seen = fs::read_to_string(&seen)
        .unwrap_or_else(|error| panic!("the probe ran ({error}): {}", text(&output)));
    assert!(
        seen.contains("takt"),
        "the probe reached takt among its ancestors:\n{seen}"
    );
    let leaked: Vec<&str> = seen.lines().filter(|line| line.contains(MARKER)).collect();
    assert!(
        leaked.is_empty(),
        "the test command read the endpoint key back from an ancestor's /proc/<pid>/environ: {leaked:?}"
    );
}

/// Acceptance 3: the staged file is "removed with its directory when the run ends"; the story
/// leaves only SIGKILL out of scope. A run ended by SIGTERM (what a pod's stop or a supervisor
/// sends) or SIGINT (Ctrl-C) is not SIGKILL. The double sleeps, so the run is under way, holding
/// the staged file, when the signal arrives.
#[test]
fn a_run_ended_by_sigterm_or_sigint_removes_the_staged_token() {
    let mut left_behind = Vec::new();
    for signal in ["TERM", "INT"] {
        let outer = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
            "endpoint-env-adv1-sig{signal}-{}",
            std::process::id()
        ));
        let started = outer.join("started");
        let double = format!(
            "#!/bin/sh\necho $$ > '{}'\nexec sleep 30\n",
            started.display()
        );
        let world = World::new(&format!("sig{signal}"), "#!/bin/sh\nexit 0\n", &double);
        let mut child = world
            .command(&[
                "--harness",
                "harness",
                "--endpoint-base-url",
                "https://endpoint.invalid/v1",
                "--endpoint-wire",
                "openai-responses",
                "--endpoint-token-env",
                VARIABLE,
                "--iterations",
                "3",
                "--test-cmd",
                "sh probe.sh",
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("takt starts");
        let deadline = Instant::now() + Duration::from_secs(120);
        while fs::read_to_string(&started).map_or(true, |pid| pid.trim().is_empty()) {
            if Instant::now() > deadline || child.try_wait().expect("polls").is_some() {
                let _ = child.kill();
                panic!("b10x-harness never started under SIG{signal}");
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        assert_eq!(
            world.staged().len(),
            1,
            "the token is staged while the run is under way"
        );
        let status = Command::new("kill")
            .args([&format!("-{signal}"), &child.id().to_string()])
            .status()
            .expect("kill runs");
        assert!(status.success());
        let _ = child.wait();
        let double_pid = fs::read_to_string(&started).expect("the double's pid");
        let _ = Command::new("kill")
            .args(["-KILL", double_pid.trim()])
            .status();
        let left = world.staged();
        if !left.is_empty() {
            left_behind.push(format!("SIG{signal}: {left:?}"));
        }
    }
    assert!(
        left_behind.is_empty(),
        "a signal ended the run and the staged token is left behind: {left_behind:#?}"
    );
}

/// The harness session is a process takt starts (acceptance 4), and the flag's help says no
/// process takt starts sees the variable. b10x-harness runs in takt's own user namespace, so it
/// reads takt's initial environment block back through `/proc/<takt>/environ`, where
/// `remove_var` left the value. The double walks from its parent up, as the test command's
/// probe above does (which a user namespace of its own keeps out).
/// Linux only: `/proc`.
#[cfg(target_os = "linux")]
#[test]
fn the_harness_session_cannot_read_the_value_back_from_takts_proc_environ() {
    let outer = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "endpoint-env-adv1-harness-environ-{}",
        std::process::id()
    ));
    let seen = outer.join("seen");
    let double = format!(
        "#!/bin/sh\np=$PPID\nwhile [ \"$p\" -gt 1 ]; do\n  echo \"== $p $(cat /proc/$p/comm)\"\n  tr '\\0' '\\n' < /proc/$p/environ\n  p=$(sed 's/.*) //' /proc/$p/stat | cut -d' ' -f2)\ndone > '{}' 2>&1\nexit 1\n",
        seen.display()
    );
    let world = World::new("harness-environ", "#!/bin/sh\nexit 0\n", &double);
    let output = world
        .command(&[
            "--harness",
            "harness",
            "--endpoint-base-url",
            "https://endpoint.invalid/v1",
            "--endpoint-wire",
            "openai-responses",
            "--endpoint-token-env",
            VARIABLE,
            "--iterations",
            "3",
            "--test-cmd",
            "sh probe.sh",
        ])
        .output()
        .expect("takt runs");
    let seen = fs::read_to_string(&seen)
        .unwrap_or_else(|error| panic!("b10x-harness ran ({error}): {}", text(&output)));
    assert!(
        seen.contains("takt"),
        "the double reached takt among its ancestors:\n{seen}"
    );
    let leaked: Vec<&str> = seen.lines().filter(|line| line.contains(MARKER)).collect();
    assert!(
        leaked.is_empty(),
        "the harness session read the endpoint key back from an ancestor's /proc/<pid>/environ: {leaked:?}"
    );
}

/// The configuration route: a `loom.agent/1` document's `takt` member names the harness and an
/// endpoint file whose `[credential]` is `env`; no flag names either. The variable is taken out
/// before the test command starts, and the case records it by name.
/// Linux only: the probe writes outside the tree, which the macOS test sandbox refuses.
#[cfg(target_os = "linux")]
#[test]
fn a_loom_agent_document_names_the_variable_through_its_endpoint_file() {
    let outer = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "endpoint-env-adv1-loom-agent-{}",
        std::process::id()
    ));
    let seen = outer.join("seen");
    let world = World::new(
        "loom-agent",
        &format!("#!/bin/sh\nenv > '{}'\nexit 0\n", seen.display()),
        &recording_double(&outer.join("args")),
    );
    fs::write(
        world.outer().join("endpoint.toml"),
        format!(
            "base_url = \"https://endpoint.invalid/v1\"\nwire = \"openai-responses\"\nmodel = \"m\"\n\n[credential]\nenv = \"{VARIABLE}\"\n"
        ),
    )
    .expect("an endpoint file");
    let agent = world.outer().join("agent.yaml");
    fs::write(
        &agent,
        "format: loom.agent/1\nagent: {id: a, name: A, brief: b}\ntakt:\n  harness: harness\n  endpoint_file: endpoint.toml\n  iterations: 3\n  test_cmd: sh probe.sh\n",
    )
    .expect("an agent document");
    let output = world
        .command(&["--config", agent.to_str().expect("utf-8")])
        .output()
        .expect("takt runs");
    let seen = fs::read_to_string(&seen)
        .unwrap_or_else(|error| panic!("the test command ran ({error}): {}", text(&output)));
    assert!(
        !seen.contains(VARIABLE) && !seen.contains(MARKER),
        "the test command saw the variable named by the agent document's endpoint file:\n{seen}"
    );
    let args = fs::read_to_string(outer.join("args"))
        .unwrap_or_else(|error| panic!("b10x-harness ran ({error}): {}", text(&output)));
    assert!(
        args.lines().any(|line| line.contains("/takt-endpoint-")),
        "the staged file reached b10x-harness:\n{args}"
    );
    assert!(world.staged().is_empty(), "the staged directory is gone");
    assert!(!text(&output).contains(MARKER), "{}", text(&output));
}
