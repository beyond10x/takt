//! story:harness-program-is-chosen-per-run, in the library: a harness whose `program` is named by
//! an absolute path starts that program for every step, and the run's `opened` line records its
//! path, the sha256 of its bytes and the first line its `--version` prints; a name is resolved on
//! `PATH` and recorded the same way; a path that is relative or names no executable file is
//! refused naming it. The program is a double; nothing reaches a model.

#[path = "support/own_home.rs"]
mod own_home;

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use loom_sdk::session::CredentialSource;
use takt::harness::{ClaudeSession, Harness as _};
use takt::validator::{TestsRun, Validator};
use takt::{Run, RunOptions};

const VERSION: &str = "2.1.309 (Claude Code)";
const BROKEN: &str = "pub fn add(a: i32, b: i32) -> i32 {\n    a - b\n}\n";

fn git(tree: &Path, args: &[&str]) {
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
}

struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// A scratch directory with a repository `tree` and a directory `elsewhere` on no `PATH`.
fn world(name: &str) -> (Scratch, PathBuf, PathBuf) {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("harness-program-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let tree = dir.join("tree");
    let elsewhere = dir.join("elsewhere");
    fs::create_dir_all(tree.join("src")).expect("the tree");
    fs::create_dir_all(&elsewhere).expect("a directory");
    fs::write(tree.join("README.md"), "a story\n").expect("a file");
    fs::write(tree.join("src/lib.rs"), BROKEN).expect("a file");
    git(&tree, &["init", "-q", "-b", "main"]);
    git(&tree, &["add", "-A"]);
    git(&tree, &["commit", "-q", "-m", "start"]);
    (Scratch(dir), tree, elsewhere)
}

/// A `claude` that answers `--version` with [`VERSION`] and, started as a session, appends `step`
/// to `calls`, writes the fix and prints the result event naming `implementation.edit`.
fn fake_claude(at: &Path, calls: &Path) {
    let script = r#"#!/bin/sh
if [ "$1" = "--version" ]; then
  printf 'version\n' >> '@CALLS@'
  printf '%s\n' '@VERSION@'
  exit 0
fi
printf 'step\n' >> '@CALLS@'
printf 'pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n' > src/lib.rs
printf '%s\n' '{"type":"system","subtype":"init","tools":["Read","Edit","Write","Glob","Grep"]}'
printf '%s\n' '{"type":"result","subtype":"success","result":"{\"action\": \"implementation.edit\", \"summary\": \"fixed add\"}","usage":{"input_tokens":3,"output_tokens":4}}'
"#
    .replace("@CALLS@", &calls.display().to_string())
    .replace("@VERSION@", VERSION);
    fs::write(at, script).expect("a script");
    fs::set_permissions(at, fs::Permissions::from_mode(0o755)).expect("exec bit");
}

fn session(program: &str) -> ClaudeSession {
    let mut session = ClaudeSession {
        credentials: CredentialSource::None,
        ..ClaudeSession::default()
    };
    session.program = program.to_owned();
    session
}

fn options(name: &str) -> RunOptions {
    RunOptions {
        protocol: "story.delivery/1".to_owned(),
        intent: "Make add add".to_owned(),
        inputs: Vec::new(),
        iterations: Some(2),
        hours: None,
        tokens: None,
        case_id: Some(format!("harness-program-{name}")),
        gate: None,
    }
}

fn sha256_of(path: &Path) -> String {
    let output = Command::new("sha256sum")
        .arg(path)
        .output()
        .expect("sha256sum runs");
    String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .next()
        .expect("a digest")
        .to_owned()
}

/// The `opened` line of the case `name` opened in `tree`.
fn opened(tree: &Path, name: &str) -> serde_json::Value {
    let events = fs::read_to_string(
        tree.join(".takt/cases")
            .join(format!("harness-program-{name}/events.jsonl")),
    )
    .expect("the run record");
    events
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("a line"))
        .find(|line| line["event"] == "opened")
        .unwrap_or_else(|| panic!("an opened line in\n{events}"))
}

/// A double at a path on no `PATH` is the program every step starts, and `opened` holds its
/// absolute path, the sha256 of its bytes and the version it printed.
#[test]
fn a_named_binary_is_used_and_recorded() {
    let (_scratch, tree, elsewhere) = world("named");
    let calls = elsewhere.join("calls");
    let named = elsewhere.join("qualified-claude");
    fake_claude(&named, &calls);
    let harness = session(named.to_str().expect("utf-8"));
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(TestsRun::new("true").expect("a command"))];
    let mut run = Run::open(&tree, options("named"), &harness, &validators).expect("opens");
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let output = String::from_utf8_lossy(&out).into_owned();
    assert!(stop.is_ok(), "the run ends: {stop:?}\n{output}");
    drop(run);
    let calls = fs::read_to_string(&calls).expect("the double's record");
    assert!(
        calls.lines().any(|call| call == "step"),
        "{calls}\n{output}"
    );
    let opened = opened(&tree, "named");
    let program = &opened["harness_program"];
    assert_eq!(program["path"], named.display().to_string(), "{opened}");
    assert_eq!(program["sha256"], sha256_of(&named), "{opened}");
    assert_eq!(program["version"], VERSION, "{opened}");
}

/// A relative path, and a path that names no executable file, are refused naming the value: by
/// the harness's own check, and by a run that is given one, which writes no `opened` line.
#[test]
fn a_relative_or_missing_program_is_refused() {
    let (_scratch, tree, elsewhere) = world("refused");
    let plain = elsewhere.join("not-executable");
    fs::write(&plain, "text\n").expect("a file");
    let missing = elsewhere.join("missing-claude");
    for value in [
        "bin/claude".to_owned(),
        "./claude".to_owned(),
        missing.display().to_string(),
        plain.display().to_string(),
        elsewhere.display().to_string(),
    ] {
        let harness = session(&value);
        let error = harness
            .check_program()
            .expect_err("a program that cannot be started is refused");
        let named = takt::git::shown(std::ffi::OsStr::new(&value));
        assert!(format!("{error:#}").contains(&named), "{value}: {error:#}");
        let error = harness
            .program_record()
            .expect_err("the record refuses it too");
        let named = takt::git::shown(std::ffi::OsStr::new(&value));
        assert!(format!("{error:#}").contains(&named), "{value}: {error:#}");
    }
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(TestsRun::new("true").expect("a command"))];
    let harness = session(&missing.display().to_string());
    let mut run = Run::open(&tree, options("refused"), &harness, &validators).expect("opens");
    let mut out = Vec::new();
    let error = match run.run(&mut out) {
        Ok(stop) => panic!("the run went on with a missing program: {stop:?}"),
        Err(error) => format!("{error:#}"),
    };
    assert!(
        error.contains(&takt::git::shown(missing.as_os_str())),
        "{error}"
    );
    drop(run);
    let events = fs::read_to_string(tree.join(".takt/cases/harness-program-refused/events.jsonl"))
        .unwrap_or_default();
    assert!(
        !events.contains("\"event\":\"opened\""),
        "an opened line was written\n{events}"
    );
}

/// A bare name is resolved on `PATH` and recorded the same way: the absolute path of the file
/// found, the sha256 of its bytes, and the version when it prints one.
#[test]
fn path_resolution_is_recorded() {
    let harness = session("true");
    let record = harness
        .program_record()
        .expect("a record")
        .expect("`true` is on PATH");
    let path = Path::new(&record.path);
    assert!(path.is_absolute(), "{}", record.path);
    assert_eq!(
        path.file_name().and_then(|name| name.to_str()),
        Some("true")
    );
    assert_eq!(record.sha256, sha256_of(path));
    let nowhere = session("takt-no-such-harness-program");
    assert_eq!(nowhere.program_record().expect("no refusal"), None);
}
