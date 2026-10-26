//! The loop's refusals and budgets, each held by a case that fails when the behaviour goes. On
//! a scratch git tree with a check that fails until `check.txt` says `fixed` and the real test
//! validator: a `repository.inspect` step that changes a file is refused and its change
//! discarded; the merge gate requested with a changed file is refused and the change discarded;
//! the token budget stops the run once the harness has reported that many tokens, and not
//! before; the time budget stops the run once that much time has passed.

use std::cell::RefCell;
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

/// A scratch git tree under this crate's build directory. Dropping it removes the tree; a
/// removal that fails fails the case, unless the case is already failing.
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

/// A committed tree on `main` whose check passes once `check.txt` says `fixed`.
fn scratch(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("loop-gaps-{}-{name}", std::process::id()));
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

/// What the scripted harness does at its first judgement step.
#[derive(Clone, Copy)]
enum First {
    /// Fixes the check and reports `implementation.edit`.
    Fix,
    /// Fixes the check and reports `repository.inspect`, which may change nothing.
    FixAsInspect,
}

/// A harness that does `first` at its first judgement step and, once the tests pass, requests
/// the merge gate, writing `notes.md` first when `dirty_gate` is set. Every step sleeps `pause`
/// and reports `tokens`; every action it reports is logged.
struct Scripted {
    first: First,
    dirty_gate: bool,
    pause: Duration,
    tokens: u64,
    log: RefCell<Vec<String>>,
}

impl Scripted {
    fn new(first: First) -> Self {
        Self {
            first,
            dirty_gate: false,
            pause: Duration::ZERO,
            tokens: 100,
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

    /// The script that answers `repository.inspect` keeps state: only such a harness is offered
    /// an action that only reads (story:no-inspect-for-stateless-harness).
    fn keeps_state(&self) -> bool {
        matches!(self.first, First::FixAsInspect)
    }

    fn step(&self, context: &StepContext<'_>) -> Result<StepReport, anyhow::Error> {
        std::thread::sleep(self.pause);
        let action = if context.decision.holds("tests.pass") {
            assert!(
                context.approvals.contains(&"repository.merge".to_owned()),
                "the gate is offered once the tests pass: {:?}",
                context.approvals
            );
            if self.dirty_gate {
                fs::write(context.tree.join("notes.md"), "one more thing\n").expect("a write");
            }
            "repository.merge"
        } else {
            let action = match self.first {
                First::Fix => "implementation.edit",
                First::FixAsInspect => "repository.inspect",
            };
            assert!(
                context.offered.contains(&action.to_owned()),
                "{action} is offered: {:?}",
                context.offered
            );
            fs::write(context.tree.join("check.txt"), "fixed\n").expect("a write");
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

/// Opens the case `id` on `tree` with `options` and runs it with `harness`: the stop, the
/// steps taken, the tokens counted and what the run printed.
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

/// The paths `git status` names outside the cases directory.
fn dirty(tree: &Path) -> Vec<String> {
    git(tree, &["status", "--porcelain", "--untracked-files=all"])
        .lines()
        .map(|line| line[3..].to_owned())
        .filter(|path| !path.starts_with(".engineering/cases"))
        .collect()
}

/// `repository.inspect` reads and changes nothing: a step reported as one that changed a
/// tracked file is refused, the change discarded and nothing committed, though the change
/// itself would have made the check pass.
#[test]
fn an_inspect_step_that_changes_a_file_is_refused_and_its_change_discarded() {
    let tree = scratch("inspect");
    let head = git(&tree, &["rev-parse", "HEAD"]);
    let harness = Scripted::new(First::FixAsInspect);
    let (stop, steps, _, text) = run(&tree, options("inspect"), &harness);
    assert_eq!(
        stop,
        Stop::StepRefused("repository.inspect changed 1 path(s)".to_owned()),
        "{text}"
    );
    assert_eq!(steps, 2, "the failing check, then the inspect step: {text}");
    assert_eq!(harness.reported(), ["repository.inspect"], "{text}");
    assert!(
        text.contains("refused: repository.inspect changed 1 path(s); the change was discarded"),
        "{text}"
    );
    assert_eq!(
        fs::read_to_string(tree.join("check.txt")).expect("check.txt"),
        "broken\n",
        "{text}"
    );
    assert_eq!(git(&tree, &["rev-list", "--count", "main"]), "1", "{text}");
    assert_eq!(
        git(&tree, &["rev-parse", "HEAD"]),
        head,
        "nothing is committed"
    );
    assert_eq!(dirty(&tree), Vec::<String>::new(), "{text}");
}

/// Requesting the merge gate performs nothing: a request made with a changed file is refused
/// and the change discarded, never left behind for the approval to carry.
#[test]
fn the_gate_requested_with_a_changed_file_is_refused_and_the_change_discarded() {
    let tree = scratch("dirty-gate");
    let mut harness = Scripted::new(First::Fix);
    harness.dirty_gate = true;
    let (stop, _, _, text) = run(&tree, options("dirty-gate"), &harness);
    assert_eq!(
        stop,
        Stop::StepRefused("repository.merge requested with changes".to_owned()),
        "{text}"
    );
    assert_eq!(
        harness.reported(),
        ["implementation.edit", "repository.merge"],
        "{text}"
    );
    assert!(
        text.contains(
            "refused: repository.merge was requested with 1 changed path(s); the change was discarded"
        ),
        "{text}"
    );
    assert!(!tree.join("notes.md").exists(), "{text}");
    assert_eq!(dirty(&tree), Vec::<String>::new(), "{text}");
    assert_eq!(
        git(&tree, &["show", "--name-only", "--format=", "HEAD"]),
        "check.txt",
        "only the fix is committed: {text}"
    );
}

/// The same run with a clean request stops at the gate: the refusal above is the changed file's.
#[test]
fn the_gate_requested_with_nothing_changed_stops_for_its_approval() {
    let tree = scratch("clean-gate");
    let harness = Scripted::new(First::Fix);
    let (stop, steps, tokens, text) = run(&tree, options("clean-gate"), &harness);
    assert_eq!(
        stop,
        Stop::ApprovalRequired(vec!["repository.merge".to_owned()]),
        "{text}"
    );
    assert_eq!(steps, 4, "{text}");
    assert_eq!(tokens, 200, "{text}");
}

/// `--tokens`: once the harness has reported as many tokens as the budget, the run stops there,
/// before the next step; a budget not yet reached lets the run go on to the gate.
#[test]
fn the_token_budget_stops_the_run_once_reached_and_not_before() {
    let tree = scratch("tokens");
    let harness = Scripted::new(First::Fix);
    let mut spent = options("tokens");
    spent.tokens = Some(100);
    let (stop, steps, tokens, text) = run(&tree, spent, &harness);
    assert_eq!(stop, Stop::Tokens(100), "{text}");
    assert_eq!(tokens, 100, "{text}");
    assert_eq!(steps, 2, "the failing check, then the fix: {text}");
    assert_eq!(harness.reported(), ["implementation.edit"], "{text}");
    assert!(!text.contains("evidence: test_result pass"), "{text}");

    let tree = scratch("tokens-left");
    let harness = Scripted::new(First::Fix);
    let mut left = options("tokens-left");
    left.tokens = Some(201);
    let (stop, _, tokens, text) = run(&tree, left, &harness);
    assert_eq!(
        stop,
        Stop::ApprovalRequired(vec!["repository.merge".to_owned()]),
        "{text}"
    );
    assert_eq!(tokens, 200, "{text}");
}

/// `--hours`: once that much time has passed since the run was opened, the run stops before
/// its next step. The budget is half a second and the harness's step takes longer, so the run
/// stops before it reaches the gate, wherever a slow machine puts the first stop.
#[test]
fn the_time_budget_stops_the_run_once_spent() {
    let tree = scratch("hours");
    let mut harness = Scripted::new(First::Fix);
    harness.pause = Duration::from_millis(600);
    let hours = 0.5 / 3600.0;
    let mut spent = options("hours");
    spent.hours = Some(hours);
    let (stop, steps, _, text) = run(&tree, spent, &harness);
    assert_eq!(stop, Stop::Hours(hours), "{text}");
    assert!(steps <= 2, "{steps} steps: {text}");
    assert!(
        !harness.reported().contains(&"repository.merge".to_owned()),
        "{text}"
    );
}
