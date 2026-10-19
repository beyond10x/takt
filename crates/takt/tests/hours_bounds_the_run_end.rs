//! `--hours` ends the run at one instant, the run's own: a validator is given that instant, a
//! gate of several commands that runs past it is caught when it returns and records nothing, a
//! step discarded there keeps its step line so the steps' tokens add up to the run's, and what
//! of the git directory could not be put back is named. Every harness and validator is a
//! double; nothing reaches a model. One case at a time: a run sweeps what left the test
//! process's own group, a sibling case's gate command among it.

use std::cell::Cell;
use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;
use std::time::{Duration, Instant};

use takt::harness::{Harness, StepContext, StepReport};
use takt::validator::{GateRun, Observation, Validator};
use takt::{Run, RunOptions, Stop};

/// The hour budget of the runs below but the gate's: 3.6 s.
const HOURS: f64 = 0.001;

/// How long past its budget a run may take to stop: the kill, the sweep and the discard.
const GRACE: Duration = Duration::from_secs(10);

/// The gate-only protocol: `gate.run` is its one action.
const GATE_ONLY: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/gate-only/1.yaml"
);

fn serial() -> std::sync::MutexGuard<'static, ()> {
    static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());
    SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn budget(hours: f64) -> Duration {
    Duration::from_secs_f64(hours * 3600.0)
}

fn git(tree: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(args)
        .current_dir(tree)
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?}");
}

/// A scratch directory under this crate's build directory, removed when dropped.
struct Scratch(PathBuf);

impl std::ops::Deref for Scratch {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            let message = format!("remove the scratch tree {}: {error}", self.0.display());
            if std::thread::panicking() {
                eprintln!("{message}");
            } else {
                panic!("{message}");
            }
        }
    }
}

fn directory(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("hours-end-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(&dir).expect("scratch dir");
    Scratch(dir)
}

/// A committed repository on `main` with a README and `files`, executable when their name ends
/// in `.sh`.
fn repository(name: &str, files: &[(&str, &str)]) -> Scratch {
    let dir = directory(name);
    fs::write(dir.join("README.md"), "a story\n").expect("a file");
    for (path, text) in files {
        let path = dir.join(path);
        fs::create_dir_all(path.parent().expect("a parent")).expect("a parent directory");
        fs::write(&path, text).expect("a file");
        if path.extension().is_some_and(|e| e == "sh") {
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("exec bit");
        }
    }
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "start"]);
    dir
}

fn options(id: &str) -> RunOptions {
    RunOptions {
        protocol: "software.change/1".to_owned(),
        intent: "probe".to_owned(),
        inputs: vec![
            ("risk".to_owned(), "standard".to_owned()),
            ("change_kind".to_owned(), "bugfix".to_owned()),
            ("affects_security_boundary".to_owned(), "false".to_owned()),
        ],
        iterations: Some(3),
        hours: Some(HOURS),
        tokens: None,
        case_id: Some(id.to_owned()),
        gate: Some("repository.merge".to_owned()),
    }
}

/// Kills every pid listed in `file`, if it exists.
fn kill_listed(file: &Path) {
    if let Ok(text) = fs::read_to_string(file) {
        for pid in text.split_whitespace() {
            let _ = Command::new("kill").args(["-9", pid]).status();
        }
    }
}

/// The lines of the case's `events.jsonl`, parsed.
fn events(tree: &Path, id: &str) -> Vec<serde_json::Value> {
    fs::read_to_string(
        tree.join(".engineering/cases")
            .join(id)
            .join("events.jsonl"),
    )
    .expect("the run record")
    .lines()
    .map(|line| serde_json::from_str(line).expect("a JSON line"))
    .collect()
}

/// How many evidence records the case holds.
fn records(tree: &Path, id: &str) -> usize {
    fs::read_dir(tree.join(".engineering/cases").join(id).join("evidence"))
        .map_or(0, Iterator::count)
}

/// A harness that fails at once.
struct Fails;

impl Harness for Fails {
    fn name(&self) -> &str {
        "fails"
    }

    fn step(&self, _: &StepContext<'_>) -> anyhow::Result<StepReport> {
        anyhow::bail!("fails: no step")
    }
}

/// A gate of three commands that each sleep 25 s, under a 3 s hour budget: every command is
/// held to what was left when the gate started collecting, so the gate runs past the run's end
/// by up to twice that, and is caught when it returns. The run stops on the hour budget, and
/// nothing is recorded.
#[test]
fn a_gate_of_three_commands_past_the_hour_budget_stops_on_hours_and_records_nothing() {
    let _serial = serial();
    let scripts = directory("gate-scripts");
    let pids = scripts.join("pids");
    let tree = repository(
        "gate",
        &[
            (
                ".engineering/gates.yaml",
                "format: engineering-gates/1\nlanguage: canon-expr/1\ngates:\n  - id: check\n    \
                 text: The checks hold.\n    assertions:\n      - 'process.succeeded(\"a\")'\n      \
                 - 'process.succeeded(\"b\")'\n      - 'process.succeeded(\"c\")'\n",
            ),
            (
                ".engineering/assertions-project.yaml",
                "format: engineering-assertions-project/1\ntimeout_seconds: 40\ncommands:\n  \
                 a:\n    executable: check.sh\n  b:\n    executable: check.sh\n  c:\n    \
                 executable: check.sh\n",
            ),
            (
                "check.sh",
                &format!(
                    "#!/bin/sh\necho $$ >> '{}'\nexec sleep 25\n",
                    pids.display()
                ),
            ),
        ],
    );
    let hours = 3.0 / 3600.0;
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(GateRun::new(".engineering/gates.yaml"))];
    let mut options = options("gate-hours");
    options.protocol = GATE_ONLY.to_owned();
    options.inputs = Vec::new();
    options.gate = None;
    options.hours = Some(hours);
    let started = Instant::now();
    let mut run = Run::open(&tree, options, &Fails, &validators).expect("opens");
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let elapsed = started.elapsed();
    kill_listed(&pids);
    let text = String::from_utf8_lossy(&out);
    let ran = fs::read_to_string(&pids).unwrap_or_default();
    assert!(
        !ran.trim().is_empty(),
        "the premise: the gate ran its commands\n{stop:?}\n{text}"
    );
    assert!(
        elapsed < budget(hours) * 3 + GRACE,
        "the run took {elapsed:?}\n{stop:?}\n{text}"
    );
    assert!(matches!(stop, Ok(Stop::Hours(_))), "{stop:?}\n{text}");
    assert_eq!(
        records(&tree, "gate-hours"),
        0,
        "nothing is recorded\n{text}"
    );
}

/// What [`Clocked`] was given: nothing before it observes.
type Given = Rc<Cell<Option<Option<Instant>>>>;

/// A validator that keeps the deadline it was given and observes nothing.
struct Clocked {
    given: Given,
}

impl Validator for Clocked {
    fn name(&self) -> &str {
        "takt.clocked"
    }

    fn action(&self) -> &str {
        "tests.run"
    }

    fn kind(&self) -> &str {
        "test_result"
    }

    fn subject(&self) -> &str {
        "implementation"
    }

    fn observe(&self, _: &Path, _: &str, _: usize) -> anyhow::Result<Observation> {
        Ok(Observation {
            summary: "nothing observed".to_owned(),
            records: Vec::new(),
            artifact: None,
        })
    }

    fn observe_within(
        &self,
        tree: &Path,
        revision: &str,
        seq: usize,
        deadline: Option<Instant>,
    ) -> anyhow::Result<(Observation, bool)> {
        self.given.set(Some(deadline));
        Ok((self.observe(tree, revision, seq)?, false))
    }
}

/// A validator is given the run's own deadline, the instant the run's clock started and the
/// budget, not one it works out later from what is left: the run and the validator judge
/// "returned at or after the run's end" against the same instant.
#[test]
fn a_validator_is_given_the_runs_own_deadline() {
    let _serial = serial();
    let tree = repository("clocked", &[]);
    let given: Given = Rc::new(Cell::new(None));
    let validators: Vec<Box<dyn Validator>> = vec![Box::new(Clocked {
        given: given.clone(),
    })];
    let before_open = Instant::now();
    let mut run = Run::open(&tree, options("clocked-run"), &Fails, &validators).expect("opens");
    let after_open = Instant::now();
    let _ = run.run(&mut Vec::new());
    let given = given
        .get()
        .expect("the validator observed")
        .expect("the validator is given a deadline");
    assert!(
        given >= before_open + budget(HOURS) && given <= after_open + budget(HOURS),
        "the deadline given is not the run's: {:?} after the latest it could be",
        given.saturating_duration_since(after_open + budget(HOURS))
    );
}

/// A harness that runs `before` on the tree, then returns success, reporting `tokens`, a second
/// past what was left of the hour budget when it was called.
struct Late {
    tokens: u64,
    before: fn(&Path),
}

impl Harness for Late {
    fn name(&self) -> &str {
        "late"
    }

    fn step(&self, context: &StepContext<'_>) -> anyhow::Result<StepReport> {
        let called = Instant::now();
        (self.before)(context.tree);
        let left = context.time_left.expect("a time is given");
        std::thread::sleep(
            (called + left + Duration::from_secs(1)).saturating_duration_since(Instant::now()),
        );
        Ok(StepReport {
            action: context.offered.first().cloned().unwrap_or_default(),
            summary: "late".to_owned(),
            tokens: Some(self.tokens),
        })
    }
}

/// A step that reports 123 tokens a second past the run's end is discarded, and keeps its step
/// line with its tokens and a summary saying so: the steps' tokens add up to the stop's.
#[test]
fn the_steps_tokens_add_up_to_the_stops_when_the_last_step_is_discarded_at_the_run_end() {
    let _serial = serial();
    let tree = repository("tokens", &[]);
    let harness = Late {
        tokens: 123,
        before: |tree| fs::write(tree.join("late.txt"), "late\n").expect("a file"),
    };
    let validators: Vec<Box<dyn Validator>> = Vec::new();
    let mut run = Run::open(&tree, options("tokens-run"), &harness, &validators).expect("opens");
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let text = String::from_utf8_lossy(&out);
    assert!(matches!(stop, Ok(Stop::Hours(_))), "{stop:?}\n{text}");
    assert!(
        !tree.join("late.txt").exists(),
        "the step is discarded\n{text}"
    );
    let events = events(&tree, "tokens-run");
    let of = |kind: &str| -> Vec<&serde_json::Value> {
        events.iter().filter(|e| e["event"] == kind).collect()
    };
    let steps: u64 = of("step")
        .iter()
        .map(|e| e["tokens"].as_u64().unwrap_or(0))
        .sum();
    let stops = of("stop");
    assert_eq!(stops.len(), 1, "{events:?}");
    assert_eq!(stops[0]["tokens"].as_u64(), Some(123), "{events:?}");
    assert_eq!(
        steps, 123,
        "the steps' tokens add up to the stop's: {events:?}"
    );
    assert!(
        of("step").iter().any(|e| e["summary"]
            .as_str()
            .is_some_and(|s| s.contains("discarded at the run's end"))),
        "{events:?}"
    );
}

/// A step that leaves `.git/index.lock` as a directory takt cannot remove, and returns past the
/// run's end: the put-back of the git directory fails there, and whichever way the run ends,
/// on the hour budget or on the error of a discard that `git` then refuses, its line names
/// what was not put back.
#[test]
fn a_git_directory_not_put_back_at_the_run_end_is_named() {
    let _serial = serial();
    let tree = repository("lock", &[]);
    let harness = Late {
        tokens: 1,
        before: |tree| {
            let inner = tree.join(".git/index.lock/inner");
            fs::create_dir_all(&inner).expect("a directory");
            fs::write(inner.join("f"), "x\n").expect("a file");
            fs::set_permissions(&inner, fs::Permissions::from_mode(0o000)).expect("a mode");
        },
    };
    let validators: Vec<Box<dyn Validator>> = Vec::new();
    let mut run = Run::open(&tree, options("lock-run"), &harness, &validators).expect("opens");
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let inner = tree.join(".git/index.lock/inner");
    if inner.exists() {
        fs::set_permissions(&inner, fs::Permissions::from_mode(0o755)).expect("a mode");
    }
    let text = String::from_utf8_lossy(&out);
    let said = match &stop {
        Ok(Stop::Hours(_)) => text.to_string(),
        Err(error) => format!("{error:#}"),
        other => {
            panic!("the run stops on the hour budget or its discard's error, not {other:?}\n{text}")
        }
    };
    assert!(
        said.contains("not put back") && said.contains("index.lock"),
        "{said}\n{text}"
    );
}
