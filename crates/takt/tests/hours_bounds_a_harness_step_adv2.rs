//! Adversary cases, pass 2, for `--hours` bounding a step. Every harness is a double: a Rust
//! fake or a `claude` script; nothing reaches a model. Every process a case starts writes its
//! pid to a file outside the tree and is killed before the case ends. Timing margins are whole
//! seconds: the hour budget is 3.6 s and the grace 10 s.

use std::cell::{Cell, RefCell};
use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use takt::harness::{ClaudeCode, Harness, StepContext, StepReport};
use takt::validator::{GateRun, TestsRun, Validator};
use takt::{Run, RunOptions, Stop};

/// The hour budget of every run below: 3.6 s.
const HOURS: f64 = 0.001;

/// How long past its budget a run may take to stop, as the acceptance allows.
const GRACE: Duration = Duration::from_secs(10);

/// The gate-only protocol: `gate.run` is its one action.
const GATE_ONLY: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/gate-only/1.yaml"
);

/// One case at a time: a run sweeps what left the test process's own group
/// ([`takt::forge::sweep_left`]), a concurrent case's gate command among it.
fn serial() -> std::sync::MutexGuard<'static, ()> {
    static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());
    SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn budget() -> Duration {
    Duration::from_secs_f64(HOURS * 3600.0)
}

fn git(tree: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(args)
        .current_dir(tree)
        .output()
        .expect("git runs");
    assert!(output.status.success(), "git {args:?}");
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
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
        .join(format!("hours-adv2-{}-{name}", std::process::id()));
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

/// A committed repository on `main` with a README and `files`, executable when their name ends
/// in `.sh`.
fn repository(name: &str, files: &[(&str, &str)]) -> Scratch {
    let dir = directory(name);
    fs::write(dir.join("README.md"), "a story\n").expect("a file");
    for (path, text) in files {
        let path = dir.join(path);
        fs::create_dir_all(path.parent().expect("a parent")).expect("a parent directory");
        if path.extension().is_some_and(|e| e == "sh") {
            executable(&path, text);
        } else {
            fs::write(&path, text).expect("a file");
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

/// Kills every pid listed in `file`, if it exists; the pids that were still alive.
fn kill_listed(file: &Path) -> Vec<String> {
    let mut living = Vec::new();
    if let Ok(text) = fs::read_to_string(file) {
        for pid in text.split_whitespace() {
            if alive(pid) {
                living.push(pid.to_owned());
                let _ = Command::new("kill").args(["-9", pid]).status();
            }
        }
    }
    living
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

/// The gate's command runs under the gate validator's own executor, held to the project file's
/// `timeout_seconds` (here 40 s) and never to what is left of the hour budget: `GateRun` does
/// not override `observe_within` (validator.rs:502), so the default at validator.rs:74-83
/// drops `time_left`. The commit says validator steps are held to the deadline too, and the
/// prose (takt.yaml, the validator) names only the test command as held and the forge's
/// programs as exempt; `gate.run` is neither. The CLI registers it whenever
/// `.engineering/gates.yaml` is a file (main.rs:544-552).
#[test]
fn a_gate_command_that_outlives_the_hour_budget_runs_the_run_past_it() {
    let _serial = serial();
    let scripts = directory("gate-scripts");
    let pids = scripts.join("pids");
    let tree = repository(
        "gate",
        &[
            (
                ".engineering/gates.yaml",
                "format: engineering-gates/1\nlanguage: canon-expr/1\ngates:\n  - id: check\n    \
                 text: The check holds.\n    assertions:\n      - 'process.succeeded(\"check\")'\n",
            ),
            (
                ".engineering/assertions-project.yaml",
                "format: engineering-assertions-project/1\ntimeout_seconds: 40\ncommands:\n  check:\n    executable: check.sh\n",
            ),
            (
                "check.sh",
                &format!("#!/bin/sh\necho $$ >> '{}'\nsleep 25\n", pids.display()),
            ),
        ],
    );
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(GateRun::new(".engineering/gates.yaml"))];
    let mut options = options("adv2-gate");
    options.protocol = GATE_ONLY.to_owned();
    options.inputs = Vec::new();
    options.gate = None;
    let started = Instant::now();
    let mut run = Run::open(&tree, options, &Fails, &validators).expect("opens");
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let elapsed = started.elapsed();
    kill_listed(&pids);
    let text = String::from_utf8_lossy(&out);
    assert!(
        fs::read_to_string(&pids).is_ok_and(|p| !p.trim().is_empty()),
        "the premise: the gate ran its command\n{stop:?}\n{text}"
    );
    assert!(
        elapsed < budget() + GRACE,
        "the run took {elapsed:?} under a {:?} hour budget, stopping on {stop:?}\n{text}",
        budget()
    );
}

/// A harness that changes a file, then returns success well after the run's end.
struct Late {
    until: Cell<Option<Instant>>,
    offered: RefCell<Vec<String>>,
    returned: Cell<Option<Instant>>,
}

impl Harness for Late {
    fn name(&self) -> &str {
        "late"
    }

    fn step(&self, context: &StepContext<'_>) -> anyhow::Result<StepReport> {
        self.offered.replace(context.offered.to_vec());
        fs::create_dir_all(context.tree.join("tests"))?;
        fs::write(
            context.tree.join("tests/late.txt"),
            "written past the run's end\n",
        )?;
        let until = self.until.get().expect("set before the run");
        std::thread::sleep(until.saturating_duration_since(Instant::now()));
        self.returned.set(Some(Instant::now()));
        let action = context
            .offered
            .iter()
            .find(|a| a.ends_with(".write") || a.ends_with(".edit"))
            .or_else(|| context.offered.first())
            .cloned()
            .unwrap_or_default();
        Ok(StepReport {
            action,
            summary: "changed tests/late.txt".to_owned(),
            tokens: None,
        })
    }
}

/// Decision 1 of the brief: a step that outlives what is left of the budget has its change
/// discarded and the run stops on `Hours`. The run discards it only when the step *failed*
/// at or after the deadline (run.rs:3971-3994); a step that returns success 2 s past the run's
/// end keeps what it changed. A Rust harness that ignores `time_left` reaches this directly;
/// Claude Code's child reaches it when it exits 0 between the deadline and `run_child`'s next
/// poll (forge.rs:647-651 reads the status before it checks the timeout).
#[test]
fn a_harness_that_returns_success_past_the_run_end_keeps_its_change() {
    let _serial = serial();
    let tree = repository("late", &[]);
    let harness = Late {
        until: Cell::new(None),
        offered: RefCell::new(Vec::new()),
        returned: Cell::new(None),
    };
    let validators: Vec<Box<dyn Validator>> = Vec::new();
    let mut run = Run::open(&tree, options("adv2-late"), &harness, &validators).expect("opens");
    let run_end = Instant::now() + budget();
    harness.until.set(Some(run_end + Duration::from_secs(2)));
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let text = String::from_utf8_lossy(&out);
    let returned = harness.returned.get().expect("the harness took a step");
    assert!(
        returned >= run_end,
        "the premise: the step returned past the run's end"
    );
    let committed = git(
        &tree,
        &["log", "--all", "--format=%h", "--", "tests/late.txt"],
    );
    let offered = harness.offered.borrow().clone();
    assert!(
        !tree.join("tests/late.txt").exists() && committed.is_empty(),
        "a step that returned {:?} past the run's end kept its change (in the tree: {}, in commits: [{committed}]); offered {offered:?}; stop {stop:?}\n{text}",
        returned.duration_since(run_end),
        tree.join("tests/late.txt").exists()
    );
}

/// The prose of the validator (takt.yaml, at "the test command also to what is left of the
/// run's hour budget"): "an observation that fails and returns at or after the run's end
/// records nothing, leaves a refusal by takt naming the budget, and ends the run on `hours`".
/// A test command the budget kills after it wrote into the tree ends the run
/// `StepRefused`, its refusal naming the foreign write and "timed out", not the hour budget:
/// the foreign-write check (run.rs:3609-3642) runs before the budget's (run.rs:3643-3666).
/// Decided as the safer stop: the case asserts `StepRefused`, the refusal naming both.
#[test]
fn a_test_command_the_budget_killed_after_writing_into_the_tree_does_not_stop_on_hours() {
    let _serial = serial();
    let scripts = directory("dirty-scripts");
    let pids = scripts.join("pids");
    let tree = repository(
        "dirty",
        &[(
            "check.sh",
            &format!(
                "#!/bin/sh\necho $$ >> '{}'\necho dirty >> README.md\nsleep 20\n",
                pids.display()
            ),
        )],
    );
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(TestsRun::new("./check.sh").expect("a command"))];
    let started = Instant::now();
    let mut run = Run::open(&tree, options("adv2-dirty"), &Fails, &validators).expect("opens");
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let elapsed = started.elapsed();
    kill_listed(&pids);
    let text = String::from_utf8_lossy(&out);
    assert!(
        elapsed >= budget() && elapsed < budget() + GRACE,
        "the premise: the budget ended the test command, {elapsed:?}\n{stop:?}\n{text}"
    );
    match &stop {
        Ok(Stop::StepRefused(why)) => assert!(
            why.contains("README.md") && why.contains("timed out"),
            "the refusal names the write and the timeout: {why}\n{text}"
        ),
        other => panic!(
            "an observation the budget ended after writing into the tree stops on {other:?}, not `StepRefused`\n{text}"
        ),
    }
}

/// Probe: a test command that starts a daemon of its own session holding its output, then
/// exits; the run is still bounded, and the daemon is gone once the run returns.
#[test]
fn a_test_command_that_leaves_a_daemon_is_bounded_and_the_daemon_killed() {
    let _serial = serial();
    let scripts = directory("daemon-scripts");
    let pids = scripts.join("pids");
    let tree = repository(
        "daemon",
        &[(
            "check.sh",
            &format!(
                "#!/bin/sh\nsetsid sh -c 'trap \"\" TERM; exec sleep 30' &\necho $$ $! >> '{p}'\nexit 0\n",
                p = pids.display()
            ),
        )],
    );
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(TestsRun::new("./check.sh").expect("a command"))];
    let started = Instant::now();
    let mut run = Run::open(&tree, options("adv2-daemon"), &Fails, &validators).expect("opens");
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let elapsed = started.elapsed();
    std::thread::sleep(Duration::from_millis(500));
    let listed = fs::read_to_string(&pids).unwrap_or_default();
    let living = kill_listed(&pids);
    let text = String::from_utf8_lossy(&out);
    assert_eq!(
        listed.split_whitespace().count(),
        2,
        "the premise: the daemon started"
    );
    assert!(elapsed < budget() + GRACE, "{elapsed:?}\n{stop:?}\n{text}");
    assert!(
        living.is_empty(),
        "still running: {living:?}\n{stop:?}\n{text}"
    );
}

/// Probe: the refusal a budget-killed step leaves is one line, names the step, and is in
/// `events.jsonl` after the discard.
#[test]
fn the_budget_refusal_is_written_once_and_names_the_step() {
    let _serial = serial();
    let scripts = directory("once-scripts");
    let pids = scripts.join("pids");
    let program = scripts.join("claude");
    executable(
        &program,
        &format!(
            "#!/bin/sh\necho $$ >> '{p}'\necho stray > stray.txt\nsleep 20 &\necho $! >> '{p}'\nwait\n",
            p = pids.display()
        ),
    );
    let harness = ClaudeCode {
        program: program.display().to_string(),
        ..ClaudeCode::default()
    };
    let tree = repository("once", &[]);
    let validators: Vec<Box<dyn Validator>> = Vec::new();
    let mut run = Run::open(&tree, options("adv2-once"), &harness, &validators).expect("opens");
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    kill_listed(&pids);
    let text = String::from_utf8_lossy(&out);
    assert!(matches!(stop, Ok(Stop::Hours(_))), "{stop:?}\n{text}");
    let events = fs::read_to_string(tree.join(".engineering/cases/adv2-once/events.jsonl"))
        .expect("the run record");
    let refusals: Vec<&str> = events
        .lines()
        .filter(|line| line.contains("\"event\":\"refusal\""))
        .collect();
    assert_eq!(refusals.len(), 1, "{events}");
    assert!(
        refusals[0].contains("step 1") && refusals[0].contains("hour budget"),
        "{events}"
    );
    assert!(!tree.join("stray.txt").exists(), "{text}");
}
