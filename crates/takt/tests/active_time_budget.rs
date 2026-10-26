//! story:a-re-trial-stops-when-its-verdict-is-decided, rule 3: a run's active time, its wall
//! time less the time it waits on the forge, has a budget of its own
//! ([`Run::cap_active_time`], `--active-seconds`). A forge wait longer than the budget does not
//! spend it; steps that pass it stop the run `ActiveTime`, naming the budget, the active time
//! and the wait time.

use std::cell::Cell;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;
use std::time::{Duration, Instant};

use anyhow::anyhow;
use takt::evidence::Record;
use takt::forge::{Checks, Forge, Memory, PullRequest, PullRequestView};
use takt::frontier::Aep;
use takt::harness::{Harness, StepContext, StepReport};
use takt::validator::{CiRead, PrMerged, PrOpen, Validator};
use takt::{Run, RunOptions, Stop};

const PROTOCOL: &str = "story.delivery/1";

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");

/// The active-time budget of the runs here, in seconds.
const BUDGET: u64 = 2;

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
        .join(format!("active-time-budget-{}-{name}", std::process::id()));
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
/// inspection that changes nothing.
#[derive(Default)]
struct Gatekeeper {
    offers: Cell<usize>,
}

impl Harness for Gatekeeper {
    fn name(&self) -> &str {
        "gatekeeper"
    }

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

fn options(iterations: Option<u64>) -> RunOptions {
    RunOptions {
        protocol: PROTOCOL.to_owned(),
        intent: String::new(),
        inputs: Vec::new(),
        iterations,
        hours: None,
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

fn record(id: String, kind: &str, result: Option<&str>, revision: &str, producer: &str) -> Record {
    Record::new(
        id,
        kind,
        result.map(str::to_owned),
        "implementation",
        revision,
        producer,
    )
}

/// The double: its checks pending for the first `pending` reads, then each read taking `slow`
/// before it answers its own answer (pass, or what [`Checked::answering`] names).
struct Checked {
    memory: Rc<Memory>,
    pending: Cell<usize>,
    slow: Duration,
}

impl Checked {
    fn new(pending: usize, slow: Duration) -> Self {
        Self::answering(Checks::Pass, pending, slow)
    }

    /// The double answering `checks` once no read is left pending.
    fn answering(checks: Checks, pending: usize, slow: Duration) -> Self {
        Self {
            memory: Rc::new(Memory {
                checks,
                ..Memory::default()
            }),
            pending: Cell::new(pending),
            slow,
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

impl Forge for Checked {
    fn target_tip(&self, tree: &Path, base: &str) -> anyhow::Result<String> {
        self.memory.target_tip(tree, base)
    }

    fn scan_outgoing(
        &self,
        _: &Path,
        _: &Path,
        _: &takt::forge::Outgoing<'_>,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    fn name(&self) -> &str {
        "checked"
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
            std::thread::sleep(self.slow);
            return Ok(answer);
        }
        self.pending.set(left - 1);
        Ok(Checks::Pending)
    }

    fn merge(&self, tree: &Path, dir: &Path, number: u64, head: &str) -> anyhow::Result<()> {
        self.memory.merge(tree, dir, number, head)
    }
}

/// The story taken, its work committed on the case's branch, and the tests, the gate, the
/// handover and the change recorded at that revision: the case id. The case is ready for
/// `pr.open`.
fn worked_case(repo: &Path) -> String {
    let forge: Rc<dyn Forge> = Rc::new(Memory::default());
    let takers = validators(forge);
    let mut out = Vec::new();
    let opened = Run::open_next(
        repo,
        options(Some(1)),
        &Aep::default(),
        &NoHarness,
        &takers,
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
    let observers = validators(forge);
    let mut run =
        Run::resume(repo, options(Some(1)), &id, &NoHarness, &observers).expect("resumes");
    let short = &worked[..8];
    for (id, kind, result, producer) in [
        (
            format!("e1-{short}"),
            "test_result",
            Some("pass"),
            "active-time",
        ),
        (
            format!("e2-{short}"),
            "gate_result",
            Some("pass"),
            "active-time",
        ),
        (
            format!("handover-{short}"),
            "handover",
            None,
            takt::run::CHOICE_PRODUCER,
        ),
        (
            format!("change-record-{short}"),
            "change_record",
            Some("ahead"),
            "takt.change-read",
        ),
    ] {
        run.case
            .add_evidence(&record(id, kind, result, &worked, producer))
            .expect("written");
    }
    id
}

/// Resumes the case `id` through `forge`, its active time capped at [`BUDGET`] and its poll
/// `poll`, and runs it: the stop, what it printed, and the wall time from before the resume,
/// where the run's own clock starts, to its stop.
fn capped_run(
    repo: &Path,
    id: &str,
    forge: Rc<dyn Forge>,
    harness: &dyn Harness,
    poll: Duration,
) -> (Result<Stop, String>, String, Duration) {
    let observers = validators(forge);
    let started = Instant::now();
    let mut run =
        Run::resume(repo, options(Some(20)), id, harness, &observers).expect("the case resumes");
    run.poll = poll;
    run.cap_active_time(BUDGET);
    let mut out = Vec::new();
    let stop = run.run(&mut out).map_err(|error| format!("{error:#}"));
    let wall = started.elapsed();
    (stop, String::from_utf8_lossy(&out).into_owned(), wall)
}

/// The candidate's first run of the 0.3.0 trial, small: the checks stay pending for three reads,
/// each followed by a 1.5 s pause, so the run waits 4.5 s on the forge against an active budget
/// of 2 s. The wait spends none of it: the run reads the checks a fourth time, records them and
/// reaches the merge's approval.
#[test]
fn a_forge_wait_longer_than_the_active_budget_does_not_stop_the_run() {
    let repo = repository("wait");
    let id = worked_case(&repo);
    let forge = Rc::new(Checked::new(3, Duration::ZERO));
    let harness = Gatekeeper::default();

    let (stop, text, wall) = capped_run(
        &repo,
        &id,
        forge.clone(),
        &harness,
        Duration::from_millis(1500),
    );

    assert_eq!(
        stop,
        Ok(Stop::ApprovalRequired(vec!["repository.merge".to_owned()])),
        "{text}"
    );
    assert!(
        wall > Duration::from_secs(BUDGET),
        "the run's wall time {wall:?} passed the active budget: {text}"
    );
    assert_eq!(forge.checks_read(), 4, "{text}");
    assert_eq!(text.matches("  waiting: ").count(), 3, "{text}");
    assert_eq!(harness.offers.get(), 0, "{text}");
}

/// A step that takes longer than the budget (a checks read of 2.5 s, after one pending read and
/// a 1 s pause): before the next step the run stops `ActiveTime`, naming the budget, an active
/// time past it and the time it waited, which is not counted in it. The checks fail, so the
/// next step is a judgement step: a case that only a grant moves stops on that first, ahead of
/// any budget (approval-record:loop-pre-checks-ahead-of-loom).
#[test]
fn steps_that_pass_the_active_budget_stop_the_run_on_it() {
    let repo = repository("steps");
    let id = worked_case(&repo);
    let forge = Rc::new(Checked::answering(
        Checks::Fail,
        1,
        Duration::from_millis(2500),
    ));
    let harness = Gatekeeper::default();

    let (stop, text, wall) =
        capped_run(&repo, &id, forge.clone(), &harness, Duration::from_secs(1));

    let Ok(Stop::ActiveTime {
        budget,
        active,
        waited,
    }) = stop
    else {
        panic!("{stop:?}\n{text}");
    };
    assert_eq!(budget, BUDGET, "{text}");
    assert!(active >= 2.5, "active {active}: {text}");
    assert!((1.0..2.5).contains(&waited), "waited {waited}: {text}");
    assert!(
        active + waited <= wall.as_secs_f64() + 0.01,
        "active {active} and waited {waited} add to more than the wall time {wall:?}: {text}"
    );
    assert_eq!(forge.checks_read(), 2, "{text}");
    assert_eq!(harness.offers.get(), 0, "{text}");
    let shown = Stop::ActiveTime {
        budget,
        active,
        waited,
    }
    .to_string();
    assert!(
        shown.starts_with(&format!("ActiveTime ({BUDGET} s; active ")) && shown.contains("waited"),
        "{shown}"
    );
}

/// Without a cap the same run is not stopped on active time: the cap is off by default.
#[test]
fn a_run_with_no_active_budget_is_not_stopped_on_it() {
    let repo = repository("uncapped");
    let id = worked_case(&repo);
    let forge = Rc::new(Checked::new(0, Duration::from_millis(2500)));
    let harness = Gatekeeper::default();
    let observers = validators(forge.clone());
    let mut run =
        Run::resume(&repo, options(Some(20)), &id, &harness, &observers).expect("resumes");
    run.poll = Duration::from_millis(5);
    let mut out = Vec::new();
    let stop = run.run(&mut out).expect("the run stops");
    let text = String::from_utf8_lossy(&out);
    assert_eq!(
        stop,
        Stop::ApprovalRequired(vec!["repository.merge".to_owned()]),
        "{text}"
    );
}
