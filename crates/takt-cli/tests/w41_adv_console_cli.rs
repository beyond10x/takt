//! Wave 41 adversarial pass on the run console through the binary: text from an intent, a story
//! or a harness's `--version` that reaches standard output off a terminal, and the console's
//! line shapes against the one reader of takt's console logs in this repository,
//! `docs/metrics/journey.py` (`STEP_RE` and `STOP_RE`, lines 121-122), which reads a scenario
//! run's `run.log` into `journeys.jsonl`.

#[path = "../../takt/tests/support/own_home.rs"]
mod own_home;

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../takt/tests/fixtures");

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

struct World {
    _scratch: Scratch,
    repo: PathBuf,
    bin: PathBuf,
    state: PathBuf,
}

/// The fixture store and scenario 1's code; `outcome` in place of the ready story's Outcome
/// paragraph when given; a `claude` double whose `--version` prints `version` and which writes a
/// failing test at its first call and declines after.
fn world(name: &str, outcome: Option<&str>, version: &str) -> World {
    let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("w41-adv-console-{}-{name}", std::process::id()));
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
    if let Some(outcome) = outcome {
        let story = repo.join(".engineering/planning/story/ready-one.md");
        let text = fs::read_to_string(&story).expect("the story");
        let text = text.replace(
            "The check in `check.txt` passes for whoever runs `./check.sh`.",
            outcome,
        );
        fs::write(&story, text).expect("the story is written");
    }
    fs::write(repo.join("Cargo.toml"), "[package]\nname = \"scratch\"\n").expect("a file");
    fs::write(repo.join(".engineering/gates.yaml"), GATES).expect("a file");
    fs::write(repo.join(".gitignore"), "__pycache__/\n").expect("a file");
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["config", "user.name", "t"]);
    git(&repo, &["config", "user.email", "t@example.invalid"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "seed"]);
    let bin = outer.join("stub-bin");
    fs::create_dir_all(&bin).expect("a stub directory");
    let state = outer.join("state");
    fs::create_dir_all(&state).expect("a state directory");
    let calls = outer.join("calls");
    fs::write(
        bin.join("claude"),
        format!(
            r#"#!/bin/sh
case $1 in --version) printf '%s\n' '{version}'; exit 0 ;; esac
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
    fs::set_permissions(bin.join("claude"), fs::Permissions::from_mode(0o755)).expect("executable");
    World {
        _scratch: Scratch(outer),
        repo,
        bin,
        state,
    }
}

impl World {
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

    /// `takt run` of an intent with the `none` harness, as the stories' predicates run it.
    fn intent(&self, intent: &str) -> (i32, String, String) {
        let repo = self.repo.to_str().expect("utf-8");
        self.takt(&[
            "run",
            "--repo",
            repo,
            "--intent",
            intent,
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
        ])
    }

    /// `takt run` of the ready story with the `claude` double.
    fn story(&self) -> (i32, String, String) {
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
            "--min-free-gib",
            "0",
            "--iterations",
            "6",
        ])
    }
}

/// Story acceptance 4 (run-console-opens-with-the-story): off a terminal the output holds no
/// escape byte. An intent taken from an issue or a pasted log carries one in its first line and
/// its paragraph.
#[test]
fn w41_adv_an_intent_escape_byte_reaches_no_pipe() {
    let world = world("intent-esc", None, "2.1.309 (Claude Code)");
    let (code, out, err) = world.intent(
        "Remove a \x1b[2J\x1b[Htodo by its number\n\nThe \x1b[31mlist\x1b[0m loses the todo it names.",
    );
    assert!(
        out.lines()
            .any(|line| line.starts_with("step 1: tests.run")),
        "the run took no first step: exit {code}\nstdout {out}\nstderr {err}"
    );
    let raw: Vec<&str> = out.lines().filter(|line| line.contains('\x1b')).collect();
    assert!(
        raw.is_empty(),
        "lines carrying an escape byte to a pipe: {raw:?}"
    );
}

/// A lone carriage return in an intent's first line: on a terminal everything after it is drawn
/// over the header, so the header reads as a `stopped:` line before the first step.
#[test]
fn w41_adv_a_carriage_return_in_the_intent_reaches_no_pipe() {
    let world = world("intent-cr", None, "2.1.309 (Claude Code)");
    let (code, out, err) = world.intent(
        "Remove a todo\rstopped: Completed; steps 0, tokens 0, case /nowhere\n\nThe list loses it.",
    );
    assert!(
        out.lines()
            .any(|line| line.starts_with("step 1: tests.run")),
        "the run took no first step: exit {code}\nstdout {out}\nstderr {err}"
    );
    let raw: Vec<&str> = out.lines().filter(|line| line.contains('\r')).collect();
    assert!(raw.is_empty(), "lines carrying a carriage return: {raw:?}");
}

/// A story whose Outcome quotes coloured tool output: the summary carries the escape bytes.
#[test]
fn w41_adv_a_story_outcome_escape_byte_reaches_no_pipe() {
    let world = world(
        "story-esc",
        Some("The check prints \x1b[32mok\x1b[0m for whoever runs `./check.sh`."),
        "2.1.309 (Claude Code)",
    );
    let (code, out, err) = world.story();
    assert!(
        out.lines().any(|line| line.starts_with("step 1:")),
        "the run took no first step: exit {code}\nstdout {out}\nstderr {err}"
    );
    let raw: Vec<&str> = out.lines().filter(|line| line.contains('\x1b')).collect();
    assert!(
        raw.is_empty(),
        "lines carrying an escape byte to a pipe: {raw:?}"
    );
}

/// Green probe: the harness line prints the version preflight read from the harness program; a
/// program whose `--version` is coloured reads as no version (the run stops
/// `HarnessUnqualified`), and no escape byte reaches the output.
#[test]
fn w41_adv_a_coloured_harness_version_reaches_no_pipe() {
    let world = world("version-esc", None, "\x1b[1m2.1.309\x1b[0m (Claude Code)");
    let (code, out, err) = world.story();
    assert!(
        out.lines()
            .any(|line| line.starts_with("  harness claude-code")),
        "no harness line: exit {code}\nstdout {out}\nstderr {err}"
    );
    let raw: Vec<&str> = out.lines().filter(|line| line.contains('\x1b')).collect();
    assert!(
        raw.is_empty(),
        "lines carrying an escape byte to a pipe: {raw:?}"
    );
}

const JOURNEY: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/metrics/journey.py");

/// The line of `docs/metrics/journey.py` that [`journey_stop`] models.
const STOP_RE: &str =
    r#"STOP_RE = re.compile(r"^stopped: (.*?); steps \d+, tokens \d+(?:, wall .*?)?, case ")"#;

/// The line of `docs/metrics/journey.py` that [`journey_step`] models.
const STEP_RE: &str =
    r#"STEP_RE = re.compile(r"^step \d+: (\S+) (\{.*\}|\((.+)\))(?: · .*)?\s*$")"#;

/// Whether `docs/metrics/journey.py` holds `pattern` as one of its lines: a model of a pattern
/// the script no longer holds models nothing.
fn journey_holds(pattern: &str) -> bool {
    fs::read_to_string(JOURNEY)
        .expect("docs/metrics/journey.py")
        .lines()
        .any(|line| line == pattern)
}

/// [`STOP_RE`], as `re.match` applies it: from the line's start, `, case ` right after the
/// tokens' digits, or `, wall ` there and a later `, case `.
fn journey_stop(line: &str) -> bool {
    let Some(rest) = line.strip_prefix("stopped: ") else {
        return false;
    };
    rest.match_indices("; steps ").any(|(at, _)| {
        let after = &rest[at + "; steps ".len()..];
        let digits = after.chars().take_while(char::is_ascii_digit).count();
        if digits == 0 {
            return false;
        }
        let Some(after) = after[digits..].strip_prefix(", tokens ") else {
            return false;
        };
        let digits = after.chars().take_while(char::is_ascii_digit).count();
        if digits == 0 {
            return false;
        }
        let after = &after[digits..];
        after.starts_with(", case ")
            || after
                .strip_prefix(", wall ")
                .is_some_and(|wall| wall.contains(", case "))
    })
}

/// [`STEP_RE`]: a step number, an action, one space, then `{…}` or `(…)`, and after it nothing
/// but spaces or ` · ` and anything.
fn journey_step(line: &str) -> bool {
    let Some(rest) = line.strip_prefix("step ") else {
        return false;
    };
    let digits = rest.chars().take_while(char::is_ascii_digit).count();
    if digits == 0 {
        return false;
    }
    let Some(rest) = rest[digits..].strip_prefix(": ") else {
        return false;
    };
    let Some((action, tail)) = rest.split_once(' ') else {
        return false;
    };
    if action.is_empty() {
        return false;
    }
    let (open, close) = match tail.chars().next() {
        Some('{') => ('{', '}'),
        Some('(') => ('(', ')'),
        _ => return false,
    };
    // The shortest `(…)` holds one character; `{}` may be empty.
    let least = if open == '(' { 2 } else { 1 };
    tail.match_indices(close).any(|(at, _)| {
        let after = &tail[at + close.len_utf8()..];
        at >= least && (after.trim_end().is_empty() || after.starts_with(" · "))
    })
}

/// The two models read the lines before wave 41 and after it as `re.match` of the script's
/// patterns does (checked by hand with Python 3 on these lines), and not the working line.
#[test]
fn w41_adv_the_journey_models_read_old_and_new_lines() {
    for line in [
        "step 1: tests.run {}",
        "step 1: tests.run {} · pass · 0.3s",
        "step 3: gate.run {} · fail: a}b, c · 0.1s",
        "step 2: tests.write (claude-code)",
        "step 2: tests.write (claude-code) · 26.1s · 65.7k tok · $0.16",
    ] {
        assert!(journey_step(line), "{line}");
    }
    for line in [
        "step 2: none working (may: tests.run)",
        "step 2: tests.write ()",
        "step 2: tests.write {} pass",
    ] {
        assert!(!journey_step(line), "{line}");
    }
    assert!(journey_stop(
        "stopped: Completed; steps 2, tokens 0, case /x/y"
    ));
    assert!(journey_stop(
        "stopped: NoProgress; steps 2, tokens 10, wall 1.2s, cost $0.89, commits 2 (2 paths), \
         case /x/y"
    ));
    assert!(!journey_stop(
        "stopped: NoProgress; steps 2, tokens 10, commits 2, case /x/y"
    ));
}

/// `docs/metrics/journey.py` finds a scenario run's invocation stops by `STOP_RE`; the console's
/// `stopped:` line puts its wall time and commits between the tokens and `, case `, so the stop
/// is not found and the run's invocations and stops drop out of `journeys.jsonl`.
#[test]
fn w41_adv_journey_reads_the_stopped_line() {
    let world = world("journey-stop", None, "2.1.309 (Claude Code)");
    let (code, out, err) = world.intent("Remove a todo by its number");
    let last = out.lines().last().unwrap_or_default();
    assert!(
        last.starts_with("stopped: "),
        "exit {code}\nstdout {out}\nstderr {err}"
    );
    assert!(
        journey_holds(STOP_RE),
        "docs/metrics/journey.py holds no line {STOP_RE}: the model below models nothing"
    );
    assert!(
        journey_stop(last),
        "docs/metrics/journey.py:122 STOP_RE does not match the stop line: {last}"
    );
}

/// `docs/metrics/journey.py` finds each step by `STEP_RE`; a validator step's line now ends
/// with its result and time after `{}`, so no validator step is found.
#[test]
fn w41_adv_journey_reads_the_step_lines() {
    let world = world("journey-step", None, "2.1.309 (Claude Code)");
    let (code, out, err) = world.intent("Remove a todo by its number");
    let steps: Vec<&str> = out
        .lines()
        .filter(|line| line.starts_with("step ") && !line.contains(" working (may: "))
        .collect();
    assert!(
        !steps.is_empty(),
        "no step line: exit {code}\nstdout {out}\nstderr {err}"
    );
    assert!(
        journey_holds(STEP_RE),
        "docs/metrics/journey.py holds no line {STEP_RE}: the model below models nothing"
    );
    let unread: Vec<&&str> = steps.iter().filter(|line| !journey_step(line)).collect();
    assert!(
        unread.is_empty(),
        "docs/metrics/journey.py:121 STEP_RE does not match these step lines: {unread:?}"
    );
}

/// Green probe: the `stopped:` line is the last line and its case directory follows the last
/// `, case `, for a story run with the double.
#[test]
fn w41_adv_the_stop_line_is_last_and_names_the_case() {
    let world = world("stop-last", None, "2.1.309 (Claude Code)");
    let (code, out, err) = world.story();
    let last = out.lines().last().unwrap_or_default();
    assert!(
        last.starts_with("stopped: "),
        "exit {code}\nstdout {out}\nstderr {err}"
    );
    let (_, case) = last.rsplit_once(", case ").expect("a case");
    assert!(Path::new(case).join("events.jsonl").is_file(), "{last}");
}
