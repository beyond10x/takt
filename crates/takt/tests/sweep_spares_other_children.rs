//! A sweep reaps no child another call is waiting on. The gate validator's provider starts a
//! gate's command in a process group of its own and waits on it itself, outside
//! `forge::run_child`; while it runs, another thread of the same process sweeps what was left
//! running (`forge::sweep_left`, as a run does after each step) or ends a child of
//! `forge::run_child`, which sweeps too. The command still ends as itself, a pass, and not in
//! "No child processes" because the sweep took its exit status. Without the other thread the
//! same command passes: the sweep is the only variable.

#[path = "support/own_home.rs"]
mod own_home;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use takt::confinement::Command;

use anyhow::anyhow;
use takt::forge::Limits;
use takt::harness::{Harness, StepContext, StepReport};
use takt::validator::{GateRun, Observation, Validator};
use takt::{Run, RunOptions};

/// The protocol the case is opened on, relative to this crate: `gate_result`, `gate.run`.
const PROTOCOL: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/gate-only/1.yaml"
);

/// Runs `git <args>` in `tree`; its trimmed standard output.
fn git(tree: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(args)
        .current_dir(tree)
        .output()
        .expect("git runs");
    assert!(output.status.success(), "git {args:?}");
    String::from_utf8(output.stdout)
        .expect("utf-8")
        .trim()
        .to_owned()
}

/// A scratch git tree, under this crate's build directory. Dropping it removes the tree; a
/// removal that fails fails the case, unless the case is already failing.
struct Scratch(PathBuf);

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

/// A committed scratch repository whose one gate asserts that the command `check` succeeds,
/// the command admitted by its project file: a script that sleeps a moment and exits 0.
fn scratch(name: &str) -> Scratch {
    use std::os::unix::fs::PermissionsExt as _;
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("sweep-spares-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(dir.join(".engineering")).expect("scratch dir");
    fs::write(dir.join("Cargo.toml"), "[package]\nname = \"scratch\"\n").expect("a manifest");
    fs::write(
        dir.join(".engineering/gates.yaml"),
        "format: engineering-gates/1\nlanguage: canon-expr/1\ngates:\n  - id: check\n    \
         text: The gate check holds.\n    assertions:\n      - 'process.succeeded(\"check\")'\n",
    )
    .expect("a gates document");
    fs::write(
        dir.join(".engineering/assertions-project.yaml"),
        "format: engineering-assertions-project/1\ncommands:\n  check:\n    \
         executable: check.sh\n",
    )
    .expect("a project file");
    fs::write(dir.join("check.sh"), "#!/bin/sh\nsleep 0.3\nexit 0\n").expect("a script");
    fs::set_permissions(dir.join("check.sh"), fs::Permissions::from_mode(0o755)).expect("exec bit");
    let _ = git(&dir, &["init", "-q", "-b", "main"]);
    let _ = git(&dir, &["add", "-A"]);
    let _ = git(&dir, &["commit", "-q", "-m", "start"]);
    Scratch(dir)
}

/// A harness that is never offered a step: the case is only opened.
struct NoHarness;

impl Harness for NoHarness {
    fn name(&self) -> &str {
        "none"
    }

    fn step(&self, context: &StepContext<'_>) -> Result<StepReport, anyhow::Error> {
        Err(anyhow!(
            "no harness: a judgement step among {} is left",
            context.offered.join(", ")
        ))
    }
}

/// The gate validator's observation of a fresh scratch tree named `name`, made while another
/// thread calls `beside` over and over until the observation has ended; with no `beside`, made
/// alone.
fn observed_beside(name: &str, beside: Option<fn()>) -> Observation {
    let tree = scratch(name);
    // The gate validator reads the gates document from the case's base revision: the case is
    // opened first, as a run opens it, and pins that revision.
    let options = RunOptions {
        protocol: PROTOCOL.to_owned(),
        intent: "pass the gate".to_owned(),
        inputs: Vec::new(),
        iterations: Some(0),
        hours: None,
        tokens: None,
        case_id: Some(name.to_owned()),
        gate: None,
    };
    let none: Vec<Box<dyn Validator>> = Vec::new();
    let opened = Run::open(&tree.0, options, &NoHarness, &none).expect("the case opens");
    let head = git(&tree.0, &["rev-parse", "HEAD"]);
    let done = &AtomicBool::new(false);
    let observed = std::thread::scope(|scope| {
        if let Some(beside) = beside {
            scope.spawn(move || {
                while !done.load(Ordering::Relaxed) {
                    beside();
                    // A sweep taken again at once starves every reader waiting for the lock.
                    std::thread::sleep(Duration::from_millis(2));
                }
            });
        }
        let observed = GateRun::new(".engineering/gates.yaml").observe(&tree.0, &head, 1);
        done.store(true, Ordering::Relaxed);
        observed
    });
    drop(opened);
    observed.expect("the gate is observed")
}

/// Asserts that `observation` wrote one record, a pass.
fn passed(observation: &Observation) {
    let results: Vec<Option<&str>> = observation
        .records
        .iter()
        .map(|record| record.result.as_deref())
        .collect();
    assert_eq!(
        results,
        [Some("pass")],
        "the gate's command passes: {}",
        observation.summary
    );
}

/// Sweeps what was left running, as a run does after each step.
fn sweep() {
    takt::forge::sweep_left().expect("the sweep ends");
}

/// Runs `true` through `forge::run_child`, whose end sweeps what it left.
fn run_a_child() {
    takt::forge::run_child(
        &mut Command::new("true"),
        Limits {
            timeout: Duration::from_secs(10),
            output: 1024,
        },
    )
    .expect("true runs");
}

#[test]
fn a_gate_command_ends_as_itself_while_another_thread_sweeps() {
    passed(&observed_beside("sweep", Some(sweep)));
}

#[test]
fn a_gate_command_ends_as_itself_while_another_threads_child_ends() {
    passed(&observed_beside("run-child", Some(run_a_child)));
}

/// Expected before: on macOS a sweep took a child in this process's own group and session
/// that had ended and was not waited on yet, whose session `getsid` no longer answers, for one
/// that left the session, and reaped it: its caller's wait was "No child processes".
#[test]
fn a_sweep_reaps_no_ended_child_its_caller_has_not_waited_on() {
    let mut child = Command::new("true").spawn().expect("true starts");
    std::thread::sleep(Duration::from_millis(300));
    sweep();
    let status = child.wait().expect("the caller waits on its own child");
    assert!(status.success(), "{status}");
}

#[test]
fn a_gate_command_ends_as_itself_alone() {
    passed(&observed_beside("alone", None));
}
