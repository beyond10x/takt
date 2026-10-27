//! story:declared-setup-runs-before-the-first-check, in the library: the commands a run file's
//! `setup` lists run once when the case opens, before the red-base check and before any model
//! call, each as one process whose arguments no shell reads; their durations are recorded and
//! their output kept under the state directory; one that does not end 0 stops the run
//! `SetupFailed`; a resumed case runs them again only when its base moved. The model is a
//! double that counts the steps it is started for.

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use loom_sdk::session::CredentialSource;
use takt::harness::ClaudeSession;
use takt::run_config::{RunConfig, SetupCommand};
use takt::validator::{TestsRun, Validator};
use takt::{Run, RunOptions, Stop};

fn git(tree: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(args)
        .current_dir(tree)
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct World {
    _scratch: Scratch,
    tree: PathBuf,
    elsewhere: PathBuf,
}

impl World {
    fn calls(&self) -> Vec<String> {
        fs::read_to_string(self.elsewhere.join("calls"))
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect()
    }

    fn steps(&self) -> usize {
        self.calls().iter().filter(|call| *call == "step").count()
    }

    fn state(&self) -> PathBuf {
        self.elsewhere.join("state")
    }

    /// The lines of the case's run record, as values.
    fn events(&self, case: &str) -> Vec<serde_json::Value> {
        fs::read_to_string(
            self.tree
                .join(".takt/cases")
                .join(case)
                .join("events.jsonl"),
        )
        .unwrap_or_default()
        .lines()
        .map(|line| serde_json::from_str(line).expect("a line"))
        .collect()
    }

    fn case_dir(&self, case: &str) -> PathBuf {
        self.tree.join(".takt/cases").join(case)
    }
}

/// A repository of two commits, and a directory `elsewhere` beside it holding the double `claude`.
fn world(name: &str) -> World {
    let dir =
        Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("setup-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let tree = dir.join("tree");
    let elsewhere = dir.join("elsewhere");
    fs::create_dir_all(&tree).expect("the tree");
    fs::create_dir_all(&elsewhere).expect("a directory");
    fs::write(tree.join("README.md"), "a story\n").expect("a file");
    git(&tree, &["init", "-q", "-b", "main"]);
    git(&tree, &["add", "-A"]);
    git(&tree, &["commit", "-q", "-m", "start"]);
    fs::write(tree.join("NOTES.md"), "second\n").expect("a file");
    git(&tree, &["add", "-A"]);
    git(&tree, &["commit", "-q", "-m", "second"]);
    World {
        _scratch: Scratch(dir),
        tree,
        elsewhere,
    }
}

/// A `claude` that answers `--version` and, started as a session, appends `step` to `calls`,
/// changes nothing and ends on a result naming `tests.write`.
fn fake_claude(world: &World) -> PathBuf {
    let at = world.elsewhere.join("claude");
    let script = r#"#!/bin/sh
if [ "$1" = "--version" ]; then
  echo '2.1.309 (Claude Code)'
  exit 0
fi
printf 'step\n' >> '@CALLS@'
printf '%s\n' '{"type":"system","subtype":"init","tools":["Read","Edit","Write","Glob","Grep"]}'
printf '%s\n' '{"type":"result","subtype":"success","result":"{\"action\": \"tests.write\", \"summary\": \"nothing\"}","usage":{"input_tokens":3,"output_tokens":4}}'
"#
    .replace("@CALLS@", &world.elsewhere.join("calls").display().to_string());
    fs::write(&at, script).expect("a script");
    fs::set_permissions(&at, fs::Permissions::from_mode(0o755)).expect("exec bit");
    at
}

fn session(program: &Path) -> ClaudeSession {
    let mut session = ClaudeSession {
        credentials: CredentialSource::None,
        ..ClaudeSession::default()
    };
    session.program = program.to_str().expect("utf-8").to_owned();
    session
}

fn options(case: &str) -> RunOptions {
    RunOptions {
        protocol: "story.delivery/1".to_owned(),
        intent: "Make a change".to_owned(),
        inputs: Vec::new(),
        iterations: Some(1),
        hours: None,
        tokens: None,
        case_id: Some(case.to_owned()),
        gate: None,
    }
}

fn command(name: &str, argv: &[&str]) -> SetupCommand {
    SetupCommand {
        name: name.to_owned(),
        argv: argv.iter().map(|word| (*word).to_owned()).collect(),
    }
}

/// A run of the case `case` that opens it, or resumes it when `resume`, declaring `setup`, the
/// tests command `test`; its stop and its output.
fn go(
    world: &World,
    case: &str,
    resume: bool,
    test: &str,
    setup: Vec<SetupCommand>,
) -> (Stop, String) {
    let claude = fake_claude(world);
    let harness = session(&claude);
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(TestsRun::new(test).expect("a command"))];
    let mut run = if resume {
        Run::resume(&world.tree, options(case), case, &harness, &validators).expect("resumes")
    } else {
        Run::open(&world.tree, options(case), &harness, &validators).expect("opens")
    };
    run.transcripts_in(world.state().join("transcripts"))
        .expect("a state directory");
    run.declare_setup(setup);
    let mut out = Vec::new();
    let stop = run.run(&mut out).expect("the run ends");
    (stop, String::from_utf8_lossy(&out).into_owned())
}

/// A command that exits 3 stops the run `SetupFailed` naming it and `3`, the stop line the
/// record's last; the double model is started for no step, and the tests command never ran.
#[test]
fn a_failed_setup_stops_before_a_model_call() {
    let world = world("failed");
    let (stop, output) = go(
        &world,
        "setup-failed",
        false,
        "true",
        vec![command("deps", &["sh", "-c", "exit 3"])],
    );
    assert_eq!(
        stop,
        Stop::SetupFailed {
            name: "deps".to_owned(),
            exit: Some(3)
        },
        "{output}"
    );
    assert_eq!(stop.to_string(), "SetupFailed (deps exited 3)");
    assert_eq!(world.steps(), 0, "a model was called\n{output}");
    let events = world.events("setup-failed");
    let setup = events
        .iter()
        .find(|line| line["event"] == "setup")
        .expect("a setup line");
    assert_eq!(setup["name"], "deps");
    assert_eq!(setup["exit"], 3);
    assert!(setup["duration_ms"].is_u64(), "{setup}");
    let last = events.last().expect("a last line");
    assert_eq!(last["event"], "stop");
    assert_eq!(last["stop"], "SetupFailed (deps exited 3)");
    assert!(
        !events.iter().any(|line| line["event"] == "step"),
        "a step ran: {events:?}"
    );
}

/// With a red base, the record holds the `setup` line before the first check's `step` line, and
/// the run then stops `RedBase`: setup ran first and the check after it.
#[test]
fn setup_runs_before_the_red_base_check() {
    let world = world("order");
    let (stop, output) = go(
        &world,
        "setup-order",
        false,
        "false",
        vec![command("deps", &["true"]), command("more", &["true"])],
    );
    assert!(matches!(stop, Stop::RedBase { .. }), "{stop}\n{output}");
    let events = world.events("setup-order");
    let at = |wanted: &dyn Fn(&serde_json::Value) -> bool| {
        events
            .iter()
            .position(wanted)
            .unwrap_or_else(|| panic!("a line is missing in {events:?}"))
    };
    let first_setup = at(&|line| line["event"] == "setup" && line["name"] == "deps");
    let second_setup = at(&|line| line["event"] == "setup" && line["name"] == "more");
    let check = at(&|line| line["event"] == "step" && line["action"] == "tests.run");
    assert!(first_setup < second_setup, "{events:?}");
    assert!(second_setup < check, "{events:?}");
    assert_eq!(world.steps(), 0, "a model was called\n{output}");
}

/// A resume on the base the setup ran at runs it no more; one on another base runs it again.
/// The counter is in the tree's `.takt`: on macOS a setup command writes only in its tree.
#[test]
fn a_resume_on_the_same_base_skips_setup() {
    let world = world("resume");
    let counter = world.tree.join(".takt/setup-counter");
    let setup = || {
        vec![command(
            "count",
            &[
                "sh",
                "-c",
                "echo x >> \"$0\"",
                counter.to_str().expect("utf-8"),
            ],
        )]
    };
    let ran = || {
        fs::read_to_string(&counter)
            .unwrap_or_default()
            .lines()
            .count()
    };
    let (stop, output) = go(&world, "setup-resume", false, "false", setup());
    assert!(matches!(stop, Stop::RedBase { .. }), "{stop}\n{output}");
    assert_eq!(ran(), 1, "{output}");
    let (_, output) = go(&world, "setup-resume", true, "false", setup());
    assert_eq!(
        ran(),
        1,
        "a resume on the same base ran the setup\n{output}"
    );
    assert!(output.contains("setup: not run again"), "{output}");
    let setups = world
        .events("setup-resume")
        .iter()
        .filter(|line| line["event"] == "setup")
        .count();
    assert_eq!(setups, 1);

    let case_dir = world.case_dir("setup-resume");
    let mut branch = takt::case::Branch::read(&case_dir).expect("the branch record");
    let moved = git(&world.tree, &["rev-parse", "HEAD~1"]);
    assert_ne!(branch.base_commit.as_deref(), Some(moved.as_str()));
    branch.base_commit = Some(moved.clone());
    branch.write(&case_dir).expect("the moved base");
    let (_, output) = go(&world, "setup-resume", true, "false", setup());
    assert_eq!(
        ran(),
        2,
        "a resume on a moved base did not run it\n{output}"
    );
    let last = world
        .events("setup-resume")
        .into_iter()
        .rfind(|line| line["event"] == "setup")
        .expect("a setup line");
    assert_eq!(last["base"], moved);
}

/// A key beside `name` and `argv` in a setup entry is refused naming the file and the line it
/// is written on, in TOML and in YAML, and so is a bad name or an empty `argv`.
#[test]
fn an_unknown_setup_key_is_refused_at_its_line() {
    let world = world("unknown");
    let yaml = world.elsewhere.join("run.yaml");
    fs::write(
        &yaml,
        "test_cmd: ./check.sh\nsetup:\n  - name: deps\n    argv: [/bin/true]\n  - name: more\n    argv: [/bin/true]\n    shell: make\n",
    )
    .expect("a file");
    let error = format!("{:#}", RunConfig::read(&yaml).expect_err("refused"));
    assert!(error.contains(&yaml.display().to_string()), "{error}");
    assert!(error.contains("line 7"), "{error}");
    assert!(!error.contains("make"), "a value is repeated: {error}");
    let toml = world.elsewhere.join("run.toml");
    fs::write(
        &toml,
        "test_cmd = \"./check.sh\"\n\n[[setup]]\nname = \"deps\"\nargv = [\"true\"]\n\n[[setup]]\nname = \"more\"\nargv = [\"true\"]\nshell = \"make\"\n",
    )
    .expect("a file");
    let error = format!("{:#}", RunConfig::read(&toml).expect_err("refused"));
    assert!(error.contains(&toml.display().to_string()), "{error}");
    assert!(error.contains("line 10"), "{error}");
    assert!(!error.contains("make"), "a value is repeated: {error}");
    for (text, why) in [
        (
            "setup:\n  - name: a/b\n    argv: [/bin/true]\n",
            "plain word",
        ),
        (
            "setup:\n  - name: a\n    argv: [/bin/true]\n  - name: a\n    argv: [/bin/true]\n",
            "repeats the name",
        ),
        ("setup:\n  - name: a\n    argv: []\n", "names a program"),
        ("setup:\n  - name: a\n", "line"),
    ] {
        fs::write(&yaml, text).expect("a file");
        let error = format!("{:#}", RunConfig::read(&yaml).expect_err("refused"));
        assert!(error.contains(why), "{text}: {error}");
    }
    fs::write(&yaml, "setup:\n  - name: a\n    argv: [/bin/true]\n").expect("a file");
    let config = RunConfig::read(&yaml).expect("reads");
    assert_eq!(config.setup, Some(vec![command("a", &["/bin/true"])]));
}

/// An argument is an argument: `a; touch pwned` reaches the program whole, is printed to its
/// output file, and no shell ran `touch pwned`.
#[test]
fn an_argument_is_never_a_shell_string() {
    let world = world("shell");
    let (stop, output) = go(
        &world,
        "setup-shell",
        false,
        "true",
        vec![command("print", &["printf", "%s", "a; touch pwned"])],
    );
    assert!(
        !matches!(stop, Stop::SetupFailed { .. }),
        "{stop}\n{output}"
    );
    let events = world.events("setup-shell");
    let kept = events
        .iter()
        .find(|line| line["event"] == "setup")
        .and_then(|line| line["output"].as_str())
        .expect("an output file named on the setup line");
    let text = fs::read_to_string(world.state().join("transcripts").join(kept))
        .expect("the output file under the state directory");
    assert_eq!(text, "a; touch pwned");
    assert!(!world.tree.join("pwned").exists());
    assert!(!world.elsewhere.join("pwned").exists());
    assert!(!Path::new("pwned").exists());
}
