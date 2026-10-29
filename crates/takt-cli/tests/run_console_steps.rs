//! story:run-console-steps-are-compact through the binary: before each harness call `takt run`
//! prints `step N: <harness> working (may: …)`; a harness step's line, after the call, says the
//! call's duration, tokens and cost when the harness reported them; `effect: committed N
//! path(s); implementation is now <rev>` names the paths it committed; and the `stopped:` line
//! names the run's wall time, its cost and its commits before its `, case <dir>`. The record
//! (`events.jsonl`) holds what it held before. A harness double on the program path stands in
//! for Claude Code: it writes two failing tests at its first call, reporting tokens, cost and
//! duration, and declines every later call reporting none. Nothing reaches a network or a model.

#[path = "../../takt/tests/support/own_home.rs"]
mod own_home;

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The fixture stores of the library crate, one directory each.
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../takt/tests/fixtures");

/// A gates document whose one assertion is true in a repository with a manifest.
const GATES: &str = "format: engineering-gates/1\nlanguage: canon-expr/1\ngates:\n  - id: manifest\n    \
                     text: The repository has its manifest.\n    assertions:\n      \
                     - 'fs.file.exists(\"Cargo.toml\")'\n";

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

/// A scratch directory under this crate's build directory, removed on drop.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("a directory");
    for entry in fs::read_dir(from).expect("a readable fixture") {
        let entry = entry.expect("an entry");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("a file type").is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).expect("a copied file");
        }
    }
}

/// The scratch world: a repository with the fixture store, its one story `proposed`, scenario 1's
/// code and its passing tests, a manifest and a gates document; a `claude` double in `bin`.
struct World {
    _scratch: Scratch,
    repo: PathBuf,
    bin: PathBuf,
    state: PathBuf,
}

/// What the double reports for its first call: 1200 input and 34 output tokens, $0.0123 and
/// 4321 ms.
const FIRST_LINE: &str = "step 2: tests.write (claude-code) · 4.3s · 1.2k tok · $0.01";

fn world(name: &str) -> World {
    let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("run-console-steps-{}-{name}", std::process::id()));
    if outer.exists() {
        fs::remove_dir_all(&outer).expect("a stale scratch tree is removed");
    }
    let repo = outer.join("repo");
    copy_dir(&Path::new(FIXTURES).join("one-ready"), &repo);
    copy_dir(&Path::new(FIXTURES).join("scenario-1/base"), &repo);
    for script in ["check.sh", "todo.py"] {
        fs::set_permissions(repo.join(script), fs::Permissions::from_mode(0o755))
            .expect("executable");
    }
    fs::write(repo.join("Cargo.toml"), "[package]\nname = \"scratch\"\n").expect("a file");
    fs::write(repo.join(".engineering/gates.yaml"), GATES).expect("a file");
    fs::write(repo.join(".gitignore"), "__pycache__/\n").expect("a file");
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "seed"]);
    let bin = outer.join("stub-bin");
    fs::create_dir_all(&bin).expect("a stub directory");
    let state = outer.join("state");
    fs::create_dir_all(&state).expect("a state directory");
    let path = bin.join("claude");
    let calls = outer.join("calls");
    fs::write(
        &path,
        format!(
            r#"#!/bin/sh
case $1 in --version) echo '2.1.309 (Claude Code)'; exit 0 ;; esac
echo call >> '{calls}'
if [ "$(wc -l < '{calls}')" -eq 1 ]; then
  printf 'import unittest\n\n\nclass ConsoleTest(unittest.TestCase):\n    def test_not_yet(self):\n        self.fail("not yet")\n' > tests/test_console.py
  printf 'import unittest\n\n\nclass OtherTest(unittest.TestCase):\n    def test_other(self):\n        self.fail("not yet")\n' > tests/test_other.py
  printf '%s' '{{"type":"result","subtype":"success","result":"{{\"action\": \"tests.write\", \"summary\": \"two failing tests\"}}","usage":{{"input_tokens":1200,"output_tokens":34}},"total_cost_usd":0.0123,"duration_ms":4321}}'
  exit 0
fi
printf '%s' '{{"type":"result","subtype":"success","result":"{{\"action\": \"decline\", \"summary\": \"nothing more to do\"}}"}}'
"#,
            calls = calls.display(),
        ),
    )
    .expect("the double is written");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("executable");
    World {
        _scratch: Scratch(outer),
        repo,
        bin,
        state,
    }
}

impl World {
    /// `takt run` of the store's one ready story on `story.delivery/2` with the double.
    fn run(&self) -> (i32, String, String) {
        let repo = self.repo.to_str().expect("utf-8");
        self.takt(&[
            "run",
            "--repo",
            repo,
            "--story",
            "story:ready-one",
            "--protocol",
            "story.delivery/2",
            "--test-cmd",
            "./check.sh",
            "--forge",
            "none",
            "--iterations",
            "6",
        ])
    }

    /// Runs takt with `args`: the double first on the program path, the state home in the
    /// scratch world. The exit code, stdout and stderr.
    fn takt(&self, args: &[&str]) -> (i32, String, String) {
        let output = Command::new(env!("CARGO_BIN_EXE_takt"))
            .args(args)
            .env_remove("B10X_GATES_POLICY")
            .env("XDG_STATE_HOME", self.state.join("xdg-state"))
            .env(
                "PATH",
                format!(
                    "{}:{}",
                    self.bin.display(),
                    std::env::var("PATH").unwrap_or_default()
                ),
            )
            .output()
            .expect("takt runs");
        (
            output.status.code().expect("an exit code"),
            String::from_utf8_lossy(&output.stdout).into_owned(),
            String::from_utf8_lossy(&output.stderr).into_owned(),
        )
    }
}

/// The index of the first line of `out` that starts with `prefix`.
fn line_of(out: &str, prefix: &str) -> Option<usize> {
    out.lines().position(|line| line.starts_with(prefix))
}

/// The step number and the harness of a harness step's result line, `step N: <action>
/// (<harness>)…`; none for any other line.
fn harness_result(line: &str) -> Option<(u64, String)> {
    let rest = line.strip_prefix("step ")?;
    let (n, rest) = rest.split_once(": ")?;
    let n: u64 = n.parse().ok()?;
    let (action, rest) = rest.split_once(" (")?;
    if action.contains(' ') || !rest.contains(')') {
        return None;
    }
    let (harness, _) = rest.split_once(')')?;
    Some((n, harness.to_owned()))
}

/// The case directory the `stopped:` line names after its last `, case `.
fn case_of(out: &str) -> PathBuf {
    let last = out.lines().last().unwrap_or_default();
    let (_, case) = last
        .rsplit_once(", case ")
        .unwrap_or_else(|| panic!("no case on the stop line: {out}"));
    PathBuf::from(case)
}

/// The run's record, one JSON value a line.
fn events(case: &Path) -> Vec<serde_json::Value> {
    fs::read_to_string(case.join("events.jsonl"))
        .expect("the run record")
        .lines()
        .map(|line| serde_json::from_str(line).expect("a JSON line"))
        .collect()
}

/// Acceptance 1: before each harness call, a `step N: <harness> working (may: …)` line comes
/// before that step's result line, naming the actions offered.
#[test]
fn a_working_line_precedes_each_harness_step() {
    let world = world("working");
    let (code, out, err) = world.run();
    let lines: Vec<&str> = out.lines().collect();
    let results: Vec<(usize, u64, String)> = lines
        .iter()
        .enumerate()
        .filter_map(|(at, line)| harness_result(line).map(|(n, h)| (at, n, h)))
        .collect();
    assert!(
        results.len() >= 2,
        "two harness steps at least: exit {code}\nstdout {out}\nstderr {err}"
    );
    for (at, n, harness) in &results {
        let working = format!("step {n}: {harness} working (may: ");
        let before = lines[..*at]
            .iter()
            .rposition(|line| line.starts_with(&working))
            .unwrap_or_else(|| panic!("no working line before step {n}'s result: {out}"));
        assert!(
            lines[before + 1..*at]
                .iter()
                .all(|line| !line.starts_with("step ")),
            "the working line is the step's own: {out}"
        );
        assert!(lines[before].ends_with(')'), "{out}");
    }
    let working = line_of(&out, "step 2: claude-code working (may: ")
        .unwrap_or_else(|| panic!("no working line for step 2: {out}"));
    assert!(lines[working].contains("tests.write"), "{out}");
}

/// Acceptance 1, the story's `expected_before` predicate: the `none` harness's step 2 has its
/// working line before its result line.
#[test]
fn the_none_harness_says_it_is_working() {
    let world = world("none");
    git(&world.repo, &["config", "user.name", "t"]);
    git(&world.repo, &["config", "user.email", "t@example.invalid"]);
    let repo = world.repo.to_str().expect("utf-8");
    let (code, out, err) = world.takt(&[
        "run",
        "--repo",
        repo,
        "--intent",
        "Remove a todo by its number",
        "--harness",
        "none",
        "--forge",
        "none",
        "--test-cmd",
        "true",
        "--min-free-gib",
        "0",
        "--iterations",
        "2",
    ]);
    let result = line_of(&out, "step 2: repository.merge (none)")
        .unwrap_or_else(|| panic!("no harness step 2: exit {code}\nstdout {out}\nstderr {err}"));
    let lines: Vec<&str> = out.lines().collect();
    assert!(
        lines[..result]
            .iter()
            .any(|line| line.starts_with("step 2: ") && line.contains("working")),
        "no working line before the harness was called: {out}"
    );
    assert!(
        line_of(&out, "step 2: none working (may: ").is_some_and(|at| at < result),
        "{out}"
    );
    assert_eq!(
        lines[result], "step 2: repository.merge (none)",
        "a harness that reported nothing appends nothing: {out}"
    );
}

/// Acceptance 2: the step whose call reported tokens, cost and duration says each on its line;
/// the decline, whose call reported none, says nothing of them.
#[test]
fn a_harness_step_line_says_its_time_tokens_and_cost() {
    let world = world("cost");
    let (code, out, err) = world.run();
    let first = line_of(&out, "step 2: tests.write (claude-code)")
        .unwrap_or_else(|| panic!("no step 2: exit {code}\nstdout {out}\nstderr {err}"));
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines[first], FIRST_LINE, "{out}");
    assert!(
        lines[first + 1].starts_with("  said: two failing tests"),
        "said: follows its step line: {out}"
    );
    let decline = lines
        .iter()
        .find(|line| line.starts_with("step ") && line.contains(": decline (claude-code)"))
        .unwrap_or_else(|| panic!("no decline: {out}"));
    assert!(
        decline.ends_with(": decline (claude-code)"),
        "a call that reported nothing appends nothing: {out}"
    );
}

/// The story's outcome for a validator step: its line says its result in a few words, the
/// failed tests named, and its duration.
#[test]
fn a_validator_step_line_says_its_result_and_time() {
    let world = world("validator");
    let (code, out, err) = world.run();
    let lines: Vec<&str> = out.lines().collect();
    let pass = line_of(&out, "step 1: tests.run {}")
        .unwrap_or_else(|| panic!("no step 1: exit {code}\nstdout {out}\nstderr {err}"));
    let rest = lines[pass]
        .strip_prefix("step 1: tests.run {} · pass · ")
        .unwrap_or_else(|| panic!("a pass and its time: {out}"));
    assert!(
        rest.ends_with('s') && rest.chars().next().is_some_and(|c| c.is_ascii_digit()),
        "{out}"
    );
    let fail = line_of(&out, "step 3: tests.run {}").unwrap_or_else(|| panic!("no step 3: {out}"));
    let rest = lines[fail]
        .strip_prefix("step 3: tests.run {} · fail: ")
        .unwrap_or_else(|| panic!("a failure and what failed: {out}"));
    let (named, time) = rest
        .rsplit_once(" · ")
        .unwrap_or_else(|| panic!("the time last: {out}"));
    assert!(
        named.starts_with("test_not_yet") && named.contains("test_other"),
        "{out}"
    );
    assert!(time.ends_with('s'), "{out}");
}

/// Acceptance 3: the committed effect names the paths it committed, relative to the tree.
#[test]
fn the_committed_effect_names_its_paths() {
    let world = world("paths");
    let (code, out, err) = world.run();
    let effect = out
        .lines()
        .find(|line| line.starts_with("  effect: committed "))
        .unwrap_or_else(|| panic!("no committed effect: exit {code}\nstdout {out}\nstderr {err}"));
    let rest = effect
        .strip_prefix("  effect: committed 2 path(s); implementation is now ")
        .unwrap_or_else(|| panic!("two paths committed: {out}"));
    let (revision, paths) = rest
        .split_once(": ")
        .unwrap_or_else(|| panic!("the paths follow the revision: {out}"));
    assert_eq!(revision.len(), 8, "{out}");
    assert!(revision.chars().all(|c| c.is_ascii_hexdigit()), "{out}");
    assert_eq!(paths, "tests/test_console.py, tests/test_other.py", "{out}");
}

/// Acceptance 4: the `stopped:` line is the last line, keeps `stopped: <stop>; steps N, tokens
/// N`, then names the wall time, the cost reported and the commits with their paths, and ends
/// with `, case <dir>`.
#[test]
fn the_stopped_line_says_wall_time_cost_and_commits() {
    let world = world("stopped");
    let (code, out, err) = world.run();
    let last = out.lines().last().unwrap_or_default();
    assert!(
        last.starts_with("stopped: Declined"),
        "exit {code}\nstdout {out}\nstderr {err}"
    );
    let (_, figures) = last
        .split_once("; steps ")
        .unwrap_or_else(|| panic!("steps: {out}"));
    let (steps, figures) = figures
        .split_once(", tokens ")
        .expect("tokens follow steps");
    assert!(steps.parse::<u64>().is_ok(), "{last}");
    let (tokens, figures) = figures.split_once(", ").expect("figures follow the tokens");
    assert_eq!(tokens, "1234", "{last}");
    let (figures, case) = figures
        .rsplit_once(", case ")
        .unwrap_or_else(|| panic!("the case ends the line: {last}"));
    assert!(Path::new(case).join("events.jsonl").is_file(), "{last}");
    let parts: Vec<&str> = figures.split(", ").collect();
    assert_eq!(parts.len(), 3, "wall, cost, commits: {last}");
    let wall = parts[0]
        .strip_prefix("wall ")
        .unwrap_or_else(|| panic!("the wall time first: {last}"));
    assert!(
        wall.ends_with('s') && wall.chars().next().is_some_and(|c| c.is_ascii_digit()),
        "{last}"
    );
    assert_eq!(parts[1], "cost $0.01", "{last}");
    assert_eq!(parts[2], "commits 1 (2 paths)", "{last}");
}

/// Acceptance 4 for a run that committed nothing and reported no cost: the commits are said as
/// none and no cost is named.
#[test]
fn a_run_without_commits_or_cost_says_so() {
    let world = world("bare");
    let repo = world.repo.to_str().expect("utf-8");
    let (code, out, err) = world.takt(&[
        "run",
        "--repo",
        repo,
        "--intent",
        "Remove a todo by its number",
        "--harness",
        "none",
        "--forge",
        "none",
        "--test-cmd",
        "true",
        "--min-free-gib",
        "0",
        "--iterations",
        "1",
    ]);
    let last = out.lines().last().unwrap_or_default();
    assert!(
        last.starts_with("stopped: "),
        "exit {code}\nstdout {out}\nstderr {err}"
    );
    assert!(last.contains("; steps 1, tokens 0, wall "), "{last}");
    assert!(last.contains(", commits 0, case "), "{last}");
    assert!(!last.contains("cost"), "{last}");
}

/// Acceptance 5: the record holds what the console prints, as it did: the harness step's tokens,
/// cost and duration on its `step` event, the committed paths on its `effect` event; and no
/// event holds a console line.
#[test]
fn the_record_is_unchanged() {
    let world = world("record");
    let (code, out, err) = world.run();
    let case = case_of(&out);
    let record = events(&case);
    assert!(
        !record.is_empty(),
        "exit {code}\nstdout {out}\nstderr {err}"
    );
    let step = record
        .iter()
        .find(|e| e["event"] == "step" && e["action"] == "tests.write")
        .unwrap_or_else(|| panic!("no tests.write step: {record:?}"));
    assert_eq!(step["tokens"], 1234, "{step}");
    assert_eq!(step["cost_usd"], 0.0123, "{step}");
    assert_eq!(step["duration_ms"], 4321, "{step}");
    let effects: Vec<&serde_json::Value> = record
        .iter()
        .filter(|e| e["event"] == "effect" && !e["revision"].is_null())
        .collect();
    assert_eq!(effects.len(), 1, "{record:?}");
    assert_eq!(
        effects[0]["paths"],
        serde_json::json!(["tests/test_console.py", "tests/test_other.py"]),
        "{record:?}"
    );
    let text = fs::read_to_string(case.join("events.jsonl")).expect("the record");
    assert!(!text.contains(" working (may: "), "{text}");
    assert!(!text.contains("wall "), "{text}");
}
