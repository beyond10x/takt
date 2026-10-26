//! Adversary cases for the loop's budgets: the bounds `loop_gaps.rs` leaves open. The time
//! budget is held only from above there (a run that stops at once passes), the token budget only
//! at exact equality (a step that overshoots it is never compared), and the time budget only
//! with one step longer than the whole budget (a clock that restarts at every step passes).

use std::cell::{Cell, RefCell};
use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use takt::harness::{Harness, StepContext, StepReport};
use takt::validator::{TestsRun, Validator};
use takt::{Run, RunOptions, Stop};

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
    String::from_utf8_lossy(&output.stdout)
        .trim_end()
        .to_owned()
}

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

/// A committed tree on `main` whose check passes once `check.txt` says `fixed`.
fn scratch(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("loop-gaps-adv1-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(&dir).expect("scratch dir");
    fs::write(dir.join("check.txt"), "broken\n").expect("check file");
    fs::write(dir.join("check.sh"), "#!/bin/sh\ngrep -q fixed check.txt\n").expect("check script");
    fs::set_permissions(dir.join("check.sh"), fs::Permissions::from_mode(0o755)).expect("exec bit");
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "broken"]);
    Scratch(dir)
}

fn options(id: &str) -> RunOptions {
    RunOptions {
        protocol: "software.change/1".to_owned(),
        intent: "make the check pass".to_owned(),
        inputs: vec![
            ("risk".to_owned(), "trivial".to_owned()),
            ("change_kind".to_owned(), "feature".to_owned()),
            ("affects_security_boundary".to_owned(), "false".to_owned()),
        ],
        iterations: Some(8),
        hours: None,
        tokens: None,
        case_id: Some(id.to_owned()),
        gate: Some("repository.merge".to_owned()),
    }
}

fn validators() -> Vec<Box<dyn Validator>> {
    vec![Box::new(TestsRun::new("./check.sh").expect("a command"))]
}

/// A harness that sleeps `pause` and reports `tokens` at every step. With `fix` it fixes the
/// check at its first judgement step and requests the merge gate once the tests pass; without,
/// it commits a new file at every step and never fixes the check, so the run goes on until a
/// budget stops it.
struct Scripted {
    fix: bool,
    pause: Duration,
    tokens: u64,
    n: Cell<u32>,
    log: RefCell<Vec<String>>,
}

impl Scripted {
    fn new(fix: bool, pause: Duration, tokens: u64) -> Self {
        Self {
            fix,
            pause,
            tokens,
            n: Cell::new(0),
            log: RefCell::new(Vec::new()),
        }
    }

    fn reported(&self) -> Vec<String> {
        self.log.borrow().clone()
    }
}

impl Harness for Scripted {
    fn name(&self) -> &str {
        "scripted"
    }

    fn step(&self, context: &StepContext<'_>) -> Result<StepReport, anyhow::Error> {
        std::thread::sleep(self.pause);
        self.n.set(self.n.get() + 1);
        let action = if context.decision.holds("tests.pass") {
            "repository.merge"
        } else {
            let action = "implementation.edit";
            assert!(
                context.offered.contains(&action.to_owned()),
                "{action} is offered: {:?}",
                context.offered
            );
            if self.fix {
                fs::write(context.tree.join("check.txt"), "fixed\n").expect("a write");
            } else {
                fs::write(
                    context.tree.join(format!("step-{}.txt", self.n.get())),
                    "more\n",
                )
                .expect("a write");
            }
            action
        };
        self.log.borrow_mut().push(action.to_owned());
        Ok(StepReport {
            action: action.to_owned(),
            summary: format!("{action} by the script"),
            tokens: Some(self.tokens),
        })
    }
}

fn run(tree: &Path, options: RunOptions, harness: &Scripted) -> (Stop, u64, u64, String) {
    let validators = validators();
    let mut run = Run::open(tree, options, harness, &validators).expect("opens");
    let mut out = Vec::new();
    run.continue_past_red_base();
    let stop = run.run(&mut out).expect("the run stops without an error");
    (
        stop,
        run.steps,
        run.tokens,
        String::from_utf8_lossy(&out).into_owned(),
    )
}

/// `--hours` not yet spent lets the run go on to the gate. Thirty seconds of budget; the two
/// judgement steps sleep 0.6 s each, so a budget read in the wrong unit (minutes: 0.5 s;
/// seconds: 8 ms) or a comparison turned round stops the run before the gate.
#[test]
fn adv1_the_time_budget_does_not_stop_a_run_before_it_is_spent() {
    let tree = scratch("hours-left");
    let harness = Scripted::new(true, Duration::from_millis(600), 100);
    let mut left = options("hours-left");
    left.hours = Some(30.0 / 3600.0);
    let (stop, steps, _, text) = run(&tree, left, &harness);
    assert_eq!(
        stop,
        Stop::ApprovalRequired(vec!["repository.merge".to_owned()]),
        "{text}"
    );
    assert_eq!(steps, 4, "{text}");
    assert_eq!(
        harness.reported(),
        ["implementation.edit", "repository.merge"],
        "{text}"
    );
}

/// `--hours` counts from when the run was opened, not from the last step: steps of 0.2 s each,
/// every one shorter than the 0.5 s budget, add up past it, and the run stops on the time
/// budget long before its 40 iterations.
#[test]
fn adv1_the_time_budget_counts_every_step_not_only_the_last() {
    let tree = scratch("hours-sum");
    let harness = Scripted::new(false, Duration::from_millis(200), 1);
    let hours = 0.5 / 3600.0;
    let mut spent = options("hours-sum");
    spent.hours = Some(hours);
    spent.iterations = Some(40);
    let (stop, steps, _, text) = run(&tree, spent, &harness);
    assert_eq!(stop, Stop::Hours(hours), "{steps} steps: {text}");
    assert!(steps < 40, "{steps} steps: {text}");
}

/// `--tokens` overshot by one step: the harness reports 100 tokens at a step and the budget is
/// 50, so the count passes the budget without ever equalling it. The run stops there all the
/// same, before the next step.
#[test]
fn adv1_the_token_budget_stops_a_run_whose_step_overshoots_it() {
    let tree = scratch("tokens-over");
    let harness = Scripted::new(true, Duration::ZERO, 100);
    let mut over = options("tokens-over");
    over.tokens = Some(50);
    let (stop, steps, tokens, text) = run(&tree, over, &harness);
    assert_eq!(stop, Stop::Tokens(50), "{text}");
    assert_eq!(tokens, 100, "{text}");
    assert_eq!(steps, 2, "the failing check, then the fix: {text}");
    assert_eq!(harness.reported(), ["implementation.edit"], "{text}");
}
