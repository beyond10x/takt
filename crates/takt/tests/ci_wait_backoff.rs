//! story:ci-wait-watches-the-check: the wait on the forge starts short and grows. A run whose
//! pull request's checks stay pending waits the run's first interval ([`Run::poll`], 2 s by
//! default), then half as long again each consecutive wait on the same awaited set, up to the
//! cap (15 s by default, the same multiple of the first interval for a test's shorter one);
//! a change of the awaited set or of the head the forge reports starts it again at the first
//! interval, and every wait is still cut to what is left of the hour budget. So a `ci_result`
//! follows the checks' completion within one current interval, and a CI that never reports
//! still ends at the hour budget with the budget stop.
//!
//! The runs go through a double of the forge; nothing here reaches a host.

#[path = "support/own_home.rs"]
mod own_home;

use std::cell::{Cell, RefCell};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;
use std::time::{Duration, Instant, SystemTime};

use anyhow::anyhow;
use takt::case::CASES_DIR;
use takt::evidence::Record;
use takt::forge::{Checks, Forge, Head, Memory, PullRequest, PullRequestView};
use takt::frontier::Aep;
use takt::harness::{Harness, StepContext, StepReport};
use takt::run::Backoff;
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
        .join(format!("ci-wait-backoff-{}-{name}", std::process::id()));
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
        "ci-wait-backoff",
    )
}

/// The story taken through `open_next`, its work committed on the case's branch, and the tests
/// and the gate recorded green at that revision: the case id. The case is ready for `pr.open`.
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

/// The double: its checks pending until `green_at` (never, when none), then passing; the
/// `n`-th read of the pull request (from 1) reports the head `heads[n - 1]` on the case's
/// branch where that is set, as a forge whose pull request lags the push, and the head
/// pushed otherwise.
struct Watched {
    memory: Rc<Memory>,
    green_at: Option<SystemTime>,
    heads: Vec<Option<String>>,
    views: Cell<usize>,
    reads: RefCell<Vec<(Instant, Checks)>>,
}

impl Watched {
    fn new(green_at: Option<SystemTime>, heads: Vec<Option<String>>) -> Self {
        Self {
            memory: Rc::new(Memory {
                checks: Checks::Pass,
                ..Memory::default()
            }),
            green_at,
            heads,
            views: Cell::new(0),
            reads: RefCell::new(Vec::new()),
        }
    }
}

impl Forge for Watched {
    fn target_tip(&self, tree: &Path, base: &str) -> anyhow::Result<String> {
        self.memory.target_tip(tree, base)
    }

    /// Scans nothing: the double publishes nothing off the machine.
    fn scan_outgoing(
        &self,
        _: &Path,
        _: &Path,
        _: &takt::forge::Outgoing<'_>,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    fn name(&self) -> &str {
        "watched"
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
        let mut view = self.memory.pull_request(tree, number)?;
        let n = self.views.get() + 1;
        self.views.set(n);
        if let Some(Some(sha)) = self.heads.get(n - 1) {
            view.head = Head {
                branch: view.head.branch,
                sha: sha.clone(),
            };
        }
        Ok(view)
    }

    fn checks(&self, tree: &Path, number: u64) -> anyhow::Result<Checks> {
        self.memory.checks(tree, number)?;
        let checks = match self.green_at {
            Some(at) if SystemTime::now() >= at => Checks::Pass,
            _ => Checks::Pending,
        };
        self.reads.borrow_mut().push((Instant::now(), checks));
        Ok(checks)
    }

    fn merge(&self, tree: &Path, dir: &Path, number: u64, head: &str) -> anyhow::Result<()> {
        self.memory.merge(tree, dir, number, head)
    }
}

/// Resumes the case `id` through `forge` with `harness`, its first interval `poll` unless none,
/// and runs it: the stop, or the error, and what it printed.
fn resumed_run(
    repo: &Path,
    id: &str,
    options: RunOptions,
    forge: Rc<dyn Forge>,
    harness: &dyn Harness,
    poll: Option<Duration>,
) -> (Result<Stop, String>, String) {
    let validators = validators(forge);
    let mut run = Run::resume(repo, options, id, harness, &validators).expect("the case resumes");
    if let Some(poll) = poll {
        run.poll = poll;
    }
    let mut out = Vec::new();
    let stop = run.run(&mut out).map_err(|error| format!("{error:#}"));
    (stop, String::from_utf8_lossy(&out).into_owned())
}

/// The `waiting` lines of the case's record: each wait's actions and its seconds, in order.
fn waits(repo: &Path, id: &str) -> Vec<(Vec<String>, f64)> {
    let path = repo
        .join(CASES_DIR)
        .join(id)
        .join(takt::events::EVENTS_FILE);
    fs::read_to_string(&path)
        .expect("the record")
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("json"))
        .filter(|line| line["event"] == "waiting")
        .map(|line| {
            let on = line["on"]
                .as_array()
                .expect("on")
                .iter()
                .map(|action| action.as_str().expect("an action").to_owned())
                .collect();
            (on, line["seconds"].as_f64().expect("seconds"))
        })
        .collect()
}

/// The evidence files of `kind` the case holds, each with its JSON.
fn evidence(repo: &Path, id: &str, kind: &str) -> Vec<(PathBuf, serde_json::Value)> {
    let dir = repo.join(CASES_DIR).join(id).join("evidence");
    let Ok(entries) = fs::read_dir(&dir) else {
        return Vec::new();
    };
    entries
        .map(|entry| {
            let path = entry.expect("an entry").path();
            let json =
                serde_json::from_str(&fs::read_to_string(&path).expect("a file")).expect("json");
            (path, json)
        })
        .filter(|(_, record): &(PathBuf, serde_json::Value)| record["kind"] == kind)
        .collect()
}

/// The waits a backoff from `first` gives, `n` of them: half as long again each, capped at
/// 7.5 times `first` (the default's 15 s over its 2 s).
fn grown(first: Duration, n: usize) -> Vec<f64> {
    let cap = first.as_secs_f64() * 7.5;
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

/// Checks that stay pending: each wait is half as long again as the one before, from the first
/// interval up to the cap and then at the cap, every one on `ci.read`.
#[test]
fn the_wait_grows_from_two_seconds_to_fifteen() {
    let repo = repository("grows");
    let id = worked_case(&repo);
    let forge = Rc::new(Watched::new(None, Vec::new()));
    let first = Duration::from_millis(4);

    let (stop, text) = resumed_run(
        &repo,
        &id,
        options(Some(10), None),
        forge,
        &Gatekeeper::default(),
        Some(first),
    );

    assert_eq!(
        stop,
        Ok(Stop::Waiting {
            on: vec!["ci.read".to_owned()],
            budget: Some(Box::new(Stop::Iterations(10))),
            local: Vec::new(),
        }),
        "{text}"
    );
    let waits = waits(&repo, &id);
    assert!(waits.len() >= 7, "seven waits at least: {waits:?}\n{text}");
    assert!(
        waits.iter().all(|(on, _)| on == &["ci.read".to_owned()]),
        "{waits:?}"
    );
    let seconds: Vec<f64> = waits.iter().map(|(_, seconds)| *seconds).collect();
    assert!(
        close(&seconds, &grown(first, seconds.len())),
        "the waits {seconds:?} are not {:?}, story:ci-wait-watches-the-check\n{text}",
        grown(first, seconds.len())
    );
}

/// The backoff starts again at the first interval when the head the forge reports changes: a
/// pull request at the head pushed with its checks pending, then lagging the push at one
/// commit, then at another, then at the head pushed again with its checks still pending,
/// waits from the first interval at each change.
#[test]
fn the_backoff_resets_when_the_head_changes() {
    let repo = repository("head-changes");
    let id = worked_case(&repo);
    let base = git(&repo, &["rev-parse", "main"]);
    let other = "0123456789abcdef0123456789abcdef01234567".to_owned();
    // `pr.open` opens the pull request and reads no view through the double, so every read
    // is `ci.read`'s: the first sees the head pushed and the checks pending, the next three the
    // base, the two after that another commit, and every later one the head pushed.
    let heads = vec![
        None,
        Some(base.clone()),
        Some(base.clone()),
        Some(base),
        Some(other.clone()),
        Some(other),
    ];
    let forge = Rc::new(Watched::new(None, heads));
    let first = Duration::from_millis(4);

    let (stop, text) = resumed_run(
        &repo,
        &id,
        options(Some(12), None),
        forge,
        &Gatekeeper::default(),
        Some(first),
    );

    assert!(matches!(stop, Ok(Stop::Waiting { .. })), "{stop:?}\n{text}");
    let seconds: Vec<f64> = waits(&repo, &id).iter().map(|(_, s)| *s).collect();
    assert!(seconds.len() >= 8, "{seconds:?}\n{text}");
    let mut expected = grown(first, 1);
    expected.extend(grown(first, 3));
    expected.extend(grown(first, 2));
    expected.extend(grown(first, seconds.len() - 6));
    assert!(
        close(&seconds, &expected),
        "the waits {seconds:?} are not {expected:?}: a new head starts the backoff again, story:ci-wait-watches-the-check\n{text}"
    );
}

/// Checks that turn green at a known instant, under the default backoff: the `ci_result`
/// record is written within one current interval of it, and under the story's 5 s.
#[test]
fn ci_result_follows_completion_within_one_interval() {
    let repo = repository("follows");
    let id = worked_case(&repo);
    let green_at = SystemTime::now() + Duration::from_millis(2500);
    let forge = Rc::new(Watched::new(Some(green_at), Vec::new()));
    let harness = Gatekeeper::default();

    let (stop, text) = resumed_run(
        &repo,
        &id,
        options(Some(20), None),
        forge.clone(),
        &harness,
        None,
    );

    assert_eq!(
        stop,
        Ok(Stop::ApprovalRequired(vec!["repository.merge".to_owned()])),
        "{text}"
    );
    assert_eq!(harness.offers.get(), 0, "{text}");
    let ci = evidence(&repo, &id, "ci_result");
    assert_eq!(ci.len(), 1, "{text}");
    assert_eq!(ci[0].1["result"], "pass");
    let written = fs::metadata(&ci[0].0)
        .and_then(|meta| meta.modified())
        .expect("a modification time");
    let after = written.duration_since(green_at).unwrap_or_default();
    let interval = waits(&repo, &id)
        .last()
        .map(|(_, seconds)| Duration::from_secs_f64(*seconds))
        .unwrap_or_default();
    assert!(
        after <= interval + Duration::from_secs(1),
        "ci_result written {after:?} after the checks passed, the current interval {interval:?}, story:ci-wait-watches-the-check\n{text}"
    );
    assert!(
        after < Duration::from_secs(5),
        "ci_result written {after:?} after the checks passed, not under 5 s, story:ci-wait-watches-the-check\n{text}"
    );
}

/// Checks that never report, under an hour budget: the run stops `Waiting` on `ci.read` with
/// `Hours` spent, its waits grown and the last cut to what was left. The budget runs from the
/// resume, so on a loaded host it can run out before the run reads the checks at all: that
/// attempt does not test the wait, and the next has twice the budget.
#[test]
fn a_ci_that_never_reports_ends_at_the_hour_budget() {
    let first = Duration::from_millis(100);
    let mut spent_before_the_wait = Vec::new();
    for (attempt, seconds) in [3.0_f64, 6.0, 12.0, 24.0].iter().enumerate() {
        let repo = repository(&format!("never-{attempt}"));
        let id = worked_case(&repo);
        let forge = Rc::new(Watched::new(None, Vec::new()));
        let hours = seconds / 3600.0;
        let started = Instant::now();
        let (stop, text) = resumed_run(
            &repo,
            &id,
            options(None, Some(hours)),
            forge.clone(),
            &NoHarness,
            Some(first),
        );
        let elapsed = started.elapsed();
        if stop == Ok(Stop::Hours(hours)) && forge.reads.borrow().is_empty() {
            spent_before_the_wait.push(*seconds);
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
        assert!(
            elapsed < Duration::from_secs_f64(seconds + 30.0),
            "waited {elapsed:?} on a {seconds} s budget: {text}"
        );
        let waits: Vec<f64> = waits(&repo, &id).iter().map(|(_, s)| *s).collect();
        assert!(waits.len() >= 2, "{waits:?}\n{text}");
        let cap = first.as_secs_f64() * 7.5;
        assert!(
            waits.iter().all(|wait| *wait <= cap + 1e-9),
            "a wait past the cap {cap} s: {waits:?}, story:ci-wait-watches-the-check"
        );
        let full = &waits[..waits.len() - 1];
        assert!(
            close(full, &grown(first, full.len())),
            "the waits before the last {full:?} did not grow, story:ci-wait-watches-the-check\n{text}"
        );
        return;
    }
    panic!(
        "every hour budget ({spent_before_the_wait:?} s) ran out before the run read the checks once"
    );
}

/// The default backoff, exactly: 2, 3, 4.5, 6.75, 10.125, then 15 s from there on; a run's
/// backoff is it unless its poll is set, and a set poll scales it, the cap 7.5 times the poll.
#[test]
fn the_default_backoff_is_two_seconds_growing_by_half_to_fifteen() {
    let backoff = Backoff::DEFAULT;
    let mut wait = backoff.start();
    let mut seen = Vec::new();
    for _ in 0..8 {
        seen.push(wait);
        wait = backoff.after(wait);
    }
    let ms = |ms: u64| Duration::from_millis(ms);
    let micros = |us: u64| Duration::from_micros(us);
    assert_eq!(
        seen,
        vec![
            ms(2000),
            ms(3000),
            ms(4500),
            ms(6750),
            micros(10_125_000),
            ms(15_000),
            ms(15_000),
            ms(15_000),
        ],
        "story:ci-wait-watches-the-check"
    );
    assert_eq!(Backoff::scaled(ms(2000)), Backoff::DEFAULT);
    assert_eq!(Backoff::scaled(ms(4)).cap, ms(30));
    assert_eq!(
        Backoff::scaled(Duration::MAX).after(Duration::MAX),
        Duration::MAX
    );

    let repo = repository("default");
    let id = worked_case(&repo);
    let forge: Rc<dyn Forge> = Rc::new(Memory::default());
    let validators = validators(forge);
    let run = Run::resume(&repo, options(Some(1), None), &id, &NoHarness, &validators)
        .expect("the case resumes");
    assert_eq!(run.poll, ms(2000));
    assert_eq!(run.backoff(), Backoff::DEFAULT);
}
