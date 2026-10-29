//! `--hours` bounds a harness step, not only the gaps between steps: the run gives the harness
//! what is left of its hour budget, a step that outlives it is killed with what it started, its
//! change is discarded as for any failed step, and the run stops on the time budget
//! (`Stop::Hours`), not on the harness's failure. No live `claude` runs: the harness program is
//! a script that sleeps.

#[path = "support/own_home.rs"]
mod own_home;

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use takt::harness::{ClaudeCode, Harness, StepContext, StepReport};
use takt::validator::Validator;
use takt::{Run, RunOptions, Stop};

/// The hour budget of every run below: 3.6 s.
const HOURS: f64 = 0.001;

/// How long past its budget a run may take to stop: the kill, the sweep and the discard.
const GRACE: Duration = Duration::from_secs(10);

/// Runs `git <args>` in `tree`.
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

/// An empty scratch directory named for this process and `name`.
fn directory(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("hours-step-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(&dir).expect("scratch dir");
    Scratch(dir)
}

/// A committed repository on `main` with a README.
fn repository(name: &str) -> Scratch {
    let dir = directory(name);
    fs::write(dir.join("README.md"), "a story\n").expect("a file");
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

/// Whether process `pid` is still running: listed by `ps`, and not a zombie.
fn alive(pid: &str) -> bool {
    Command::new("ps")
        .args(["-o", "stat=", "-p", pid])
        .output()
        .is_ok_and(|output| {
            let state = String::from_utf8_lossy(&output.stdout);
            let state = state.trim();
            output.status.success() && !state.is_empty() && !state.starts_with('Z')
        })
}

/// A `claude` that writes a file into the tree it runs in, starts a `sleep 20` and waits for
/// it, its own pid and the sleep's written to `pids`.
fn sleeper(at: &Path, pids: &Path) -> PathBuf {
    let script = at.join("claude");
    fs::write(
        &script,
        format!(
            "#!/bin/sh\necho stray > stray.txt\nsleep 20 &\necho $$ $! > '{}'\nwait\n",
            pids.display()
        ),
    )
    .expect("a script");
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).expect("exec bit");
    script
}

/// A harness that writes a file into the tree and then sleeps 20 s, or what is left of the
/// run's hour budget when that is less, and fails as a timed-out child does; what it was given
/// is kept.
struct Sleeps {
    given: std::cell::Cell<Option<Option<Duration>>>,
}

impl Harness for Sleeps {
    fn name(&self) -> &str {
        "sleeps"
    }

    fn step(&self, context: &StepContext<'_>) -> anyhow::Result<StepReport> {
        self.given.set(Some(context.time_left));
        fs::write(context.tree.join("stray.txt"), "stray\n")?;
        let sleep = Duration::from_secs(20);
        std::thread::sleep(context.time_left.map_or(sleep, |left| left.min(sleep)));
        anyhow::bail!("sleeps timed out and was killed")
    }
}

/// A harness that would sleep 20 s is given what is left of a 3.6 s hour budget: the run stops
/// on the hour budget within the budget and the grace, and the file the step wrote is
/// discarded.
#[test]
fn a_harness_is_given_what_is_left_of_the_hour_budget() {
    let harness = Sleeps {
        given: std::cell::Cell::new(None),
    };
    let tree = repository("fake-tree");
    let validators: Vec<Box<dyn Validator>> = Vec::new();
    let started = Instant::now();
    let mut run =
        Run::open(&tree, options("fake-hours"), &harness, &validators).expect("the case opens");
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let elapsed = started.elapsed();
    let text = String::from_utf8(out).expect("utf-8");
    let budget = Duration::from_secs_f64(HOURS * 3600.0);
    let given = harness
        .given
        .get()
        .expect("the harness took a step")
        .expect("the harness is given a time");
    assert!(given <= budget, "{given:?} is more than the budget\n{text}");
    assert!(
        elapsed < budget + GRACE,
        "the run took {elapsed:?}, past {budget:?} and the grace\n{text}"
    );
    match stop {
        Ok(Stop::Hours(h)) => assert!((h - HOURS).abs() < f64::EPSILON, "{h}"),
        other => panic!("the run stops on the hour budget, not {other:?}\n{text}"),
    }
    assert!(
        !tree.join("stray.txt").exists(),
        "the step's file is discarded\n{text}"
    );
}

/// A run with no hour budget gives the harness no time: the step keeps its own timeout.
#[test]
fn without_an_hour_budget_the_harness_is_given_no_time() {
    let harness = Sleeps {
        given: std::cell::Cell::new(None),
    };
    let tree = repository("fake-no-hours");
    let validators: Vec<Box<dyn Validator>> = Vec::new();
    let mut options = options("fake-no-hours");
    options.hours = None;
    options.iterations = Some(1);
    let no_time = NoTime(&harness);
    let mut run = Run::open(&tree, options, &no_time, &validators).expect("opens");
    let stop = run.run(&mut Vec::new());
    assert!(stop.is_err(), "{stop:?}");
    assert_eq!(harness.given.get(), Some(None));
}

/// [`Sleeps`] that does not sleep: records what it was given and fails at once.
struct NoTime<'a>(&'a Sleeps);

impl Harness for NoTime<'_> {
    fn name(&self) -> &str {
        "no-time"
    }

    fn step(&self, context: &StepContext<'_>) -> anyhow::Result<StepReport> {
        self.0.given.set(Some(context.time_left));
        anyhow::bail!("no step")
    }
}

/// The claude harness, held to its own 30-minute step timeout, runs a `claude` that sleeps
/// 20 s under a 3.6 s hour budget: the run stops on the hour budget within the budget and the
/// grace, the child and its sleep are killed and the file it wrote is discarded.
#[test]
fn a_claude_step_past_the_hour_budget_is_killed_and_the_run_stops_on_hours() {
    let scripts = directory("claude-scripts");
    let pids = scripts.join("pids");
    let program = sleeper(&scripts, &pids);
    let harness = ClaudeCode {
        program: program.display().to_string(),
        ..ClaudeCode::default()
    };
    assert_eq!(harness.limits.timeout, Duration::from_secs(1800));
    let tree = repository("claude-tree");
    let validators: Vec<Box<dyn Validator>> = Vec::new();
    let started = Instant::now();
    let mut run =
        Run::open(&tree, options("claude-hours"), &harness, &validators).expect("the case opens");
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let elapsed = started.elapsed();
    let text = String::from_utf8(out).expect("utf-8");
    let budget = Duration::from_secs_f64(HOURS * 3600.0);
    assert!(
        elapsed < budget + GRACE,
        "the run took {elapsed:?}, past {budget:?} and the grace\n{text}"
    );
    match stop {
        Ok(Stop::Hours(h)) => assert!((h - HOURS).abs() < f64::EPSILON, "{h}"),
        other => panic!("the run stops on the hour budget, not {other:?}\n{text}"),
    }
    assert!(
        !tree.join("stray.txt").exists(),
        "the killed step's file is discarded\n{text}"
    );
    let pids = fs::read_to_string(&pids).expect("the step started");
    for pid in pids.split_whitespace() {
        assert!(!alive(pid), "process {pid} of the step still runs\n{text}");
    }
}

/// A test command that sleeps 20 s under a 3.6 s hour budget is held to what is left of it: the
/// run stops on the hour budget within the budget and the grace, no `test_result` is recorded,
/// and the run record says the budget ended the observation.
#[test]
fn a_test_command_past_the_hour_budget_is_killed_and_records_nothing() {
    let tree = repository("validator-tree");
    let script = tree.join("check.sh");
    fs::write(&script, "#!/bin/sh\nsleep 20\nexit 0\n").expect("a script");
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).expect("exec bit");
    git(&tree, &["add", "-A"]);
    git(&tree, &["commit", "-q", "-m", "check"]);
    let harness = Sleeps {
        given: std::cell::Cell::new(None),
    };
    let validators: Vec<Box<dyn Validator>> = vec![Box::new(
        takt::validator::TestsRun::new("./check.sh").expect("a command"),
    )];
    let started = Instant::now();
    let mut run =
        Run::open(&tree, options("validator-hours"), &harness, &validators).expect("opens");
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let elapsed = started.elapsed();
    let text = String::from_utf8(out).expect("utf-8");
    let budget = Duration::from_secs_f64(HOURS * 3600.0);
    assert!(
        elapsed < budget + GRACE,
        "the run took {elapsed:?}, past {budget:?} and the grace\n{text}"
    );
    assert!(matches!(stop, Ok(Stop::Hours(_))), "{stop:?}\n{text}");
    assert_eq!(
        harness.given.get(),
        None,
        "no judgement step was taken\n{text}"
    );
    let case = tree.join(".takt/cases/validator-hours");
    let records = fs::read_dir(case.join("evidence")).map_or(0, Iterator::count);
    assert_eq!(records, 0, "nothing is recorded\n{text}");
    let events = fs::read_to_string(case.join("events.jsonl")).expect("the run record");
    assert!(
        events
            .lines()
            .any(|line| line.contains("\"refusal\"") && line.contains("hour budget")),
        "{events}"
    );
}
