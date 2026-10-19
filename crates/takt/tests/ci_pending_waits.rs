//! story:ci-pending-waits: a story run whose pull request's checks are still running, or whose
//! pull request's head lags the push, waits for the forge inside its budget and then records
//! the result, offering the harness no judgement step meanwhile; a run whose budget ends first
//! stops `Waiting`, not `NoProgress`, and the next run resumes the case. A merge the forge
//! refuses for a required check that has not reported is nothing merged, no record, and waited
//! on like pending checks.

use std::cell::Cell;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail};
use takt::case::CASES_DIR;
use takt::evidence::Record;
use takt::forge::{Checks, Forge, Head, Memory, PullRequest, PullRequestView};
use takt::frontier::{Aep, approve};
use takt::harness::{Harness, StepContext, StepReport};
use takt::validator::{CiRead, PrMerged, PrOpen, Validator};
use takt::{Run, RunOptions, Stop};

const PROTOCOL: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../protocols/story-delivery/1.yaml"
);

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");

/// How long the runs here wait on the forge between two observations.
const POLL: Duration = Duration::from_millis(5);

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

fn repository(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("ci-pending-waits-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    copy_dir(&Path::new(FIXTURES).join("one-ready"), &dir);
    fs::write(dir.join("check.txt"), "broken\n").expect("a file");
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "start"]);
    Scratch(dir)
}

/// Requests the merge gate when it is offered; any other offer it counts and answers with an
/// inspection that changes nothing, as a harness with nothing left to do answers.
#[derive(Default)]
struct Gatekeeper {
    offers: Cell<usize>,
}

impl Harness for Gatekeeper {
    fn name(&self) -> &str {
        "gatekeeper"
    }

    /// It answers `repository.inspect`, which only a harness that keeps state is offered
    /// (story:no-inspect-for-stateless-harness).
    fn keeps_state(&self) -> bool {
        true
    }

    fn step(&self, context: &StepContext<'_>) -> Result<StepReport, anyhow::Error> {
        if context.approvals.contains(&"repository.merge".to_owned()) {
            return Ok(StepReport {
                action: "repository.merge".to_owned(),
                summary: "merge it".to_owned(),
                tokens: None,
            });
        }
        self.offers.set(self.offers.get() + 1);
        Ok(StepReport {
            action: "repository.inspect".to_owned(),
            summary: "nothing to change".to_owned(),
            tokens: None,
        })
    }
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
        "ci-pending-waits",
    )
}

/// The double with its checks pending for the first `pending` reads, then its own answer.
struct PendingFor {
    memory: Rc<Memory>,
    pending: Cell<usize>,
}

impl PendingFor {
    fn new(pending: usize) -> Self {
        Self {
            memory: Rc::new(Memory {
                checks: Checks::Pass,
                ..Memory::default()
            }),
            pending: Cell::new(pending),
        }
    }

    fn checks_read(&self) -> usize {
        self.memory
            .calls
            .borrow()
            .iter()
            .filter(|call| call.starts_with("checks "))
            .count()
    }
}

impl Forge for PendingFor {
    fn name(&self) -> &str {
        "pending-for"
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
        let answer = self.memory.checks(tree, number)?;
        let left = self.pending.get();
        if left == 0 {
            return Ok(answer);
        }
        self.pending.set(left - 1);
        Ok(Checks::Pending)
    }

    fn merge(&self, tree: &Path, dir: &Path, number: u64, head: &str) -> anyhow::Result<()> {
        self.memory.merge(tree, dir, number, head)
    }
}

/// The story taken through `open_next`, its work committed on the case's branch, and the tests
/// and the gate recorded green at that revision: the case id and the revision. The case is
/// ready for `pr.open`.
fn worked_case(repo: &Path) -> (String, String) {
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
    let worked = work(repo, &id, "fixed\n");
    (id, worked)
}

/// Commits `content` into `check.txt` on the case's branch and records the tests and the gate
/// green at the commit; the commit.
fn work(repo: &Path, id: &str, content: &str) -> String {
    fs::write(repo.join("check.txt"), content).expect("a file");
    git(repo, &["commit", "-q", "-am", "the work"]);
    let worked = git(repo, &["rev-parse", "HEAD"]);
    let forge: Rc<dyn Forge> = Rc::new(Memory::default());
    let validators = validators(forge);
    let mut run = Run::resume(repo, options(Some(1), None), id, &NoHarness, &validators)
        .expect("the case resumes");
    assert_eq!(
        run.case.revision("implementation").as_deref(),
        Some(worked.as_str())
    );
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
    worked
}

/// Resumes the case `id` through `forge` with `harness` and runs it: the stop, or the error,
/// and what it printed.
fn resumed_run(
    repo: &Path,
    id: &str,
    options: RunOptions,
    forge: Rc<dyn Forge>,
    harness: &dyn Harness,
) -> (Result<Stop, String>, String) {
    let validators = validators(forge);
    let mut run = Run::resume(repo, options, id, harness, &validators).expect("the case resumes");
    run.poll = POLL;
    let mut out = Vec::new();
    let stop = run.run(&mut out).map_err(|error| format!("{error:#}"));
    (stop, String::from_utf8_lossy(&out).into_owned())
}

/// The records of `kind` the case holds, each as its JSON.
fn records(repo: &Path, id: &str, kind: &str) -> Vec<serde_json::Value> {
    let dir = repo.join(CASES_DIR).join(id).join("evidence");
    let Ok(entries) = fs::read_dir(&dir) else {
        return Vec::new();
    };
    entries
        .map(|entry| {
            serde_json::from_str(
                &fs::read_to_string(entry.expect("an entry").path()).expect("a file"),
            )
            .expect("json")
        })
        .filter(|record: &serde_json::Value| record["kind"] == kind)
        .collect()
}

/// The story's Expected before: checks pending for the first three reads, a run with budget
/// left. It reads them again after a bounded wait, offering the harness nothing meanwhile,
/// records `ci_result pass` on the fourth read and stops at the merge's approval, where it
/// stopped `NoProgress` after the first read.
#[test]
fn checks_pending_for_three_reads_are_read_again_until_they_pass_and_the_run_reaches_the_gate() {
    let repo = repository("pending-three");
    let (id, worked) = worked_case(&repo);
    let forge = Rc::new(PendingFor::new(3));
    let harness = Gatekeeper::default();

    let (stop, text) = resumed_run(&repo, &id, options(Some(20), None), forge.clone(), &harness);

    assert_eq!(
        stop,
        Ok(Stop::ApprovalRequired(vec!["repository.merge".to_owned()])),
        "{text}"
    );
    assert_eq!(forge.checks_read(), 4, "{text}");
    assert_eq!(
        harness.offers.get(),
        0,
        "a judgement step was offered while the checks were pending: {text}"
    );
    let ci = records(&repo, &id, "ci_result");
    assert_eq!(ci.len(), 1, "{text}");
    assert_eq!(ci[0]["result"], "pass");
    assert_eq!(ci[0]["subject_revision"], worked.as_str());
    assert_eq!(text.matches("  waiting: ").count(), 3, "{text}");
}

/// The budget ends while the checks are still pending: the run stops `Waiting` on `ci.read`
/// with the budget it spent, never `NoProgress`, records nothing and offers the harness
/// nothing; the next run, through `open_next`, resumes the same case and, the checks passed,
/// records `ci_result pass` and reaches the gate.
#[test]
fn a_step_budget_that_ends_while_the_checks_are_pending_stops_waiting_and_the_next_run_resumes() {
    let repo = repository("pending-budget");
    let (id, _) = worked_case(&repo);
    let forge = Rc::new(PendingFor::new(usize::MAX));
    let harness = Gatekeeper::default();

    let (stop, text) = resumed_run(&repo, &id, options(Some(4), None), forge.clone(), &harness);

    assert_eq!(
        stop,
        Ok(Stop::Waiting {
            on: vec!["ci.read".to_owned()],
            budget: Some(Box::new(Stop::Iterations(4))),
        }),
        "{text}"
    );
    let shown = stop.expect("a stop").to_string();
    assert!(shown.starts_with("Waiting (ci.read"), "{shown}");
    assert_eq!(
        forge.checks_read(),
        3,
        "pr.open, then ci.read three times: {text}"
    );
    assert_eq!(harness.offers.get(), 0, "{text}");
    assert!(records(&repo, &id, "ci_result").is_empty(), "{text}");

    forge.pending.set(0);
    let validators = validators(forge.clone());
    let mut out = Vec::new();
    let mut run = Run::open_next(
        &repo,
        options(Some(20), None),
        &Aep::default(),
        &harness,
        &validators,
        &mut out,
    )
    .expect("the frontier answers")
    .expect("the case is resumed");
    assert_eq!(run.case.id, id, "{}", String::from_utf8_lossy(&out));
    run.poll = POLL;
    let stop = run.run(&mut out).expect("the run stops");
    let text = String::from_utf8_lossy(&out);
    assert_eq!(
        stop,
        Stop::ApprovalRequired(vec!["repository.merge".to_owned()]),
        "{text}"
    );
    assert_eq!(records(&repo, &id, "ci_result").len(), 1, "{text}");
}

/// An hour budget bounds the wait as a step budget does: a poll longer than what is left of
/// the hours is cut to it, and the run stops `Waiting` with `Hours` spent, well before the poll.
#[test]
fn an_hour_budget_cuts_the_wait_and_stops_waiting() {
    let repo = repository("pending-hours");
    let (id, _) = worked_case(&repo);
    let forge = Rc::new(PendingFor::new(usize::MAX));
    let validators = validators(forge.clone());
    let hours = 3.0 / 3600.0;
    let mut run = Run::resume(
        &repo,
        options(None, Some(hours)),
        &id,
        &NoHarness,
        &validators,
    )
    .expect("the case resumes");
    run.poll = Duration::from_secs(30);
    let mut out = Vec::new();
    let started = Instant::now();
    let stop = run.run(&mut out).map_err(|error| format!("{error:#}"));
    let text = String::from_utf8_lossy(&out);
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "waited {:?}: {text}",
        started.elapsed()
    );
    assert_eq!(
        stop,
        Ok(Stop::Waiting {
            on: vec!["ci.read".to_owned()],
            budget: Some(Box::new(Stop::Hours(hours))),
        }),
        "{text}"
    );
    assert!(forge.checks_read() >= 1, "{text}");
}

/// A run with neither a step nor an hour budget has nothing to wait in: it stops `Waiting` at
/// the first pending read, with no budget named, and never offers the harness a step.
#[test]
fn a_run_with_no_step_or_hour_budget_stops_waiting_at_once() {
    let repo = repository("pending-unbounded");
    let (id, _) = worked_case(&repo);
    let forge = Rc::new(PendingFor::new(usize::MAX));
    let harness = Gatekeeper::default();

    let (stop, text) = resumed_run(&repo, &id, options(None, None), forge.clone(), &harness);

    assert_eq!(
        stop,
        Ok(Stop::Waiting {
            on: vec!["ci.read".to_owned()],
            budget: None,
        }),
        "{text}"
    );
    assert_eq!(forge.checks_read(), 1, "{text}");
    assert_eq!(harness.offers.get(), 0, "{text}");
}

/// The double, its pull request's head reported at the head the case's branch had before the
/// last push that moved it, for the reads `from..=to` counted since that push (or since
/// [`Lagging::lag`]), as a forge whose pull request lags the push; every other read reports the
/// head pushed.
struct Lagging {
    memory: Rc<Memory>,
    previous: std::cell::RefCell<Option<String>>,
    views: Cell<usize>,
    stale: Cell<(usize, usize)>,
}

impl Lagging {
    fn new(memory: Rc<Memory>) -> Self {
        Self {
            memory,
            previous: std::cell::RefCell::new(None),
            views: Cell::new(0),
            stale: Cell::new((1, 0)),
        }
    }

    /// From now on, the reads `from..=to` report the head before the last push.
    fn lag(&self, from: usize, to: usize) {
        self.views.set(0);
        self.stale.set((from, to));
    }

    fn record_push(&self, branch: &str, pushing: &str) {
        let before = self.memory.pushed.borrow().get(branch).cloned();
        if before.as_deref() != Some(pushing) {
            *self.previous.borrow_mut() = before;
            self.views.set(0);
        }
    }
}

impl Forge for Lagging {
    fn name(&self) -> &str {
        "lagging"
    }

    fn push(&self, tree: &Path, branch: &str) -> anyhow::Result<()> {
        let head = git(tree, &["rev-parse", "HEAD"]);
        self.record_push(branch, &head);
        self.memory.push(tree, branch)
    }

    fn push_revision(&self, tree: &Path, branch: &str, revision: &str) -> anyhow::Result<()> {
        self.record_push(branch, revision);
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
        let mut view = self.memory.pull_request(tree, number)?;
        let n = self.views.get() + 1;
        self.views.set(n);
        let (from, to) = self.stale.get();
        if (from..=to).contains(&n)
            && let Some(previous) = self.previous.borrow().clone()
        {
            view.head = Head {
                branch: view.head.branch,
                sha: previous,
            };
        }
        Ok(view)
    }

    fn checks(&self, tree: &Path, number: u64) -> anyhow::Result<Checks> {
        self.memory.checks(tree, number)
    }

    fn merge(&self, tree: &Path, dir: &Path, number: u64, head: &str) -> anyhow::Result<()> {
        self.memory.merge(tree, dir, number, head)
    }
}

/// The records of `kind` the case holds about `revision`.
fn records_at(repo: &Path, id: &str, kind: &str, revision: &str) -> Vec<serde_json::Value> {
    records(repo, id, kind)
        .into_iter()
        .filter(|record| record["subject_revision"] == revision)
        .collect()
}

/// A case whose pull request was opened, and its checks read green, at a first revision
/// through `lagging` (lagging nothing yet), then a second commit with the tests and the gate
/// green on it: the case id and the second revision.
fn moved_past_an_open_pull_request(repo: &Path, lagging: &Rc<Lagging>) -> (String, String) {
    let (id, first) = worked_case(repo);
    let (stop, text) = resumed_run(
        repo,
        &id,
        options(Some(20), None),
        lagging.clone(),
        &Gatekeeper::default(),
    );
    assert_eq!(
        stop,
        Ok(Stop::ApprovalRequired(vec!["repository.merge".to_owned()])),
        "the first revision {} reaches the gate: {text}",
        &first[..8]
    );
    let second = work(repo, &id, "fixed again\n");
    (id, second)
}

fn lagging() -> Rc<Lagging> {
    Rc::new(Lagging::new(Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    })))
}

/// The trial's first shape, `pr.open` after its own push: `pull request #N has the head <branch>
/// at <earlier>, not the case's branch at the revision`. While the head lags past the budget the
/// run stops `Waiting` on `pr.open`; once it catches up after two reads, the next run records
/// the pull request open at the new revision and reaches the gate, no judgement step offered.
#[test]
fn shape_pr_open_a_head_lagging_its_own_push_is_waited_on_and_recorded_once_it_catches_up() {
    let repo = repository("lag-pr-open");
    let lagging = lagging();
    let (id, second) = moved_past_an_open_pull_request(&repo, &lagging);
    let harness = Gatekeeper::default();

    lagging.lag(1, usize::MAX);
    let (stop, text) = resumed_run(
        &repo,
        &id,
        options(Some(4), None),
        lagging.clone(),
        &harness,
    );
    match &stop {
        Ok(Stop::Waiting { on, budget }) => {
            assert!(on.contains(&"pr.open".to_owned()), "{stop:?}\n{text}");
            assert_eq!(budget.as_deref(), Some(&Stop::Iterations(4)), "{text}");
        }
        other => panic!("not waiting on pr.open: {other:?}\n{text}"),
    }
    assert!(
        text.contains("not the case's branch at the revision; no record"),
        "{text}"
    );
    assert!(
        records_at(&repo, &id, "pull_request", &second).is_empty(),
        "{text}"
    );

    lagging.lag(1, 2);
    let (stop, text) = resumed_run(
        &repo,
        &id,
        options(Some(20), None),
        lagging.clone(),
        &harness,
    );
    assert_eq!(
        stop,
        Ok(Stop::ApprovalRequired(vec!["repository.merge".to_owned()])),
        "{text}"
    );
    assert!(
        text.contains("not the case's branch at the revision; no record"),
        "{text}"
    );
    assert!(text.contains("  waiting: "), "{text}");
    assert_eq!(harness.offers.get(), 0, "{text}");
    assert_eq!(
        records_at(&repo, &id, "pull_request", &second).len(),
        1,
        "{text}"
    );
    let ci = records_at(&repo, &id, "ci_result", &second);
    assert_eq!(ci.len(), 1, "{text}");
    assert_eq!(ci[0]["result"], "pass");
}

/// The trial's third shape, `ci.read` after `pr.open` saw the head: `pull request #N has the
/// head <branch> at <earlier>, not <branch> at <revision>; nothing to read`. While it lags past
/// the budget the run stops `Waiting` on `ci.read`; once it catches up after two reads, the next
/// run records `ci_result pass` at the new revision and reaches the gate.
#[test]
fn shape_ci_read_a_head_lagging_the_push_is_waited_on_and_read_once_it_catches_up() {
    let repo = repository("lag-ci-read");
    let lagging = lagging();
    let (id, second) = moved_past_an_open_pull_request(&repo, &lagging);
    let harness = Gatekeeper::default();

    // The first read after the push, `pr.open`'s, sees the head; `ci.read`'s do not.
    lagging.lag(2, usize::MAX);
    let (stop, text) = resumed_run(
        &repo,
        &id,
        options(Some(4), None),
        lagging.clone(),
        &harness,
    );
    assert_eq!(
        stop,
        Ok(Stop::Waiting {
            on: vec!["ci.read".to_owned()],
            budget: Some(Box::new(Stop::Iterations(4))),
        }),
        "{text}"
    );
    let shape = format!("at {}; nothing to read", &second[..8]);
    assert!(text.contains(&shape), "{shape}: {text}");
    assert_eq!(
        records_at(&repo, &id, "pull_request", &second).len(),
        1,
        "{text}"
    );
    assert!(
        records_at(&repo, &id, "ci_result", &second).is_empty(),
        "{text}"
    );

    lagging.lag(1, 2);
    let (stop, text) = resumed_run(
        &repo,
        &id,
        options(Some(20), None),
        lagging.clone(),
        &harness,
    );
    assert_eq!(
        stop,
        Ok(Stop::ApprovalRequired(vec!["repository.merge".to_owned()])),
        "{text}"
    );
    assert!(text.contains(&shape), "{shape}: {text}");
    assert_eq!(text.matches("  waiting: ").count(), 2, "{text}");
    assert_eq!(harness.offers.get(), 0, "{text}");
    let ci = records_at(&repo, &id, "ci_result", &second);
    assert_eq!(ci.len(), 1, "{text}");
    assert_eq!(ci[0]["result"], "pass");
}

/// The trial's second shape, `repository.merge` after pushing the approval's commit: `pushed
/// <branch> at <approval>, and pull request #N has the head <branch> at <revision>; nothing
/// merged`. While it lags past the budget the run stops `Waiting` on the merge, nothing merged;
/// once it catches up after two reads, the next run merges the approval's head, the case
/// accepted with one merge observation about the granted revision.
#[test]
fn shape_merge_a_head_lagging_the_approval_push_is_waited_on_and_merged_once_it_catches_up() {
    let repo = repository("lag-merge");
    let lagging = lagging();
    let (id, granted) = worked_case(&repo);
    let (stop, text) = resumed_run(
        &repo,
        &id,
        options(Some(20), None),
        lagging.clone(),
        &Gatekeeper::default(),
    );
    assert_eq!(
        stop,
        Ok(Stop::ApprovalRequired(vec!["repository.merge".to_owned()])),
        "{text}"
    );
    approve(
        &repo,
        &Aep::default(),
        &id,
        "repository.merge",
        "human:operator",
    )
    .expect("the approval is given");
    let approved = git(&repo, &["rev-parse", "HEAD"]);

    // The read before the merge's push sees the granted revision (nothing was pushed over it
    // yet); every read after the push lags.
    lagging.lag(1, usize::MAX);
    let (stop, text) = resumed_run(
        &repo,
        &id,
        options(Some(4), None),
        lagging.clone(),
        &NoHarness,
    );
    assert_eq!(
        stop,
        Ok(Stop::Waiting {
            on: vec!["repository.merge".to_owned()],
            budget: Some(Box::new(Stop::Iterations(4))),
        }),
        "{text}"
    );
    let shape = format!(
        "at {} through lagging, and pull request #1 has the head takt/{id} at {}; nothing merged",
        &approved[..8],
        &granted[..8]
    );
    assert!(text.contains(&shape), "{shape}: {text}");
    assert!(lagging.memory.merged.borrow().is_empty(), "{text}");

    lagging.lag(1, 2);
    let (stop, text) = resumed_run(
        &repo,
        &id,
        options(Some(20), None),
        lagging.clone(),
        &NoHarness,
    );
    assert_eq!(stop, Ok(Stop::Completed("accepted".to_owned())), "{text}");
    assert!(text.contains(&shape), "{shape}: {text}");
    assert_eq!(lagging.memory.merged.borrow().as_slice(), [1], "{text}");
    let merged = records(&repo, &id, "merge_observation");
    assert_eq!(merged.len(), 1, "{text}");
    assert_eq!(merged[0]["subject_revision"], granted.as_str());
}

/// The double whose merge the forge refuses, the way the API refuses it, for a required check
/// that has not reported on the head, the first `refusals` times; its checks read pass, as a
/// read that answers for the head before the push does.
struct RefusesFor {
    memory: Rc<Memory>,
    refusals: Cell<usize>,
}

impl Forge for RefusesFor {
    fn name(&self) -> &str {
        "refuses-for"
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
        self.memory.checks(tree, number)
    }

    fn merge(&self, tree: &Path, dir: &Path, number: u64, head: &str) -> anyhow::Result<()> {
        let left = self.refusals.get();
        if left > 0 {
            self.refusals.set(left - 1);
            bail!(
                "b10x-gates api --method PUT --path /repos/o/r/pulls/{number}/merge exited 1: 405 Required status check \"task check\" is expected"
            );
        }
        self.memory.merge(tree, dir, number, head)
    }
}

/// A merge the forge refuses for a required check that has not reported is nothing merged and
/// no record, not an error: the run waits and merges once the forge takes it, the case
/// accepted with one merge observation.
#[test]
fn a_merge_refused_for_a_pending_required_check_is_waited_on_and_merged() {
    let repo = repository("merge-refused");
    let (id, granted) = worked_case(&repo);
    let memory = Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    });
    let (stop, text) = resumed_run(
        &repo,
        &id,
        options(Some(20), None),
        memory.clone(),
        &Gatekeeper::default(),
    );
    assert_eq!(
        stop,
        Ok(Stop::ApprovalRequired(vec!["repository.merge".to_owned()])),
        "{text}"
    );
    approve(
        &repo,
        &Aep::default(),
        &id,
        "repository.merge",
        "human:operator",
    )
    .expect("the approval is given");
    let forge = Rc::new(RefusesFor {
        memory: memory.clone(),
        refusals: Cell::new(2),
    });

    let (stop, text) = resumed_run(&repo, &id, options(Some(20), None), forge, &NoHarness);

    assert_eq!(stop, Ok(Stop::Completed("accepted".to_owned())), "{text}");
    assert_eq!(memory.merged.borrow().as_slice(), [1], "{text}");
    assert_eq!(text.matches("nothing merged").count(), 2, "{text}");
    let merged = records(&repo, &id, "merge_observation");
    assert_eq!(merged.len(), 1, "{text}");
    assert_eq!(merged[0]["subject_revision"], granted.as_str());
}

/// Only a refusal for a required check that has not reported is waited on: one for a check that
/// failed, and every other refusal of the merge, stays an error.
#[test]
fn only_a_refusal_for_a_check_that_has_not_reported_is_waited_on() {
    let refusal = |text: &str| anyhow!("{text}").context("merge pull request #1 through the forge");
    for waited in [
        "exited 1: 405 Required status check \"task check\" is expected",
        "exited 1: 405 2 of 3 required status checks are expected.",
        "exited 1: 405 Required status check \"task check\" is in progress",
        "exited 1: 405 Repository rule violations foundRequired status check \"a. b; c\" is expected.",
        "exited 1: 405 Required status check \"a\" is expected.; Required status check \"b\" is queued.",
    ] {
        assert!(
            takt::forge::refused_for_a_pending_check(&refusal(waited)),
            "{waited}"
        );
    }
    for refused in [
        "exited 1: 405 Required status check \"task check\" is failing",
        "exited 1: 405 Required status check \"a\" is expected.; Required status check \"b\" is failing.",
        "exited 1: 405 Repository rule violations foundWaiting on code owner review by @o/team.Required status check \"a\" is expected.",
        "exited 1: 405 Repository rule violations foundRequired status check \"a\" is expected.All comments must be resolved.",
        "exited 1: 405 Repository rule violations foundCommits must have verified signatures.Required status check \"a\" is expected.",
        "exited 1: 405 Required status check \"a is expected",
        "exited 1: 405 Required status check is expected",
        "exited 1: 405 ",
        "exited 1: 405 Pull Request is not mergeable",
        "exited 1: 409 Head branch was modified. Review and try the merge again.",
        "exited 1: 422 Unprocessable Entity",
        "push exited on a signal: the child outlived its time limit",
    ] {
        assert!(
            !takt::forge::refused_for_a_pending_check(&refusal(refused)),
            "{refused}"
        );
    }
}
