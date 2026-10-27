//! story:harness-program-is-chosen-per-run, where `takt run` starts its sessions: `--harness-program
//! <path>` names the program every step's session starts, and the run's `opened` line records the
//! program's absolute path, the sha256 of its bytes and the first line its `--version` prints,
//! the one found on `PATH` too when no flag is given. A path that is relative or names no
//! executable file is refused before a case opens. The `claude` here is a double; nothing reaches
//! a model.

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const VERSION: &str = "fake-claude 9.9.9 (double)";

fn git(tree: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
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

fn executable(path: &Path, text: &str) {
    fs::write(path, text).expect("a script");
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("exec bit");
}

/// A `claude` that answers `--version` with [`VERSION`] and, started as a session, writes `step`
/// to `<seen>/calls` and the path it was started by to `<seen>/program`, changes no file but a
/// test file, and prints the result event naming `tests.write`.
fn fake_claude(at: &Path, seen: &Path) {
    let script = r#"#!/bin/sh
if [ "$1" = "--version" ]; then
  printf 'version\n' >> '@SEEN@/calls'
  printf '%s\n' '@VERSION@'
  exit 0
fi
printf 'step\n' >> '@SEEN@/calls'
printf '%s\n' "$0" > '@SEEN@/program'
printf '%s\n' '{"type":"system","subtype":"init","tools":["Read","Edit","Write","Glob","Grep"]}'
mkdir -p tests
echo work >> tests/work.txt
printf '%s\n' '{"type":"result","subtype":"success","result":"{\"action\": \"tests.write\", \"summary\": \"wrote a test\"}","usage":{"input_tokens":3,"output_tokens":4}}'
"#
    .replace("@SEEN@", &seen.display().to_string())
    .replace("@VERSION@", VERSION);
    executable(at, &script);
}

struct World {
    _scratch: Scratch,
    repo: PathBuf,
    bin: PathBuf,
    elsewhere: PathBuf,
    seen: PathBuf,
}

fn world(name: &str) -> World {
    let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("harness-program-cli-{}-{name}", std::process::id()));
    if outer.exists() {
        fs::remove_dir_all(&outer).expect("a stale scratch tree is removed");
    }
    let repo = outer.join("repo");
    let bin = outer.join("bin");
    let elsewhere = outer.join("elsewhere");
    let seen = outer.join("seen");
    for dir in [&repo, &bin, &elsewhere, &seen] {
        fs::create_dir_all(dir).expect("a directory");
    }
    fs::write(repo.join("README.md"), "a story\n").expect("a file");
    executable(&repo.join("check.sh"), "#!/bin/sh\nexit 1\n");
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "start"]);
    World {
        _scratch: Scratch(outer),
        repo,
        bin,
        elsewhere,
        seen,
    }
}

impl World {
    /// `takt run --harness claude` with `extra` flags and `path_dirs` first on `PATH`.
    fn run(&self, extra: &[&str], path_dirs: &[&Path]) -> Output {
        let mut path: Vec<String> = path_dirs
            .iter()
            .map(|dir| dir.display().to_string())
            .collect();
        path.push(std::env::var("PATH").unwrap_or_default());
        Command::new(env!("CARGO_BIN_EXE_takt"))
            .args([
                "run",
                "--continue-past-red-base",
                "--repo",
                self.repo.to_str().expect("utf-8"),
                "--intent",
                "probe",
                "--harness",
                "claude",
                "--forge",
                "none",
                "--iterations",
                "2",
                "--test-cmd",
                "./check.sh",
            ])
            .args(extra)
            .env("PATH", path.join(":"))
            .env("XDG_STATE_HOME", self.seen.join("state"))
            .env_remove("B10X_GATES_POLICY")
            .output()
            .expect("takt runs")
    }

    fn calls(&self) -> Vec<String> {
        fs::read_to_string(self.seen.join("calls"))
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect()
    }

    fn cases(&self) -> Vec<PathBuf> {
        fs::read_dir(self.repo.join(".takt/cases"))
            .map(|entries| {
                entries
                    .map(|entry| entry.expect("an entry").path())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The case's `opened` line.
    fn opened(&self, said: &str) -> serde_json::Value {
        let cases = self.cases();
        assert_eq!(cases.len(), 1, "{said}");
        let events = fs::read_to_string(cases[0].join("events.jsonl")).expect("the run record");
        events
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("a line"))
            .find(|line| line["event"] == "opened")
            .unwrap_or_else(|| panic!("an opened line in\n{events}\n{said}"))
    }
}

fn said(output: &Output) -> String {
    format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
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

/// With a double at a path that is on no `PATH`, `--harness-program` starts that one, and only that
/// one (a `claude` on `PATH` is a different double that records nothing): `opened` holds the
/// absolute path, the sha256 of its bytes and the version it printed.
#[test]
fn a_named_binary_is_used_and_recorded() {
    let world = world("named");
    let named = world.elsewhere.join("qualified-claude");
    fake_claude(&named, &world.seen);
    let decoy = world.bin.join("claude");
    executable(&decoy, "#!/bin/sh\nexit 9\n");
    let output = world.run(
        &["--harness-program", named.to_str().expect("utf-8")],
        &[&world.bin],
    );
    let said = said(&output);
    assert!(world.calls().iter().any(|call| call == "step"), "{said}");
    assert_eq!(
        fs::read_to_string(world.seen.join("program"))
            .expect("the path the double was started by")
            .trim(),
        named.display().to_string(),
        "{said}"
    );
    let opened = world.opened(&said);
    let program = &opened["harness_program"];
    assert_eq!(program["path"], named.display().to_string(), "{opened}");
    assert_eq!(program["sha256"], sha256_of(&named), "{opened}");
    assert_eq!(program["version"], VERSION, "{opened}");
}

/// A relative path, a bare name and a path that names no executable file are refused before a
/// case opens, exit 1, naming the value; so is a program with `--harness none`.
#[test]
fn a_relative_or_missing_program_is_refused() {
    let world = world("refused");
    let plain = world.elsewhere.join("not-executable");
    fs::write(&plain, "text\n").expect("a file");
    let missing = world.elsewhere.join("missing-claude");
    for value in [
        "bin/claude",
        "./claude",
        "claude",
        missing.to_str().expect("utf-8"),
        plain.to_str().expect("utf-8"),
        world.elsewhere.to_str().expect("utf-8"),
    ] {
        let output = world.run(&["--harness-program", value], &[]);
        let said = said(&output);
        assert_eq!(output.status.code(), Some(1), "{value}: {said}");
        let named = takt::git::shown(std::ffi::OsStr::new(value));
        assert!(said.contains(&named), "{value} is not named: {said}");
        assert!(world.cases().is_empty(), "{value}: a case opened: {said}");
        assert!(world.calls().is_empty(), "{value}: a program started");
    }
    let named = world.elsewhere.join("qualified-claude");
    fake_claude(&named, &world.seen);
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args([
            "run",
            "--repo",
            world.repo.to_str().expect("utf-8"),
            "--intent",
            "probe",
            "--harness",
            "none",
            "--forge",
            "none",
            "--harness-program",
            named.to_str().expect("utf-8"),
        ])
        .env_remove("B10X_GATES_POLICY")
        .output()
        .expect("takt runs");
    let said = said(&output);
    assert_eq!(output.status.code(), Some(1), "{said}");
    assert!(said.contains("--harness none"), "{said}");
    assert!(world.cases().is_empty(), "{said}");
}

/// Without the flag the binary is the one found on `PATH`, recorded the same way.
#[test]
fn path_resolution_is_recorded() {
    let world = world("path");
    let found = world.bin.join("claude");
    fake_claude(&found, &world.seen);
    let output = world.run(&[], &[&world.bin]);
    let said = said(&output);
    assert!(world.calls().iter().any(|call| call == "step"), "{said}");
    let opened = world.opened(&said);
    let program = &opened["harness_program"];
    assert_eq!(program["path"], found.display().to_string(), "{opened}");
    assert_eq!(program["sha256"], sha256_of(&found), "{opened}");
    assert_eq!(program["version"], VERSION, "{opened}");
}
