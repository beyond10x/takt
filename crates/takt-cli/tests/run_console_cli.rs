//! story:run-console-opens-with-the-story through the binary: `takt run` opens with the story it
//! runs (takt's version and the story's id, its title, a summary of its intent) before the first
//! step, and says what the frontier admits as a change (`may:` once, then `may now: +x -y` when
//! the admissible set moves) instead of every action's status at every decision; `--verbose`
//! prints the full `frontier:` line at each decision too. A harness double on the program path
//! stands in for Claude Code, writes a failing test and then declines; nothing reaches a network
//! or a model.

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
/// code and its passing tests, a manifest and a gates document; a `claude` double in `bin` that
/// writes a failing test at its first call and names `tests.write`, and declines every later one.
struct World {
    _scratch: Scratch,
    repo: PathBuf,
    bin: PathBuf,
    state: PathBuf,
}

fn world(name: &str) -> World {
    let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("run-console-cli-{}-{name}", std::process::id()));
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
action=decline
summary='nothing more to do'
if [ "$(wc -l < '{calls}')" -eq 1 ]; then
  printf 'import unittest\n\n\nclass ConsoleTest(unittest.TestCase):\n    def test_not_yet(self):\n        self.fail("not yet")\n' > tests/test_console.py
  action=tests.write
  summary='a failing test'
fi
printf '{{"result":"{{\\"action\\": \\"%s\\", \\"summary\\": \\"%s\\"}}","usage":{{"input_tokens":1}}}}' "$action" "$summary"
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
    /// `takt run` of the store's one ready story on `story.delivery/2`, with `extra` after the
    /// flags.
    fn run(&self, extra: &[&str]) -> (i32, String, String) {
        let repo = self.repo.to_str().expect("utf-8");
        let mut args = vec![
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
        ];
        args.extend(extra);
        self.takt(&args)
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

/// Whether `line` is a per-decision frontier line: `frontier: <action> (<status>)…`, not one of
/// the store's (`frontier: took`, `resumed`, `passed over`, `nothing ready`).
fn is_decision_frontier(line: &str) -> bool {
    line.strip_prefix("frontier: ").is_some_and(|rest| {
        rest.split_once(' ').is_some_and(|(action, after)| {
            !action.ends_with(':') && action.contains('.') && after.starts_with('(')
        })
    })
}

/// Acceptance: before `step 1:` the run names takt's version and the story, its title and the
/// first paragraph of its `## Outcome`, and the actions it may take; no per-decision frontier
/// line is printed, and the failing first `tests.run` admits `implementation.edit`, said as
/// `may now: +implementation.edit` after the step.
#[test]
fn the_run_opens_with_the_story_and_says_the_frontier_as_changes() {
    let world = world("quiet");
    let (code, out, err) = world.run(&[]);
    let step1 = line_of(&out, "step 1:")
        .unwrap_or_else(|| panic!("no step 1: exit {code}\nstdout {out}\nstderr {err}"));
    let header = line_of(
        &out,
        &format!("takt {} · story:ready-one", env!("CARGO_PKG_VERSION")),
    )
    .unwrap_or_else(|| panic!("no header line: {out}"));
    let lines: Vec<&str> = out.lines().collect();
    assert!(header < step1, "{out}");
    assert_eq!(lines[header + 1], "  Make the check pass", "{out}");
    assert_eq!(
        lines[header + 2],
        "  The check in `check.txt` passes for whoever runs `./check.sh`.",
        "the summary is the first paragraph of the story's Outcome: {out}"
    );
    let may = line_of(&out, "  may: ").unwrap_or_else(|| panic!("no may: line: {out}"));
    assert!(header < may && may < step1, "{out}");
    assert!(lines[may].contains("tests.run"), "{out}");
    assert!(
        !out.lines().any(is_decision_frontier),
        "no per-decision frontier line without --verbose: {out}"
    );
    let failing = line_of(&out, "  evidence: test_result fail")
        .unwrap_or_else(|| panic!("no failing tests.run: {out}"));
    assert!(
        lines[..failing]
            .iter()
            .rev()
            .find(|line| line.starts_with("step "))
            .is_some_and(|step| step.contains(": tests.run")),
        "the failing result is a tests.run's: {out}"
    );
    let now = line_of(&out, "  may now: ").unwrap_or_else(|| panic!("no may now: line: {out}"));
    assert_eq!(lines[now], "  may now: +implementation.edit", "{out}");
    assert!(failing < now, "{out}");
    assert_eq!(
        out.lines()
            .filter(|line| line.starts_with("  may now: "))
            .count(),
        1,
        "a decision that does not change the admissible set prints nothing: {out}"
    );
    assert!(
        !out.contains('\x1b'),
        "no escape byte off a terminal: {out}"
    );
    assert!(
        out.lines()
            .last()
            .unwrap_or_default()
            .starts_with("stopped: "),
        "{out}"
    );
}

/// A run of an intent names the intent's first line in the header, and, as the story's
/// `expected_before` predicate reads it, prints no `frontier: … (admissible)` line.
#[test]
fn an_intent_run_names_its_first_line_and_keeps_the_frontier_quiet() {
    let world = world("intent");
    git(&world.repo, &["config", "user.name", "t"]);
    git(&world.repo, &["config", "user.email", "t@example.invalid"]);
    let repo = world.repo.to_str().expect("utf-8");
    let (code, out, err) = world.takt(&[
        "run",
        "--repo",
        repo,
        "--intent",
        "Remove a todo by its number\n\nThe list loses the todo it names.",
        "--harness",
        "none",
        "--forge",
        "none",
        "--test-cmd",
        "true",
        "--iterations",
        "2",
    ]);
    let step1 = line_of(&out, "step 1: tests.run")
        .unwrap_or_else(|| panic!("no step 1: exit {code}\nstdout {out}\nstderr {err}"));
    let header = line_of(
        &out,
        &format!(
            "takt {} · Remove a todo by its number",
            env!("CARGO_PKG_VERSION")
        ),
    )
    .unwrap_or_else(|| panic!("no header line: {out}"));
    let lines: Vec<&str> = out.lines().collect();
    assert!(header < step1, "{out}");
    assert_eq!(
        lines[header + 1],
        "  The list loses the todo it names.",
        "{out}"
    );
    assert!(
        !out.lines()
            .any(|line| line.starts_with("frontier: ") && line.contains("(admissible)")),
        "{out}"
    );
    assert!(
        line_of(&out, "  may: ").is_some_and(|may| may < step1),
        "{out}"
    );
}

/// `--verbose` prints the full `frontier: <action> (<status>), …` line at each decision.
#[test]
fn verbose_prints_the_full_frontier_line() {
    let world = world("verbose");
    let (code, out, err) = world.run(&["--verbose"]);
    let step1 = line_of(&out, "step 1:")
        .unwrap_or_else(|| panic!("no step 1: exit {code}\nstdout {out}\nstderr {err}"));
    let full = out
        .lines()
        .position(is_decision_frontier)
        .unwrap_or_else(|| panic!("no full frontier line under --verbose: {out}"));
    assert!(full < step1, "{out}");
    assert!(out.contains("tests.run (admissible)"), "{out}");
    assert!(
        out.lines()
            .filter(|line| is_decision_frontier(line))
            .count()
            > 1,
        "one at each decision: {out}"
    );
}
