//! Adversary, wave 50, unit a (story:fenced-forge-dispatch): a merge whose answer was lost, or
//! cut off before its step's line, is sent at most once; `pr.open`'s lost answer and a pull
//! request closed after an unknown merge carry the class the story names.
//!
//! Every case runs the built-in `story.delivery/1` over the in-memory forge ([`Memory`]),
//! wrapped by [`Lagging`]: a forge that reports a merge it applied as still open for a set
//! number of reads, as the glab adapter reports GitLab's `locked` (a merge in progress) as
//! open (`crates/takt/src/forge/gitlab.rs`, `state_of`). Nothing leaves the machine.

#[path = "support/own_home.rs"]
mod own_home;

use std::cell::Cell;
use std::fs;
use std::panic::AssertUnwindSafe;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;
use std::time::Duration;

use anyhow::anyhow;
use takt::case::{LOCK_FILE, grant};
use takt::evidence::Record;
use takt::forge::{
    Checks, Forge, IdentityScan, Memory, MergeAnswer, MergeMessage, Outgoing, PullRequest,
    PullRequestState, PullRequestView, WriteError, Written,
};
use takt::harness::{Harness, StepContext, StepReport};
use takt::validator::{CiRead, PrMerged, PrOpen, Validator};
use takt::{Run, RunOptions, Stop};

const PROTOCOL: &str = "story.delivery/1";

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

/// A scratch git tree under the tree's `target/scratch`, removed when dropped.
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

fn scratch(name: &str) -> Scratch {
    let target = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .parent()
        .expect("the target directory")
        .join("scratch");
    let dir = target.join(format!("w50-adv-a-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(&dir).expect("scratch dir");
    fs::write(dir.join("README.md"), "a story\n").expect("a file");
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

struct Requester;

impl Harness for Requester {
    fn name(&self) -> &str {
        "requester"
    }

    fn step(&self, context: &StepContext<'_>) -> Result<StepReport, anyhow::Error> {
        if context.approvals.contains(&"repository.merge".to_owned()) {
            return Ok(StepReport {
                action: "repository.merge".to_owned(),
                summary: "the intent holds; merge it".to_owned(),
                tokens: None,
            });
        }
        Err(anyhow!(
            "the gate was not offered; a judgement step among {} is left",
            context.offered.join(", ")
        ))
    }
}

fn options(id: &str) -> RunOptions {
    RunOptions {
        protocol: PROTOCOL.to_owned(),
        intent: "deliver the story\n\nThe story's body.".to_owned(),
        inputs: Vec::new(),
        iterations: Some(8),
        hours: None,
        tokens: None,
        case_id: Some(id.to_owned()),
        gate: Some("repository.merge".to_owned()),
    }
}

/// [`Memory`], with three ways of answering it does not have: a merge it applied read as still
/// open for `lag` more reads (a merge in progress); every open pull request read closed once
/// `closed` is set; `pr.open`'s answer lost after it opened (`lose_open`), and a merge whose
/// request went out followed by the process going away (`crash_on_merge`). It finds the open
/// pull request of a branch as the forge would.
struct Lagging {
    inner: Rc<Memory>,
    lag: Cell<u32>,
    closed: Cell<bool>,
    lose_open: Cell<bool>,
    crash_on_merge: Cell<bool>,
}

impl Lagging {
    fn new(inner: Rc<Memory>) -> Self {
        Self {
            inner,
            lag: Cell::new(0),
            closed: Cell::new(false),
            lose_open: Cell::new(false),
            crash_on_merge: Cell::new(false),
        }
    }
}

impl Forge for Lagging {
    fn name(&self) -> &str {
        "lagging-memory"
    }

    fn scan_outgoing(
        &self,
        tree: &Path,
        dir: &Path,
        outgoing: &Outgoing<'_>,
    ) -> anyhow::Result<()> {
        self.inner.scan_outgoing(tree, dir, outgoing)
    }

    fn push(&self, tree: &Path, branch: &str) -> anyhow::Result<()> {
        self.inner.push(tree, branch)
    }

    fn push_revision(&self, tree: &Path, branch: &str, revision: &str) -> anyhow::Result<()> {
        self.inner.push_revision(tree, branch, revision)
    }

    fn open_pull_request(&self, tree: &Path, dir: &Path, request: &PullRequest) -> Written<u64> {
        let number = self.inner.open_pull_request(tree, dir, request)?;
        if self.lose_open.replace(false) {
            return Err(WriteError::unknown(anyhow!(
                "the open of pull request #{number} timed out; its answer was lost"
            )));
        }
        Ok(number)
    }

    fn find_open_pull_request(
        &self,
        _: &Path,
        branch: &str,
        base: &str,
    ) -> anyhow::Result<Option<u64>> {
        self.inner.calls.borrow_mut().push(format!("find {branch}"));
        Ok(self
            .inner
            .opened
            .borrow()
            .iter()
            .position(|request| request.branch == branch && request.base == base)
            .map(|index| index as u64 + 1))
    }

    fn pull_request(&self, tree: &Path, number: u64) -> anyhow::Result<PullRequestView> {
        let mut view = self.inner.pull_request(tree, number)?;
        if view.state == PullRequestState::Merged && self.lag.get() > 0 {
            self.lag.set(self.lag.get() - 1);
            view.state = PullRequestState::Open;
        }
        if view.state == PullRequestState::Open && self.closed.get() {
            view.state = PullRequestState::Closed;
        }
        Ok(view)
    }

    fn behind_base(&self, tree: &Path, number: u64) -> anyhow::Result<Option<bool>> {
        self.inner.behind_base(tree, number)
    }

    fn checks(&self, tree: &Path, number: u64) -> anyhow::Result<Checks> {
        self.inner.checks(tree, number)
    }

    fn merge(&self, tree: &Path, dir: &Path, number: u64, head: &str) -> Written<()> {
        let merged = self.inner.merge(tree, dir, number, head);
        if self.crash_on_merge.replace(false) {
            panic!("the process went away after the merge of #{number} was sent");
        }
        merged
    }

    fn merge_with(
        &self,
        tree: &Path,
        dir: &Path,
        number: u64,
        head: &str,
        message: &MergeMessage,
    ) -> Written<()> {
        let _ = message;
        self.merge(tree, dir, number, head)
    }

    fn scan_identity(
        &self,
        tree: &Path,
        dir: &Path,
        identity: &takt::git::CommitIdentity,
    ) -> anyhow::Result<IdentityScan> {
        self.inner.scan_identity(tree, dir, identity)
    }

    fn target_tip(&self, tree: &Path, base: &str) -> anyhow::Result<String> {
        self.inner.target_tip(tree, base)
    }
}

fn validators(forge: Rc<dyn Forge>) -> Vec<Box<dyn Validator>> {
    vec![
        Box::new(PrOpen::new(forge.clone())),
        Box::new(CiRead::new(forge.clone())),
        Box::new(PrMerged::new(forge)),
    ]
}

/// The first run of case `id`, to the gate, as `fenced_forge_dispatch.rs` takes it; the grant
/// written after when `grant_it`. The revision and the case directory.
fn to_the_gate(tree: &Path, id: &str, forge: &Rc<Lagging>, grant_it: bool) -> (String, PathBuf) {
    let dynamic: Rc<dyn Forge> = forge.clone();
    let validators = validators(dynamic);
    let mut run = Run::open(tree, options(id), &Requester, &validators).expect("the case opens");
    fs::write(tree.join("implemented.txt"), "done\n").expect("a file");
    git(tree, &["add", "implemented.txt"]);
    git(tree, &["commit", "-q", "-m", "implemented by the test"]);
    let revision = git(tree, &["rev-parse", "HEAD"]);
    run.case
        .set_revision("implementation", &revision)
        .expect("the revision moves");
    for (n, (kind, result, producer)) in [
        ("test_result", Some("pass"), "w50-adv-test"),
        ("gate_result", Some("pass"), "w50-adv-test"),
        ("handover", None, takt::run::CHOICE_PRODUCER),
        ("change_record", Some("ahead"), "takt.change-read"),
    ]
    .into_iter()
    .enumerate()
    {
        run.case
            .add_evidence(&Record::new(
                format!("e{n}"),
                kind,
                result.map(str::to_owned),
                "implementation",
                &revision,
                producer,
            ))
            .expect("the record is written");
    }
    let mut out = Vec::new();
    let stop = run.run(&mut out).expect("the run stops without an error");
    let text = String::from_utf8(out).expect("utf-8");
    assert_eq!(
        stop,
        Stop::ApprovalRequired(vec!["repository.merge".to_owned()]),
        "{text}"
    );
    if grant_it {
        grant(tree, id, "repository.merge", "human:operator").expect("the grant is written");
    }
    (revision, run.case.dir.clone())
}

struct Ran {
    stop: Result<Stop, String>,
    text: String,
}

impl Ran {
    fn said(&self) -> String {
        format!("{:?}\n{}", self.stop, self.text)
    }
}

fn resumed(tree: &Path, id: &str, forge: &Rc<Lagging>) -> Ran {
    let dynamic: Rc<dyn Forge> = forge.clone();
    let validators = validators(dynamic);
    let mut run =
        Run::resume(tree, options(id), id, &NoHarness, &validators).expect("the case resumes");
    run.poll = Duration::from_millis(5);
    let mut out = Vec::new();
    let stop = run.run(&mut out).map_err(|error| format!("{error:#}"));
    Ran {
        stop,
        text: String::from_utf8(out).expect("utf-8"),
    }
}

fn lines(case_dir: &Path) -> Vec<serde_json::Value> {
    fs::read_to_string(case_dir.join("events.jsonl"))
        .expect("the run record")
        .lines()
        .map(|line| serde_json::from_str(line).expect("a line of json"))
        .collect()
}

fn steps(case_dir: &Path, action: &str) -> Vec<serde_json::Value> {
    lines(case_dir)
        .into_iter()
        .filter(|line| {
            line["event"] == "step" && line["by"] == "validator" && line["action"] == action
        })
        .collect()
}

fn outcomes(case_dir: &Path, action: &str) -> Vec<String> {
    steps(case_dir, action)
        .iter()
        .map(|line| {
            line["forge_write"]["outcome"]
                .as_str()
                .unwrap_or("absent")
                .to_owned()
        })
        .collect()
}

fn merge_observations(case_dir: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(case_dir.join("evidence")) else {
        return Vec::new();
    };
    entries
        .filter_map(|entry| {
            let text = fs::read_to_string(entry.expect("an entry").path()).expect("a record");
            let record: serde_json::Value = serde_json::from_str(&text).expect("json");
            (record["kind"] == "merge_observation")
                .then(|| record["result"].as_str().unwrap_or_default().to_owned())
        })
        .collect()
}

fn merge_calls(memory: &Memory) -> usize {
    memory
        .calls
        .borrow()
        .iter()
        .filter(|call| call.starts_with("merge #"))
        .count()
}

/// Acceptance 1 with a merge that is still landing when the next run reads it: applied, its
/// answer lost, and read open at its head for `lag` reads before the forge reports it merged.
/// Answers the record's merge outcomes, the merge calls across both runs, and what both said.
fn lost_answer_read_late(name: &str, lag: u32) -> (Vec<String>, usize, Vec<String>, String) {
    let tree = scratch(name);
    let memory = Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    });
    let forge = Rc::new(Lagging::new(memory.clone()));
    let (_, case_dir) = to_the_gate(&tree, name, &forge, true);
    *memory.merge_answer.borrow_mut() = MergeAnswer::MergedAnswerLost;
    forge.lag.set(lag);
    let first = resumed(&tree, name, &forge);
    let next = resumed(&tree, name, &forge);
    (
        outcomes(&case_dir, "repository.merge"),
        merge_calls(&memory),
        merge_observations(&case_dir),
        format!(
            "first: {}\nnext: {}\ncalls: {:#?}",
            first.said(),
            next.said(),
            memory.calls.borrow()
        ),
    )
}

/// Control: the merge applied, its answer lost, and still landing for the next run's first
/// `repository.merge` observation, which reads the pull request twice (its view, and again
/// right before the merge): both open (GitLab's `locked`). That step sends nothing and waits;
/// the next reads it merged: one merge call across both runs.
#[test]
fn w50_adv_a_lost_merge_answer_landing_one_observation_late_is_sent_once() {
    let (outcomes, calls, observed, said) = lost_answer_read_late("late-one", 2);
    assert_eq!(calls, 1, "story:fenced-forge-dispatch acceptance 1: {said}");
    assert_eq!(observed, ["merged"], "{said}");
    assert_eq!(
        outcomes,
        ["unknown", "not_attempted", "not_attempted"],
        "{said}"
    );
}

/// The same merge still landing one observation later than above: both reads of the second
/// observation report it open at its head too. The fence the unit added waits one observation
/// (one poll, 2 s by default) and then sends the merge again, so the forge is asked for the
/// merge it already took: two merge calls where acceptance 1 says one, and the story says a
/// write that may have been sent is never sent again.
#[test]
fn w50_adv_a_lost_merge_answer_landing_two_observations_late_is_sent_once() {
    let (outcomes, calls, observed, said) = lost_answer_read_late("late-two", 4);
    assert_eq!(
        calls, 1,
        "story:fenced-forge-dispatch: a merge recorded unknown and read open at its head is \
         never sent again, however many polls its landing takes; outcomes {outcomes:?}\n{said}"
    );
    assert_eq!(observed, ["merged"], "{said}");
}

/// A crash between the write and the event line: the merge's request went out and the
/// process went away before the step's line was written (an operator's interrupt, an OOM kill
/// or an eviction during the merge call). The merge is landing, read open by both reads of the
/// next run's first observation, exactly the forge state of the control above. With no line
/// saying a merge may have been sent, the next run sends it at once: two merge calls, where the
/// same forge state after a timeout gives one.
#[test]
fn w50_adv_a_merge_cut_off_before_its_line_is_sent_once() {
    let name = "crash";
    let tree = scratch(name);
    let memory = Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    });
    let forge = Rc::new(Lagging::new(memory.clone()));
    let (_, case_dir) = to_the_gate(&tree, name, &forge, true);
    forge.crash_on_merge.set(true);
    forge.lag.set(2);
    let crashed = std::panic::catch_unwind(AssertUnwindSafe(|| resumed(&tree, name, &forge)));
    assert!(crashed.is_err(), "the first run went away during the merge");
    assert_eq!(merge_calls(&memory), 1, "{:#?}", memory.calls.borrow());
    assert_eq!(memory.merged.borrow().as_slice(), [1], "the forge took it");
    // What the refusal of a held case tells the operator to do after a crash.
    let _ = fs::remove_file(case_dir.join(LOCK_FILE));
    let before = outcomes(&case_dir, "repository.merge");

    let next = resumed(&tree, name, &forge);
    assert_eq!(
        merge_calls(&memory),
        1,
        "story:fenced-forge-dispatch: a merge whose request may have been sent is never sent \
         again; the record after the crash held {before:?}, after the next run {:?}\n{}\n{:#?}",
        outcomes(&case_dir, "repository.merge"),
        next.said(),
        memory.calls.borrow()
    );
    assert_eq!(merge_observations(&case_dir), ["merged"], "{}", next.said());
}

/// `pr.open`'s answer lost after it opened: the step finds the pull request open and takes it,
/// and its line records the open's own class, `unknown`, with the number found; no second open.
#[test]
fn w50_adv_a_pr_open_answer_lost_records_unknown_with_the_number_found() {
    let name = "open-lost";
    let tree = scratch(name);
    let memory = Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    });
    let forge = Rc::new(Lagging::new(memory.clone()));
    forge.lose_open.set(true);
    let (revision, case_dir) = to_the_gate(&tree, name, &forge, false);
    let opens = steps(&case_dir, "pr.open");
    assert_eq!(opens.len(), 1, "{opens:#?}");
    let write = &opens[0]["forge_write"];
    assert_eq!(write["outcome"], "unknown", "{opens:#?}");
    assert_eq!(write["pull_request"], 1, "{opens:#?}");
    assert_eq!(write["head"], revision.as_str(), "{opens:#?}");
    assert_eq!(memory.opened.borrow().len(), 1, "one open sent");
}

/// A merge recorded `unknown`, not applied, and the pull request closed by another hand before
/// the next run: that run reads it closed, records `closed`, and sends no merge.
#[test]
fn w50_adv_a_unknown_merge_then_closed_sends_nothing_and_records_closed() {
    let name = "closed";
    let tree = scratch(name);
    let memory = Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    });
    let forge = Rc::new(Lagging::new(memory.clone()));
    let (_, case_dir) = to_the_gate(&tree, name, &forge, true);
    *memory.merge_answer.borrow_mut() = MergeAnswer::AnswerLost;
    let first = resumed(&tree, name, &forge);
    assert_eq!(
        outcomes(&case_dir, "repository.merge"),
        ["unknown"],
        "{}",
        first.said()
    );
    forge.closed.set(true);
    let next = resumed(&tree, name, &forge);
    assert_eq!(merge_calls(&memory), 1, "{:#?}", memory.calls.borrow());
    assert_eq!(merge_observations(&case_dir), ["closed"], "{}", next.said());
    assert_eq!(
        outcomes(&case_dir, "repository.merge"),
        ["unknown", "not_attempted"],
        "{}",
        next.said()
    );
}
