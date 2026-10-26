//! Adversary pass 1 on story:ci-pending-waits: the wait's bound for hour budgets the CLI parses
//! (`--hours inf`, `--hours NaN`), and the refusal of a merge for a pending required check as the
//! bot route reports it, read by `forge::refused_for_a_pending_check`.

use std::cell::Cell;
use std::fs;
use std::panic::AssertUnwindSafe;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;
use std::time::{Duration, Instant};

use anyhow::anyhow;
use takt::case::{CASES_DIR, LOCK_FILE};
use takt::evidence::Record;
use takt::forge::{
    BotRoute, Checks, Exit, Forge, Memory, PullRequest, PullRequestView, Runner,
    refused_for_a_pending_check,
};
use takt::frontier::Aep;
use takt::harness::{Harness, StepContext, StepReport};
use takt::validator::{CiRead, PrMerged, PrOpen, Validator};
use takt::{Run, RunOptions, Stop};

const PROTOCOL: &str = "story.delivery/1";

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");

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
    String::from_utf8(output.stdout)
        .expect("utf-8")
        .trim()
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

fn scratch_dir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "ci-pending-waits-adv1-{}-{name}",
        std::process::id()
    ));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    dir
}

fn repository(name: &str) -> Scratch {
    let dir = scratch_dir(name);
    copy_dir(&Path::new(FIXTURES).join("one-ready"), &dir);
    fs::write(dir.join("check.txt"), "broken\n").expect("a file");
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "start"]);
    Scratch(dir)
}

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

fn options(iterations: Option<u64>, hours: Option<f64>) -> RunOptions {
    RunOptions {
        protocol: PROTOCOL.to_owned(),
        intent: String::new(),
        inputs: Vec::new(),
        iterations,
        hours,
        tokens: None,
        case_id: None,
        gate: Some("repository.merge".to_owned()),
    }
}

fn validators(forge: Rc<dyn Forge>) -> Vec<Box<dyn Validator>> {
    vec![
        Box::new(PrOpen::new(forge.clone())),
        Box::new(CiRead::new(forge.clone())),
        Box::new(PrMerged::new(forge)),
    ]
}

fn record(n: usize, kind: &str, revision: &str) -> Record {
    Record::new(
        format!("e{n}-{}", &revision[..8]),
        kind,
        Some("pass".to_owned()),
        "implementation",
        revision,
        "ci-pending-waits-adv1",
    )
}

/// The story taken, its work committed, tests and gate green at the commit: the case id. The
/// case is ready for `pr.open`.
fn worked_case(repo: &Path) -> String {
    let forge: Rc<dyn Forge> = Rc::new(Memory::default());
    let validators = validators(forge);
    let mut out = Vec::new();
    let opened = Run::open_next(
        repo,
        options(Some(1), None),
        &Aep::default(),
        &NoHarness,
        &validators,
        &mut out,
    )
    .expect("the frontier answers")
    .expect("story:ready-one is taken");
    let id = opened.case.id.clone();
    drop(opened);
    fs::write(repo.join("check.txt"), "fixed\n").expect("a file");
    git(repo, &["commit", "-q", "-am", "the work"]);
    let worked = git(repo, &["rev-parse", "HEAD"]);
    let forge: Rc<dyn Forge> = Rc::new(Memory::default());
    let validators = self::validators(forge);
    let mut run = Run::resume(repo, options(Some(1), None), &id, &NoHarness, &validators)
        .expect("the case resumes");
    run.case
        .add_evidence(&record(1, "test_result", &worked))
        .expect("written");
    run.case
        .add_evidence(&record(2, "gate_result", &worked))
        .expect("written");
    run.case
        .add_evidence(&takt::evidence::Record::new(
            format!("handover-{}", &worked[..8]),
            "handover",
            None,
            "implementation",
            &worked,
            takt::run::CHOICE_PRODUCER,
        ))
        .expect("written");
    // A change to publish at the revision, as `change.read` records it.
    run.case
        .add_evidence(&takt::evidence::Record::new(
            format!("change-record-{}", &worked[..8]),
            "change_record",
            Some("ahead".to_owned()),
            "implementation",
            &worked,
            "takt.change-read",
        ))
        .expect("written");
    id
}

/// The double whose checks stay pending; after `cutoff` reads it answers an error, so a run
/// that never ends by itself still ends here.
struct AlwaysPending {
    memory: Rc<Memory>,
    reads: Cell<usize>,
    cutoff: usize,
}

impl AlwaysPending {
    fn new(cutoff: usize) -> Self {
        Self {
            memory: Rc::new(Memory {
                checks: Checks::Pending,
                ..Memory::default()
            }),
            reads: Cell::new(0),
            cutoff,
        }
    }
}

impl Forge for AlwaysPending {
    fn target_tip(&self, tree: &Path, base: &str) -> anyhow::Result<String> {
        self.memory.target_tip(tree, base)
    }

    /// Scans nothing: the double publishes nothing off the machine.
    fn scan_outgoing(
        &self,
        _: &std::path::Path,
        _: &std::path::Path,
        _: &takt::forge::Outgoing<'_>,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    fn name(&self) -> &str {
        "always-pending"
    }

    fn push(&self, tree: &Path, branch: &str) -> anyhow::Result<()> {
        self.memory.push(tree, branch)
    }

    fn push_revision(&self, tree: &Path, branch: &str, revision: &str) -> anyhow::Result<()> {
        self.memory.push_revision(tree, branch, revision)
    }

    fn open_pull_request(
        &self,
        tree: &Path,
        dir: &Path,
        request: &PullRequest,
    ) -> anyhow::Result<u64> {
        self.memory.open_pull_request(tree, dir, request)
    }

    fn pull_request(&self, tree: &Path, number: u64) -> anyhow::Result<PullRequestView> {
        self.memory.pull_request(tree, number)
    }

    fn checks(&self, tree: &Path, number: u64) -> anyhow::Result<Checks> {
        let n = self.reads.get() + 1;
        self.reads.set(n);
        if n > self.cutoff {
            anyhow::bail!("the double's cutoff: {} checks reads", self.cutoff);
        }
        self.memory.checks(tree, number)
    }

    fn merge(&self, tree: &Path, dir: &Path, number: u64, head: &str) -> anyhow::Result<()> {
        self.memory.merge(tree, dir, number, head)
    }
}

/// `takt run --hours inf` parses (`--hours` is an `Option<f64>`, main.rs `LoopArgs`), and
/// `budget_spent` never ends such a budget: a run with a step budget beside it waits on
/// pending checks until its steps are spent and stops `Waiting`, and lets go of its case.
/// `Run::pause` converts the hours left, infinite, with `Duration::from_secs_f64`, which
/// panics: the run never stops, and `run.lock` stays behind, so the next run is refused as
/// "held". `--hours 1e16` panics the same way (3.6e19 s is past `u64` seconds).
#[test]
fn adv1_an_infinite_hour_budget_waits_without_a_panic_and_lets_go_of_the_case() {
    let repo = repository("hours-inf");
    let id = worked_case(&repo);
    let forge = Rc::new(AlwaysPending::new(1_000));
    let validators = validators(forge.clone());
    let mut run = Run::resume(
        &repo,
        options(Some(4), Some(f64::INFINITY)),
        &id,
        &NoHarness,
        &validators,
    )
    .expect("the case resumes");
    run.poll = Duration::from_millis(5);
    let mut out = Vec::new();

    let outcome = std::panic::catch_unwind(AssertUnwindSafe(|| run.run(&mut out)));

    let text = String::from_utf8_lossy(&out).into_owned();
    let lock = repo.join(CASES_DIR).join(&id).join(LOCK_FILE);
    let stop = match outcome {
        Ok(stop) => stop.map_err(|error| format!("{error:#}")),
        Err(panic) => {
            let why = panic
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| panic.downcast_ref::<&str>().map(|s| (*s).to_owned()))
                .unwrap_or_default();
            Err(format!(
                "the run panicked: {why}; run.lock left behind: {}",
                lock.exists()
            ))
        }
    };
    assert_eq!(
        stop,
        Ok(Stop::Waiting {
            on: vec!["ci.read".to_owned()],
            budget: Some(Box::new(Stop::Iterations(4))),
            local: Vec::new(),
        }),
        "{text}"
    );
    assert!(!lock.exists(), "run.lock left behind: {text}");
}

/// `takt run --hours NaN` parses too, and `budget_spent` never ends it (`elapsed >= NaN` is
/// false); `Run::pause` cuts the poll to `(NaN - elapsed).max(0.0)`, zero. A run with no other
/// budget then observes the forge again and again with no pause and no end. Expected: it stops
/// by itself after at most a couple of reads (as a run with no usable budget does, `Waiting`),
/// or refuses the budget; never a loop on the forge that only the forge's failure ends.
#[test]
fn adv1_a_nan_hour_budget_never_polls_the_forge_without_a_pause_or_an_end() {
    let repo = repository("hours-nan");
    let id = worked_case(&repo);
    let cutoff = 40;
    let forge = Rc::new(AlwaysPending::new(cutoff));
    let validators = validators(forge.clone());
    let mut run = Run::resume(
        &repo,
        options(None, Some(f64::NAN)),
        &id,
        &NoHarness,
        &validators,
    )
    .expect("the case resumes");
    run.poll = Duration::from_millis(250);
    let mut out = Vec::new();
    let started = Instant::now();

    let stop = run.run(&mut out).map_err(|error| format!("{error:#}"));

    let elapsed = started.elapsed();
    let text = String::from_utf8_lossy(&out).into_owned();
    let tail: String = text
        .lines()
        .filter(|line| line.starts_with("  waiting: "))
        .take(2)
        .collect::<Vec<_>>()
        .join(" | ");
    assert!(
        stop.is_ok() && forge.reads.get() <= 2,
        "a NaN hour budget: {} checks reads in {elapsed:?} (the poll is 250 ms), the run ended {stop:?}; first waits: {tail}",
        forge.reads.get()
    );
}

/// What the bot route's runner answers: every line succeeds but the merge's API write, which
/// exits 1 with `stderr`.
struct RefusingMerge {
    stderr: String,
}

impl Runner for RefusingMerge {
    /// Runs the line as [`Runner::run`] does; the double starts no process, so `env` is
    /// dropped.
    fn run_env(
        &self,
        tree: &std::path::Path,
        argv: &[String],
        _: &[(&str, &str)],
    ) -> anyhow::Result<takt::forge::Exit> {
        self.run(tree, argv)
    }

    fn run(&self, _: &Path, argv: &[String]) -> anyhow::Result<Exit> {
        let merge = argv.first().map(String::as_str) == Some("b10x-gates")
            && argv.iter().any(|arg| arg == "PUT");
        Ok(Exit {
            code: Some(i32::from(merge)),
            stdout: String::new(),
            stderr: if merge {
                self.stderr.clone()
            } else {
                String::new()
            },
        })
    }
}

/// The merge's error as the bot route makes it, for the case directory `artifacts` under
/// `tree`: the line it ran (its `--input`/`--output` paths under the case directory, its
/// `--repository`) and the forge's words, with the context `PrMerged` adds.
fn bot_route_refusal(repository: &str, artifacts: &Path, stderr: &str) -> anyhow::Error {
    let route = BotRoute::new(
        repository,
        None,
        Box::new(RefusingMerge {
            stderr: stderr.to_owned(),
        }),
    );
    let tree = artifacts.ancestors().nth(4).expect("the tree");
    route
        .merge(tree, artifacts, 1, &"a".repeat(40))
        .expect_err("the forge refuses the merge")
}

/// The refusal story:ci-pending-waits waits on, `405 Required status check "<name>" is
/// expected`, as the bot route reports it. `refused_for_a_pending_check` reads the whole error
/// chain and gives up on any `fail` in it; the chain carries the line the route ran, and so the
/// tree's path, the case id (from the story id) and the repository name. A repository checked
/// out at `.../failover-service`, a story `fix-a-failing-test`, or a repository named
/// `o/fail2ban` turns the pending-check refusal into an error that ends the run (exit 5),
/// validator.rs `PrMerged::observe_waiting`, the `Err(error)` arm after the merge.
#[test]
fn adv1_a_pending_check_refusal_through_the_bot_route_is_waited_on_whatever_the_paths_say() {
    let forge_words = "405 Required status check \"task check\" is expected";
    let root = Scratch(scratch_dir("refusal-paths"));
    let plain = root
        .join("app")
        .join(CASES_DIR)
        .join("story-add-a-todo-20261018T120340Z")
        .join("artifacts");
    let in_tree = root
        .join("failover-service")
        .join(CASES_DIR)
        .join("story-add-a-todo-20261018T120340Z")
        .join("artifacts");
    let in_story = root
        .join("app")
        .join(CASES_DIR)
        .join("story-fix-a-failing-test-20261018T120340Z")
        .join("artifacts");

    // The control: the same refusal, no `fail` in any path, is waited on.
    let control = bot_route_refusal("o/app", &plain, forge_words);
    assert!(
        refused_for_a_pending_check(&control),
        "the control is not waited on: {control:#}"
    );

    let mut errors = Vec::new();
    for (what, repository, artifacts) in [
        ("a tree at failover-service", "o/app", &in_tree),
        ("a story fix-a-failing-test", "o/app", &in_story),
        ("a repository o/fail2ban", "o/fail2ban", &plain),
    ] {
        let error = bot_route_refusal(repository, artifacts, forge_words);
        if !refused_for_a_pending_check(&error) {
            errors.push(format!("{what}: an error, not waited on: {error:#}"));
        }
    }
    assert!(errors.is_empty(), "{}", errors.join("\n"));
}

/// A required check that has not reported, whose name holds `fail` (`fail-fast`,
/// `no-failures`): the forge's refusal names the check, and the refusal is read as a failing
/// check, an error, though nothing failed.
#[test]
fn adv1_a_pending_required_check_whose_name_holds_fail_is_still_waited_on() {
    let mut errors = Vec::new();
    for words in [
        "405 Required status check \"fail-fast\" is expected",
        "405 Required status check \"no-failures\" is in progress",
    ] {
        let error = anyhow!("b10x-gates api exited 1: {words}")
            .context("merge pull request #1 through the forge bot");
        if !refused_for_a_pending_check(&error) {
            errors.push(words);
        }
    }
    assert!(
        errors.is_empty(),
        "read as a failing check, not waited on: {errors:?}"
    );
}
