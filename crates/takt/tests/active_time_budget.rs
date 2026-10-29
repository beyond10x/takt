//! story:a-re-trial-stops-when-its-verdict-is-decided, rule 3: a run's active time, its wall
//! time less the time it waits on the forge, has a budget of its own
//! ([`Run::cap_active_time`], `--active-seconds`). A forge wait longer than the budget does not
//! spend it; steps that pass it stop the run `ActiveTime`, naming the budget, the active time
//! and the wait time.

#[path = "support/own_home.rs"]
mod own_home;

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

/// How many times the calibrating run's wall time a capped run's active-time budget is, before
/// the one second added to it ([`calibrated_budget`]).
const MARGIN: f64 = 3.0;

/// How many attempts a test makes, each calibrated afresh, before it gives up on a machine that
/// outruns every one ([`conclusive`]).
const ATTEMPTS: usize = 3;

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

/// The run's printed lines, each with the instant its line ended.
#[derive(Default)]
struct Stamped {
    lines: Vec<(Instant, String)>,
    partial: Vec<u8>,
}

impl std::io::Write for Stamped {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        for &byte in buf {
            if byte == b'\n' {
                let line = String::from_utf8_lossy(&std::mem::take(&mut self.partial)).into_owned();
                self.lines.push((Instant::now(), line));
            } else {
                self.partial.push(byte);
            }
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// The double: its checks pending for the first `pending` reads, then each read taking `slow`
/// before it answers its own answer (pass, or what [`Checked::answering`] names).
struct Checked {
    memory: Rc<Memory>,
    pending: Cell<usize>,
    slow: Duration,
    /// Each call made of it: its start, its end and its name.
    spans: std::cell::RefCell<Vec<(Instant, Instant, String)>>,
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
            spans: std::cell::RefCell::default(),
        }
    }

    fn timed<T>(&self, call: &str, f: impl FnOnce() -> T) -> T {
        let start = Instant::now();
        let answer = f();
        self.spans
            .borrow_mut()
            .push((start, Instant::now(), format!("forge {call}")));
        answer
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
        self.timed("target_tip", || self.memory.target_tip(tree, base))
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
        self.timed("push", || self.memory.push(tree, branch))
    }

    fn push_revision(&self, tree: &Path, branch: &str, revision: &str) -> anyhow::Result<()> {
        self.timed("push_revision", || {
            self.memory.push_revision(tree, branch, revision)
        })
    }

    fn open_pull_request(
        &self,
        tree: &Path,
        dir: &Path,
        request: &PullRequest,
    ) -> anyhow::Result<u64> {
        self.timed("open_pull_request", || {
            self.memory.open_pull_request(tree, dir, request)
        })
    }

    fn pull_request(&self, tree: &Path, number: u64) -> anyhow::Result<PullRequestView> {
        self.timed("pull_request", || self.memory.pull_request(tree, number))
    }

    fn checks(&self, tree: &Path, number: u64) -> anyhow::Result<Checks> {
        self.timed("checks", || {
            let answer = self.memory.checks(tree, number)?;
            let left = self.pending.get();
            if left == 0 {
                std::thread::sleep(self.slow);
                return Ok(answer);
            }
            self.pending.set(left - 1);
            Ok(Checks::Pending)
        })
    }

    fn merge(&self, tree: &Path, dir: &Path, number: u64, head: &str) -> anyhow::Result<()> {
        self.timed("merge", || self.memory.merge(tree, dir, number, head))
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

/// The active-time budget, in whole seconds, for a capped run of the scenario `name` on this
/// machine as loaded as it is now, and a line saying how it was found: [`MARGIN`] times the
/// wall time an uncapped run of the same steps took a moment before (a pull request opened, its
/// checks read pending three times with a 5 ms pause, then passing, the merge's approval
/// reached), plus one second. Every step a capped run here takes before its budget is meant to
/// be spent is among those steps, so the budget grows with the time they take under load, where
/// a fixed one assumed an idle machine (story:active-time-budget-tests-are-load-safe).
fn calibrated_budget(name: &str) -> (u64, String) {
    let repo = repository(&format!("{name}-calibration"));
    let id = worked_case(&repo);
    let forge: Rc<dyn Forge> = Rc::new(Checked::new(3, Duration::ZERO));
    let harness = Gatekeeper::default();
    let observers = validators(forge);
    let started = Instant::now();
    let mut run = Run::resume(&repo, options(Some(20)), &id, &harness, &observers)
        .expect("the calibrating case resumes");
    run.poll = Duration::from_millis(5);
    let mut out = Vec::new();
    let stop = run.run(&mut out).expect("the calibrating run stops");
    let steps = started.elapsed();
    assert_eq!(
        stop,
        Stop::ApprovalRequired(vec!["repository.merge".to_owned()]),
        "the calibrating run: {}",
        String::from_utf8_lossy(&out)
    );
    let budget = (steps.as_secs_f64() * MARGIN).ceil() as u64 + 1;
    (
        budget,
        format!("calibration: the same steps, uncapped, took {steps:?}; the budget is {budget} s"),
    )
}

/// One capped run of a scenario: its stop, what it printed, its wall time from before the
/// resume, where the run's own clock starts, to its stop, the double and the harness it ran
/// with, its poll and its active-time budget, and for a failure's message the calibration, the
/// output and a timeline of each printed line and each forge call against that clock.
struct Attempt {
    stop: Result<Stop, String>,
    text: String,
    wall: Duration,
    forge: Rc<Checked>,
    harness: Gatekeeper,
    poll: Duration,
    /// The active-time budget it ran under, in seconds.
    budget: u64,
    shown: String,
}

impl Attempt {
    /// Whether the machine outran this run: it stopped on its active budget, on its own account,
    /// before its checks were read `reads` times, the read the test is about; and that account
    /// agrees with what the test saw: it waited at least as long as the pauses it printed, and
    /// its active and waited times add up to no more than the wall time the test measured. Such
    /// a run met load its calibration did not foresee and says nothing about the claim either
    /// way. A run whose account disagrees is not outrun: the test judges it as it stands.
    fn outrun(&self, reads: usize) -> bool {
        let spent = match &self.stop {
            Ok(Stop::Waiting {
                budget: Some(stop), ..
            }) => stop.as_ref(),
            Ok(stop) => stop,
            Err(_) => return false,
        };
        let Stop::ActiveTime {
            budget,
            active,
            waited,
        } = *spent
        else {
            return false;
        };
        let pauses = self.text.matches("  waiting: ").count() as f64 * self.poll.as_secs_f64();
        self.forge.checks_read() < reads
            && active >= budget as f64
            && waited >= pauses
            && active + waited <= self.wall.as_secs_f64() + 0.01
    }
}

/// Resumes the case `id` through `forge`, its active time capped at `budget` seconds and its
/// poll `poll`, and runs it.
fn capped_run(
    repo: &Path,
    id: &str,
    forge: Rc<Checked>,
    poll: Duration,
    (budget, calibration): &(u64, String),
) -> Attempt {
    let harness = Gatekeeper::default();
    let observers = validators(forge.clone());
    let started = Instant::now();
    let mut run =
        Run::resume(repo, options(Some(20)), id, &harness, &observers).expect("the case resumes");
    let resumed = Instant::now();
    run.poll = poll;
    run.cap_active_time(*budget);
    let mut out = Stamped::default();
    let stop = run.run(&mut out).map_err(|error| format!("{error:#}"));
    let wall = started.elapsed();
    drop(run);
    let text: String = out
        .lines
        .iter()
        .map(|(_, line)| format!("{line}\n"))
        .collect();
    let mut events: Vec<(Instant, Instant, String)> = out
        .lines
        .into_iter()
        .map(|(at, line)| (at, at, format!("out   {line}")))
        .collect();
    events.push((started, resumed, "Run::resume".to_owned()));
    events.extend(forge.spans.borrow().iter().cloned());
    events.sort_by_key(|(_, end, _)| *end);
    let mut shown = format!("{calibration}\n{text}\ntimeline, wall {wall:?}:\n");
    let mut last = started;
    for (start, end, what) in events {
        shown.push_str(&format!(
            "{:8.3} s  +{:6.3} s since the last  {:6.3} s long  {what}\n",
            end.duration_since(started).as_secs_f64(),
            end.duration_since(last).as_secs_f64(),
            end.duration_since(start).as_secs_f64(),
        ));
        last = end;
    }
    Attempt {
        stop,
        text,
        wall,
        forge,
        harness,
        poll,
        budget: *budget,
        shown,
    }
}

/// Runs `scenario` with a budget calibrated afresh ([`calibrated_budget`]) until an attempt the
/// machine did not outrun before its checks' read `reads` ([`Attempt::outrun`]), and returns
/// it, the runs outrun before it named in its message; at most [`ATTEMPTS`] times, then it
/// panics with all of them. `scenario` gets a name for its scratch tree and the budget.
fn conclusive(
    name: &str,
    reads: usize,
    scenario: impl Fn(&str, &(u64, String)) -> Attempt,
) -> Attempt {
    let mut outrun = String::new();
    for attempt in 1..=ATTEMPTS {
        let name = format!("{name}-{attempt}");
        let calibrated = calibrated_budget(&name);
        let mut run = scenario(&name, &calibrated);
        if !run.outrun(reads) {
            run.shown = format!("{outrun}attempt {attempt}: {}", run.shown);
            return run;
        }
        outrun.push_str(&format!(
            "attempt {attempt}, outrun by the machine: {:?}\n{}\n",
            run.stop, run.shown
        ));
    }
    panic!("the machine outran every attempt, each calibrated afresh:\n{outrun}");
}

/// The candidate's first run of the 0.3.0 trial, small: the checks stay pending for three reads,
/// each followed by a pause of a third of the calibrated budget and a second, so the run waits
/// longer on the forge than its whole active budget. The wait spends none of it: the run reads
/// the checks a fourth time, records them and reaches the merge's approval.
#[test]
fn a_forge_wait_longer_than_the_active_budget_does_not_stop_the_run() {
    let run = conclusive("wait", 4, |name, calibrated| {
        let repo = repository(name);
        let id = worked_case(&repo);
        let poll = Duration::from_secs(calibrated.0 + 1) / 3;
        capped_run(
            &repo,
            &id,
            Rc::new(Checked::new(3, Duration::ZERO)),
            poll,
            calibrated,
        )
    });
    let shown = &run.shown;

    assert_eq!(
        run.stop,
        Ok(Stop::ApprovalRequired(vec!["repository.merge".to_owned()])),
        "{shown}"
    );
    assert!(
        run.wall > Duration::from_secs(run.budget),
        "the run's wall time {:?} passed the active budget: {shown}",
        run.wall
    );
    assert_eq!(run.forge.checks_read(), 4, "{shown}");
    assert_eq!(run.text.matches("  waiting: ").count(), 3, "{shown}");
    assert_eq!(run.harness.offers.get(), 0, "{shown}");
}

/// A step that takes longer than the budget (a checks read of the calibrated budget and half a
/// second, after one pending read and a 1 s pause): before the next step the run stops
/// `ActiveTime`, naming the budget, an active time past it and the time it waited, which is not
/// counted in it. The checks fail, so the next step is a judgement step: a case that only a
/// grant moves stops on that first, ahead of any budget
/// (approval-record:loop-pre-checks-ahead-of-loom).
#[test]
fn steps_that_pass_the_active_budget_stop_the_run_on_it() {
    let run = conclusive("steps", 2, |name, calibrated| {
        let repo = repository(name);
        let id = worked_case(&repo);
        let slow = Duration::from_secs(calibrated.0) + Duration::from_millis(500);
        capped_run(
            &repo,
            &id,
            Rc::new(Checked::answering(Checks::Fail, 1, slow)),
            Duration::from_secs(1),
            calibrated,
        )
    });
    let slow = run.forge.slow;
    let shown = &run.shown;

    let Ok(Stop::ActiveTime {
        budget,
        active,
        waited,
    }) = run.stop
    else {
        panic!("{:?}\n{shown}", run.stop);
    };
    assert_eq!(budget, run.budget, "{shown}");
    assert!(active >= slow.as_secs_f64(), "active {active}: {shown}");
    assert!(
        (1.0..slow.as_secs_f64()).contains(&waited),
        "waited {waited}: {shown}"
    );
    assert!(
        active + waited <= run.wall.as_secs_f64() + 0.01,
        "active {active} and waited {waited} add to more than the wall time {:?}: {shown}",
        run.wall
    );
    assert_eq!(run.forge.checks_read(), 2, "{shown}");
    assert_eq!(run.harness.offers.get(), 0, "{shown}");
    let text = Stop::ActiveTime {
        budget,
        active,
        waited,
    }
    .to_string();
    assert!(
        text.starts_with(&format!("ActiveTime ({budget} s; active ")) && text.contains("waited"),
        "{text}"
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
