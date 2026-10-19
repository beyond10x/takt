//! Adversary cases for `--hours` bounding a harness step. Every harness is a double: a Rust
//! fake or a `claude` script that sleeps; nothing reaches a model. Every process a case starts
//! is killed before it ends.

use std::cell::Cell;
use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use takt::harness::{ClaudeCode, Harness, StepContext, StepReport};
use takt::validator::{TestsRun, Validator};
use takt::{Run, RunOptions, Stop};

/// The hour budget of every run below: 3.6 s.
const HOURS: f64 = 0.001;

/// How long past its budget a run may take to stop, as the acceptance allows.
const GRACE: Duration = Duration::from_secs(10);

fn budget() -> Duration {
    Duration::from_secs_f64(HOURS * 3600.0)
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
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn directory(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("hours-adv1-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(&dir).expect("scratch dir");
    Scratch(dir)
}

fn executable(path: &Path, text: &str) {
    fs::write(path, text).expect("a script");
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("exec bit");
}

/// A committed repository on `main` with a README and `check.sh` holding `check`.
fn repository(name: &str, check: &str) -> Scratch {
    let dir = directory(name);
    fs::write(dir.join("README.md"), "a story\n").expect("a file");
    executable(&dir.join("check.sh"), check);
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

/// Whether process `pid` is still running: listed, and not a zombie.
fn alive(pid: &str) -> bool {
    fs::read_to_string(format!("/proc/{pid}/stat"))
        .ok()
        .and_then(|stat| {
            stat.rsplit_once(") ")
                .map(|(_, rest)| !rest.starts_with('Z'))
        })
        .unwrap_or(false)
}

/// Kills every pid listed in `file`, if it exists.
fn kill_listed(file: &Path) {
    if let Ok(text) = fs::read_to_string(file) {
        for pid in text.split_whitespace() {
            let _ = Command::new("kill").args(["-9", pid]).status();
        }
    }
}

/// A harness that keeps the instant it was called and the time it was given, then fails.
struct Clocked {
    seen: Cell<Option<(Instant, Option<Duration>)>>,
}

impl Harness for Clocked {
    fn name(&self) -> &str {
        "clocked"
    }

    fn step(&self, context: &StepContext<'_>) -> anyhow::Result<StepReport> {
        self.seen.set(Some((Instant::now(), context.time_left)));
        anyhow::bail!("clocked: no step")
    }
}

/// The run's own clock starts inside `Run::open` (`started`), so the run's end is no later
/// than `after_open + budget`. A harness called at `call` and given `given` holds its child
/// until `call + given`: past the run's end by however long the step's preparation took
/// (snapshots, the checkpoint's git calls), because `time_left` is read before that
/// preparation (run.rs:3774) and the child's timeout counts from its own spawn
/// (forge.rs:645). The story's outcome: "no step runs past the run's end".
#[test]
fn the_time_a_harness_is_given_is_read_before_the_step_preparation_and_overshoots_the_run_end() {
    let tree = repository("stale", "#!/bin/sh\nexit 1\n");
    let harness = Clocked {
        seen: Cell::new(None),
    };
    let validators: Vec<Box<dyn Validator>> = Vec::new();
    let mut run = Run::open(&tree, options("adv-stale"), &harness, &validators).expect("opens");
    let after_open = Instant::now();
    let _ = run.run(&mut Vec::new());
    let (call, given) = harness.seen.get().expect("the harness took a step");
    let given = given.expect("the harness is given a time");
    let held_until = call + given;
    let run_end = after_open + budget();
    assert!(
        held_until <= run_end,
        "the harness is held until {:?} past the run's end: given {given:?} when {:?} was left",
        held_until.duration_since(run_end),
        run_end.saturating_duration_since(call)
    );
}

/// A harness that changes many files, then fails for its own reason before the budget is
/// spent.
struct FailsEarly {
    fail_at: Cell<Option<Instant>>,
    failed_at: Cell<Option<Instant>>,
}

impl Harness for FailsEarly {
    fn name(&self) -> &str {
        "fails-early"
    }

    fn step(&self, context: &StepContext<'_>) -> anyhow::Result<StepReport> {
        for d in 0..20 {
            let dir = context.tree.join("gen").join(d.to_string());
            fs::create_dir_all(&dir)?;
            for f in 0..1000 {
                fs::write(dir.join(format!("{f}.txt")), "x\n")?;
            }
        }
        let fail_at = self.fail_at.get().expect("set before the run");
        std::thread::sleep(fail_at.saturating_duration_since(Instant::now()));
        self.failed_at.set(Some(Instant::now()));
        anyhow::bail!("fails-early: not logged in")
    }
}

/// A harness that fails for a reason of its own (not logged in) 100 ms before the budget is
/// spent, its step having changed 20000 files, is reported as the hour budget: the run checks
/// `time_left() == 0` after the sweep, the put-back of the git directory and the snapshots
/// (run.rs:3939), not whether the budget is what ended the step. Decision 1 of the brief: a
/// step the budget kills stops on `Hours`; one that fails by itself is "the harness failed".
#[test]
fn a_harness_that_fails_by_itself_before_the_budget_ends_is_reported_as_the_hour_budget() {
    let tree = repository("early", "#!/bin/sh\nexit 1\n");
    let harness = FailsEarly {
        fail_at: Cell::new(None),
        failed_at: Cell::new(None),
    };
    let validators: Vec<Box<dyn Validator>> = Vec::new();
    let mut run = Run::open(&tree, options("adv-early"), &harness, &validators).expect("opens");
    let after_open = Instant::now();
    let run_end = after_open + budget();
    harness
        .fail_at
        .set(Some(run_end - Duration::from_millis(100)));
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let text = String::from_utf8_lossy(&out);
    let failed_at = harness.failed_at.get().expect("the harness took a step");
    assert!(
        failed_at < run_end,
        "the premise: the harness failed before the run's end\n{text}"
    );
    match stop {
        Err(error) => assert!(format!("{error:#}").contains("not logged in"), "{error:#}"),
        Ok(stop) => panic!(
            "a harness that failed {:?} before the budget was spent stopped the run on {stop:?}, not on its own failure\n{text}",
            run_end.duration_since(failed_at)
        ),
    }
}

/// A `claude` that writes a file, starts a `sleep 20` and waits, its pids in `pids`.
fn sleeper(at: &Path, pids: &Path) -> PathBuf {
    let script = at.join("claude");
    executable(
        &script,
        &format!(
            "#!/bin/sh\necho stray > stray.txt\nsleep 20 &\necho $$ $! > '{}'\nwait\n",
            pids.display()
        ),
    );
    script
}

/// A step the hour budget killed leaves no trace in `events.jsonl`: no `step`, `refusal` or
/// `error` line says a step ran and was killed, only the `stop` line counting it among the
/// steps (run.rs:3936-3954 writes to `out` and emits nothing). `Run::run` promises one event
/// per step line it prints, and a step whose harness failed otherwise ends on an `error` line
/// naming why.
#[test]
fn a_step_the_hour_budget_killed_is_missing_from_the_run_record() {
    let scripts = directory("record-scripts");
    let pids = scripts.join("pids");
    let program = sleeper(&scripts, &pids);
    let harness = ClaudeCode {
        program: program.display().to_string(),
        ..ClaudeCode::default()
    };
    let tree = repository("record", "#!/bin/sh\nexit 1\n");
    let validators: Vec<Box<dyn Validator>> = Vec::new();
    let mut run = Run::open(&tree, options("adv-record"), &harness, &validators).expect("opens");
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    kill_listed(&pids);
    let text = String::from_utf8_lossy(&out).into_owned();
    assert!(matches!(stop, Ok(Stop::Hours(_))), "{stop:?}\n{text}");
    assert!(text.contains("hour budget ran out"), "{text}");
    let events = fs::read_to_string(tree.join(".engineering/cases/adv-record/events.jsonl"))
        .expect("the run record");
    let explained = events.lines().any(|line| {
        !line.contains("\"stop\"") && (line.contains("timed out") || line.contains("hour budget"))
    });
    assert!(
        explained,
        "the run record says nothing of the step the budget killed:\n{events}"
    );
}

/// A `claude` whose background child ignores SIGTERM, leaves into a session of its own,
/// holds the output pipe and writes into the tree 5 s later; the probe that the sweep reaches
/// it before the discard.
#[test]
fn a_setsid_grandchild_that_ignores_sigterm_and_holds_the_pipe_is_killed_before_the_discard() {
    let scripts = directory("setsid-scripts");
    let pids = scripts.join("pids");
    let program = scripts.join("claude");
    executable(
        &program,
        &format!(
            "#!/bin/sh\nsetsid sh -c 'trap \"\" TERM; echo $$ >> {p}; sleep 5; echo late > late.txt' &\necho $$ >> {p}\nsleep 20\n",
            p = pids.display()
        ),
    );
    let harness = ClaudeCode {
        program: program.display().to_string(),
        ..ClaudeCode::default()
    };
    let tree = repository("setsid", "#!/bin/sh\nexit 1\n");
    let validators: Vec<Box<dyn Validator>> = Vec::new();
    let started = Instant::now();
    let mut run = Run::open(&tree, options("adv-setsid"), &harness, &validators).expect("opens");
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let elapsed = started.elapsed();
    std::thread::sleep(Duration::from_secs(6).saturating_sub(started.elapsed()));
    let listed = fs::read_to_string(&pids).unwrap_or_default();
    let living: Vec<&str> = listed.split_whitespace().filter(|pid| alive(pid)).collect();
    kill_listed(&pids);
    let text = String::from_utf8_lossy(&out);
    assert!(matches!(stop, Ok(Stop::Hours(_))), "{stop:?}\n{text}");
    assert!(elapsed < budget() + GRACE, "{elapsed:?}\n{text}");
    assert!(living.is_empty(), "still running: {living:?}\n{text}");
    assert!(
        !tree.join("late.txt").exists(),
        "written after the step\n{text}"
    );
}

/// A `claude` whose background child joins the run's own process group and drops the call's
/// mark from its environment, then writes into the tree 5 s later: `run_child` and
/// `sweep_left` spare a process in takt's own group without the mark (forge.rs:519-524), so
/// the write lands after the killed step was discarded and the run returned `Hours`.
#[test]
#[ignore = "confinement, not the hour budget: a process that rejoins takt's own group is story:confinement-seam's"]
fn a_grandchild_that_joins_the_runs_own_group_writes_into_the_tree_after_the_discard() {
    let scripts = directory("escape-scripts");
    let pids = scripts.join("pids");
    let program = scripts.join("claude");
    executable(
        &program,
        &format!(
            "#!/bin/sh\nenv -u TAKT_CHILD python3 -I -c 'import os,sys,time\nos.setpgid(0, os.getpgid(int(sys.argv[1])))\nopen(sys.argv[2], \"a\").write(\"%d \" % os.getpid())\ntime.sleep(5)\nopen(\"escaped.txt\", \"w\").write(\"late\\n\")' $PPID '{p}' &\nsleep 20\n",
            p = pids.display()
        ),
    );
    let harness = ClaudeCode {
        program: program.display().to_string(),
        ..ClaudeCode::default()
    };
    let tree = repository("escape", "#!/bin/sh\nexit 1\n");
    let validators: Vec<Box<dyn Validator>> = Vec::new();
    let started = Instant::now();
    let mut run = Run::open(&tree, options("adv-escape"), &harness, &validators).expect("opens");
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let listed = fs::read_to_string(&pids).unwrap_or_default();
    while started.elapsed() < Duration::from_secs(8)
        && listed.split_whitespace().any(alive)
        && !tree.join("escaped.txt").exists()
    {
        std::thread::sleep(Duration::from_millis(50));
    }
    kill_listed(&pids);
    let text = String::from_utf8_lossy(&out);
    assert!(matches!(stop, Ok(Stop::Hours(_))), "{stop:?}\n{text}");
    assert!(
        !tree.join("escaped.txt").exists(),
        "a process of the killed step wrote into the tree after the run returned {stop:?}\n{text}"
    );
}

/// A test command that runs 15 s under a 3.6 s hour budget: the validator's child is held to
/// its own 30-minute timeout (validator.rs:219, 278), not to what is left of the budget, so
/// the run ends past the budget and the grace. Out of this story's scope (a validator step,
/// not a harness step); the case shows whether the stop is still reported as `Hours`.
#[test]
fn a_test_command_that_outlives_the_hour_budget_runs_the_run_past_it() {
    let tree = repository("validator", "#!/bin/sh\nsleep 15\nexit 1\n");
    let harness = Clocked {
        seen: Cell::new(None),
    };
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(TestsRun::new("./check.sh").expect("a command"))];
    let started = Instant::now();
    let mut run = Run::open(&tree, options("adv-validator"), &harness, &validators).expect("opens");
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let elapsed = started.elapsed();
    let text = String::from_utf8_lossy(&out);
    assert!(
        elapsed < budget() + GRACE,
        "the run took {elapsed:?} and stopped on {stop:?}\n{text}"
    );
}
