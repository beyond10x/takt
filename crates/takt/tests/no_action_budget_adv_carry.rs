//! Adversary, carry onto the integration branch: story:a-harness-step-without-an-action-is-retried
//! against its own words at the bound. `NO_ACTION_TRIES` (`crates/takt/src/run.rs`): "the run
//! ends on the error (exit 5) after the last. A run whose budget a try spent stops on the budget
//! instead (exit 3)"; `takt run --help`, exit 5: "a budget a try spent stops the run with exit 3";
//! the CHANGELOG: "the run ends exit 5 after the third, or exit 3 if a try spent the budget". The
//! retry loop asks for the bound before it asks for the budget, so a third try that spends the
//! budget ends the run on the error.

#[path = "support/own_home.rs"]
mod own_home;

use std::cell::Cell;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use takt::events::TokenUsage;
use takt::harness::{Harness, NoActionLine, StepContext, StepReport};
use takt::validator::{TestsRun, Validator};
use takt::{Run, RunOptions, Stop};

const BROKEN: &str = "pub fn add(a: i32, b: i32) -> i32 {\n    a - b\n}\n";

fn git(tree: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(args)
        .current_dir(tree)
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?}");
}

/// A scratch directory under this crate's build directory, `tree/` in it the repository;
/// removed when dropped.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn repository(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "no-action-budget-adv-carry-{}-{name}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    let tree = dir.join("tree");
    fs::create_dir_all(tree.join("src")).expect("the tree");
    fs::write(tree.join("README.md"), "a story\n").expect("a file");
    fs::write(tree.join("src/lib.rs"), BROKEN).expect("a file");
    git(&tree, &["init", "-q", "-b", "main"]);
    git(&tree, &["add", "-A"]);
    git(&tree, &["commit", "-q", "-m", "start"]);
    Scratch(dir)
}

/// A harness whose every session ends without the line naming its action, as Loom's session
/// executor reports one ([`NoActionLine`]), each reporting 160 tokens.
struct NoLine {
    calls: Cell<usize>,
}

impl Harness for NoLine {
    fn name(&self) -> &str {
        "no-line"
    }

    fn step(&self, _: &StepContext<'_>) -> anyhow::Result<StepReport> {
        self.calls.set(self.calls.get() + 1);
        Err(anyhow::Error::new(NoActionLine).context("the session proposed nothing"))
    }

    fn last_usage(&self) -> Option<TokenUsage> {
        Some(TokenUsage {
            input: 100,
            output: 20,
            cache_read: 40,
            cache_creation: 0,
        })
    }
}

/// Step 1 is the tests (red at the base, gone on past), steps 2 to 4 the three tries: with
/// `--iterations 4` the third try spends the step budget. The documents say the run then stops
/// on the budget, `Iterations (4)`, exit 3; it ends on the error, exit 5.
#[test]
fn a_third_try_that_spends_the_step_budget_stops_on_the_budget() {
    let scratch = repository("iterations");
    let tree = scratch.0.join("tree");
    let harness = NoLine {
        calls: Cell::new(0),
    };
    let validators: Vec<Box<dyn Validator>> = vec![Box::new(
        TestsRun::new("git grep -qE a.[+].b$").expect("a command"),
    )];
    let options = RunOptions {
        protocol: "story.delivery/1".to_owned(),
        intent: "make add add".to_owned(),
        inputs: Vec::new(),
        iterations: Some(4),
        hours: None,
        tokens: None,
        case_id: Some("no-action-budget-adv-carry".to_owned()),
        gate: None,
    };
    let mut run = Run::open(&tree, options, &harness, &validators).expect("the case opens");
    run.scan_change_with(None);
    run.continue_past_red_base();
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let text = String::from_utf8_lossy(&out).into_owned();
    let steps = run.steps;
    assert_eq!(
        harness.calls.get(),
        3,
        "three tries, the last at step 4: {stop:?}\n{text}"
    );
    assert_eq!(steps, 4, "{text}");
    assert!(
        matches!(stop, Ok(Stop::Iterations(4))),
        "the third try spent the step budget (steps {steps} of 4), so the run stops on it, exit 3; it ended {:?}\n{text}",
        stop.as_ref().map_err(|error| format!("{error:#}"))
    );
}
