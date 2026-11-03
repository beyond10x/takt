//! Adversary pass on story:ci-wait-watches-the-check: the forge wait's backoff across a second
//! run on the same case, the default backoff against a short hour budget, the record's
//! `waiting` seconds against the printed line, a forge error between two waits, and a tiny
//! poll. The runs go through a double of the forge; nothing here reaches a host.

#[path = "support/own_home.rs"]
mod own_home;

use std::cell::{Cell, RefCell};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail};
use takt::case::CASES_DIR;
use takt::evidence::Record;
use takt::forge::{Checks, Forge, Memory, PullRequest, PullRequestView};
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

fn repository(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("w49-adv-ci-wait-{}-{name}", std::process::id()));
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
        "w49-adv-ci-wait",
    )
}

/// The story taken, its work committed, the tests and the gate recorded green: ready for
/// `pr.open`. The case id.
fn worked_case(repo: &Path) -> String {
    let forge: Rc<dyn Forge> = Rc::new(Memory::default());
    let opening = validators(forge);
    let mut out = Vec::new();
    let opened = Run::open_next(
        repo,
        options(Some(1), None),
        &Aep::default(),
        &NoHarness,
        &opening,
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
    let validators = validators(forge);
    let mut run = Run::resume(repo, options(Some(1), None), &id, &NoHarness, &validators)
        .expect("the case resumes");
    run.case
        .add_evidence(&record(1, "test_result", &worked))
        .expect("written");
    run.case
        .add_evidence(&record(2, "gate_result", &worked))
        .expect("written");
    run.case
        .add_evidence(&Record::new(
            format!("handover-{}", &worked[..8]),
            "handover",
            None,
            "implementation",
            &worked,
            takt::run::CHOICE_PRODUCER,
        ))
        .expect("written");
    run.case
        .add_evidence(&Record::new(
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

/// A forge whose checks stay pending; the `fail_at`-th read of the checks (from 1), when set,
/// fails as a forge that did not answer.
struct Pending {
    memory: Rc<Memory>,
    fail_at: Cell<Option<usize>>,
    reads: Cell<usize>,
    at: RefCell<Vec<Instant>>,
}

impl Pending {
    fn new(fail_at: Option<usize>) -> Self {
        Self {
            memory: Rc::new(Memory {
                checks: Checks::Pass,
                ..Memory::default()
            }),
            fail_at: Cell::new(fail_at),
            reads: Cell::new(0),
            at: RefCell::new(Vec::new()),
        }
    }
}

impl Forge for Pending {
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
        "pending"
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
    ) -> takt::forge::Written<u64> {
        self.memory.open_pull_request(tree, dir, request)
    }

    fn pull_request(&self, tree: &Path, number: u64) -> anyhow::Result<PullRequestView> {
        self.memory.pull_request(tree, number)
    }

    fn checks(&self, tree: &Path, number: u64) -> anyhow::Result<Checks> {
        self.memory.checks(tree, number)?;
        let n = self.reads.get() + 1;
        self.reads.set(n);
        self.at.borrow_mut().push(Instant::now());
        if self.fail_at.get() == Some(n) {
            bail!("the forge did not answer read {n} of the checks");
        }
        Ok(Checks::Pending)
    }

    fn merge(&self, tree: &Path, dir: &Path, number: u64, head: &str) -> takt::forge::Written<()> {
        self.memory.merge(tree, dir, number, head)
    }
}

fn resumed_run(
    repo: &Path,
    id: &str,
    options: RunOptions,
    forge: Rc<dyn Forge>,
    poll: Option<Duration>,
) -> (Result<Stop, String>, String) {
    let validators = validators(forge);
    let mut run =
        Run::resume(repo, options, id, &NoHarness, &validators).expect("the case resumes");
    if let Some(poll) = poll {
        run.poll = poll;
    }
    let mut out = Vec::new();
    let stop = run.run(&mut out).map_err(|error| format!("{error:#}"));
    (stop, String::from_utf8_lossy(&out).into_owned())
}

/// The case's record, line by line.
fn lines(repo: &Path, id: &str) -> Vec<serde_json::Value> {
    let path = repo
        .join(CASES_DIR)
        .join(id)
        .join(takt::events::EVENTS_FILE);
    fs::read_to_string(&path)
        .expect("the record")
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("json"))
        .collect()
}

/// The seconds of each `waiting` line of the record, in order.
fn waits(repo: &Path, id: &str) -> Vec<f64> {
    lines(repo, id)
        .iter()
        .filter(|line| line["event"] == "waiting")
        .map(|line| line["seconds"].as_f64().expect("seconds"))
        .collect()
}

/// The seconds each printed `waiting:` line names.
fn printed(text: &str) -> Vec<f64> {
    text.lines()
        .filter_map(|line| line.strip_prefix("  waiting: "))
        .map(|rest| {
            rest.split(" s on the forge")
                .next()
                .expect("seconds")
                .parse::<f64>()
                .expect("a number")
        })
        .collect()
}

fn grown(first: Duration, n: usize) -> Vec<f64> {
    let cap = first.as_secs_f64() * 2.25;
    let mut wait = first.as_secs_f64();
    let mut all = Vec::new();
    for _ in 0..n {
        all.push(wait.min(cap));
        wait = (wait * 1.5).min(cap);
    }
    all
}

fn close(a: &[f64], b: &[f64]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(a, b)| (a - b).abs() < 1e-6)
}

/// A second `takt run` on a case the first left waiting on pending checks starts its backoff
/// at the first interval again, not where the first run's stopped.
#[test]
fn a_second_run_on_the_case_starts_the_backoff_at_the_first_interval() {
    let repo = repository("resumed");
    let id = worked_case(&repo);
    let first = Duration::from_millis(4);
    let forge = Rc::new(Pending::new(None));

    let (stop, text) = resumed_run(
        &repo,
        &id,
        options(Some(10), None),
        forge.clone(),
        Some(first),
    );
    assert!(matches!(stop, Ok(Stop::Waiting { .. })), "{stop:?}\n{text}");
    let before = waits(&repo, &id);
    assert!(before.len() >= 6, "{before:?}\n{text}");
    assert!(
        close(&before, &grown(first, before.len())),
        "{before:?}, story:ci-wait-watches-the-check\n{text}"
    );

    let (stop, text) = resumed_run(&repo, &id, options(Some(6), None), forge, Some(first));
    assert!(matches!(stop, Ok(Stop::Waiting { .. })), "{stop:?}\n{text}");
    let after: Vec<f64> = waits(&repo, &id)[before.len()..].to_vec();
    assert!(!after.is_empty(), "{text}");
    assert!(
        close(&after, &grown(first, after.len())),
        "the second run waited {after:?}, not from the first interval {:?}, story:ci-wait-watches-the-check\n{text}",
        grown(first, after.len())
    );
}

/// The default backoff, no poll set, against a few seconds of hour budget: the run stops
/// `Waiting` with `Hours` spent; every wait but the last is the default sequence from 2 s, the
/// last is no longer than its step and no longer than what was left; the printed line says each
/// recorded wait to a tenth of a second.
#[test]
fn the_default_backoff_is_cut_to_a_short_hour_budget_and_printed_as_recorded() {
    for seconds in [4.0_f64, 8.0, 16.0] {
        let repo = repository(&format!("default-{seconds}"));
        let id = worked_case(&repo);
        let forge = Rc::new(Pending::new(None));
        let hours = seconds / 3600.0;
        let started = Instant::now();
        let (stop, text) = resumed_run(&repo, &id, options(None, Some(hours)), forge.clone(), None);
        let elapsed = started.elapsed();
        if stop == Ok(Stop::Hours(hours)) && forge.reads.get() == 0 {
            continue;
        }
        assert_eq!(
            stop,
            Ok(Stop::Waiting {
                on: vec!["ci.read".to_owned()],
                budget: Some(Box::new(Stop::Hours(hours))),
                local: Vec::new(),
            }),
            "{text}"
        );
        let recorded = waits(&repo, &id);
        assert!(!recorded.is_empty(), "{text}");
        let full = &recorded[..recorded.len() - 1];
        let first = Duration::from_secs(2);
        assert!(
            close(full, &grown(first, full.len())),
            "the default waits {full:?} are not 2, 3, 4.5, ... s, story:ci-wait-watches-the-check\n{text}"
        );
        let last = *recorded.last().expect("a wait");
        let step = grown(first, recorded.len())[recorded.len() - 1];
        assert!(
            last <= step + 1e-9 && recorded.iter().sum::<f64>() <= seconds + 1e-6,
            "the waits {recorded:?} overrun a {seconds} s budget, story:ci-wait-watches-the-check\n{text}"
        );
        assert!(
            elapsed < Duration::from_secs_f64(seconds + 10.0),
            "waited {elapsed:?} on a {seconds} s budget\n{text}"
        );
        let said = printed(&text);
        assert_eq!(said.len(), recorded.len(), "{text}");
        assert!(
            said.iter()
                .zip(&recorded)
                .all(|(said, recorded)| (said - recorded).abs() <= 0.05 + 1e-9),
            "printed {said:?}, recorded {recorded:?}, story:ci-wait-watches-the-check"
        );
        return;
    }
    panic!("every hour budget ran out before the run read the checks once");
}

/// A forge that fails a read of the checks between two waits ends the run on that error, its
/// record ending on an `error` line; the next run on the case waits from the first interval.
#[test]
fn a_forge_error_between_waits_ends_the_run_and_the_next_starts_the_backoff_again() {
    let repo = repository("forge-error");
    let id = worked_case(&repo);
    let first = Duration::from_millis(4);
    let forge = Rc::new(Pending::new(Some(4)));

    let (stop, text) = resumed_run(
        &repo,
        &id,
        options(Some(20), None),
        forge.clone(),
        Some(first),
    );
    let error = stop.expect_err(&text);
    assert!(
        error.contains("the forge did not answer read 4 of the checks"),
        "{error}\n{text}"
    );
    let record = lines(&repo, &id);
    assert_eq!(
        record.last().map(|line| line["event"].clone()),
        Some(serde_json::json!("error")),
        "{record:?}"
    );
    let before = waits(&repo, &id);
    assert!(
        close(&before, &grown(first, 3)),
        "{before:?}, story:ci-wait-watches-the-check\n{text}"
    );

    forge.fail_at.set(None);
    let (stop, text) = resumed_run(&repo, &id, options(Some(6), None), forge, Some(first));
    assert!(matches!(stop, Ok(Stop::Waiting { .. })), "{stop:?}\n{text}");
    let after: Vec<f64> = waits(&repo, &id)[before.len()..].to_vec();
    assert!(
        !after.is_empty() && close(&after, &grown(first, after.len())),
        "after the error the next run waited {after:?}, story:ci-wait-watches-the-check\n{text}"
    );
}

/// A poll of one nanosecond still grows (never stuck at its first interval by rounding) and
/// stops at 2.25 times it; the run spends its step budget and stops `Waiting`.
#[test]
fn a_tiny_poll_still_grows_and_is_capped() {
    let repo = repository("tiny");
    let id = worked_case(&repo);
    let (stop, text) = resumed_run(
        &repo,
        &id,
        options(Some(12), None),
        Rc::new(Pending::new(None)),
        Some(Duration::from_nanos(1)),
    );
    assert!(matches!(stop, Ok(Stop::Waiting { .. })), "{stop:?}\n{text}");
    let recorded = waits(&repo, &id);
    assert!(recorded.len() >= 6, "{recorded:?}");
    assert!(
        recorded.windows(2).all(|w| w[1] >= w[0]),
        "{recorded:?}, story:ci-wait-watches-the-check"
    );
    assert!(
        recorded.last().copied().unwrap_or_default() > recorded[0],
        "a 1 ns poll never grew: {recorded:?}, story:ci-wait-watches-the-check"
    );
    assert!(
        recorded.iter().all(|s| *s <= 2.25e-9 + 1e-15),
        "a wait past 2.25 ns: {recorded:?}, story:ci-wait-watches-the-check"
    );
}
