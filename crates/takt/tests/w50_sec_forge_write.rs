//! Security review of story:fenced-forge-dispatch: a forge write whose answer was lost
//! (`unknown`) is resolved by reading the forge, never by sending the write again (the story's
//! Outcome: "the next step resolves it by reading the forge, never by sending the write again";
//! its summary: "a write that may have been sent is never sent again").
//!
//! The forge here is the in-memory double behind a wrapper that reports the pull request open
//! for a few reads after the double merged it: what GitLab does while a merge is in progress
//! (the `locked` state, which `crates/takt/src/forge/gitlab.rs` reads as open). The merge whose
//! answer was lost is the one that lands; a second merge is a second send of an `unknown`
//! write. Nothing leaves the machine.

#[path = "support/own_home.rs"]
mod own_home;

use std::cell::Cell;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;
use std::time::Duration;

use anyhow::anyhow;
use takt::case::grant;
use takt::evidence::Record;
use takt::forge::{
    Checks, Forge, Memory, MergeAnswer, Outgoing, PullRequest, PullRequestState, PullRequestView,
    Written,
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

fn scratch(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("w50-sec-{}-{name}", std::process::id()));
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

fn validators(forge: Rc<dyn Forge>) -> Vec<Box<dyn Validator>> {
    vec![
        Box::new(PrOpen::new(forge.clone())),
        Box::new(CiRead::new(forge.clone())),
        Box::new(PrMerged::new(forge)),
    ]
}

/// The double behind a merge in progress: once the double has merged a pull request, the next
/// `open_reads` reads of it report it open at the same head, as GitLab reports a merge request
/// `locked` while it merges. Every merge asked for is counted.
struct MergeInProgress {
    memory: Rc<Memory>,
    open_reads: Cell<usize>,
    merges: Cell<usize>,
}

impl Forge for MergeInProgress {
    fn name(&self) -> &str {
        "merge-in-progress"
    }

    fn scan_outgoing(
        &self,
        tree: &Path,
        dir: &Path,
        outgoing: &Outgoing<'_>,
    ) -> anyhow::Result<()> {
        self.memory.scan_outgoing(tree, dir, outgoing)
    }

    fn push(&self, tree: &Path, branch: &str) -> anyhow::Result<()> {
        self.memory.push(tree, branch)
    }

    fn push_revision(&self, tree: &Path, branch: &str, revision: &str) -> anyhow::Result<()> {
        self.memory.push_revision(tree, branch, revision)
    }

    fn open_pull_request(&self, tree: &Path, dir: &Path, request: &PullRequest) -> Written<u64> {
        self.memory.open_pull_request(tree, dir, request)
    }

    fn pull_request(&self, tree: &Path, number: u64) -> anyhow::Result<PullRequestView> {
        let view = self.memory.pull_request(tree, number)?;
        if view.state == PullRequestState::Merged && self.open_reads.get() > 0 {
            self.open_reads.set(self.open_reads.get() - 1);
            return Ok(PullRequestView {
                state: PullRequestState::Open,
                ..view
            });
        }
        Ok(view)
    }

    fn checks(&self, tree: &Path, number: u64) -> anyhow::Result<Checks> {
        self.memory.checks(tree, number)
    }

    fn merge(&self, tree: &Path, dir: &Path, number: u64, head: &str) -> Written<()> {
        self.merges.set(self.merges.get() + 1);
        self.memory.merge(tree, dir, number, head)
    }

    fn target_tip(&self, tree: &Path, base: &str) -> anyhow::Result<String> {
        self.memory.target_tip(tree, base)
    }
}

/// The case at the gate through `forge`, granted `repository.merge` by the operator: its
/// directory.
fn granted(tree: &Path, id: &str, forge: Rc<dyn Forge>) -> PathBuf {
    let validators = validators(forge);
    let mut run = Run::open(tree, options(id), &Requester, &validators).expect("the case opens");
    fs::write(tree.join("implemented.txt"), "done\n").expect("a file");
    git(tree, &["add", "implemented.txt"]);
    git(tree, &["commit", "-q", "-m", "implemented by the test"]);
    let revision = git(tree, &["rev-parse", "HEAD"]);
    run.case
        .set_revision("implementation", &revision)
        .expect("the revision moves");
    for (n, (kind, result, producer)) in [
        ("test_result", Some("pass"), "w50-sec-test"),
        ("gate_result", Some("pass"), "w50-sec-test"),
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
    assert_eq!(
        stop,
        Stop::ApprovalRequired(vec!["repository.merge".to_owned()]),
        "{}",
        String::from_utf8_lossy(&out)
    );
    grant(tree, id, "repository.merge", "human:operator").expect("the grant is written");
    run.case.dir.clone()
}

fn resumed(tree: &Path, id: &str, forge: Rc<dyn Forge>) -> String {
    let validators = validators(forge);
    let mut run =
        Run::resume(tree, options(id), id, &NoHarness, &validators).expect("the case resumes");
    run.poll = Duration::from_millis(5);
    let mut out = Vec::new();
    let stop = run.run(&mut out).map_err(|error| format!("{error:#}"));
    format!("{stop:?}\n{}", String::from_utf8_lossy(&out))
}

fn merge_outcomes(case_dir: &Path) -> Vec<String> {
    fs::read_to_string(case_dir.join("events.jsonl"))
        .expect("the run record")
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("json"))
        .filter(|line| {
            line["event"] == "step"
                && line["by"] == "validator"
                && line["action"] == "repository.merge"
        })
        .map(|line| {
            line["forge_write"]["outcome"]
                .as_str()
                .unwrap_or("absent")
                .to_owned()
        })
        .collect()
}

/// The story's Outcome, against a merge still in progress after its answer was lost: the merge
/// is sent once, recorded `unknown`, and every later step of the case resolves it by reading
/// the forge. Today the step after the one that waited sends the merge a second time, because
/// `unknown_merge_of` (`crates/takt/src/validator.rs`) reads only the last `repository.merge`
/// line that carries a forge write, and the waiting step wrote `not_attempted` over it.
#[test]
fn an_unknown_merge_still_in_progress_is_never_sent_again() {
    let tree = scratch("in-progress");
    let memory = Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    });
    let forge = Rc::new(MergeInProgress {
        memory: memory.clone(),
        open_reads: Cell::new(4),
        merges: Cell::new(0),
    });
    let case_dir = granted(&tree, "in-progress", forge.clone());
    *memory.merge_answer.borrow_mut() = MergeAnswer::MergedAnswerLost;

    let first = resumed(&tree, "in-progress", forge.clone());
    assert_eq!(merge_outcomes(&case_dir), ["unknown"], "{first}");
    assert_eq!(forge.merges.get(), 1, "{first}");

    let next = resumed(&tree, "in-progress", forge.clone());
    assert_eq!(
        forge.merges.get(),
        1,
        "story:fenced-forge-dispatch: a merge recorded `unknown` is resolved by reading the forge and never sent again; steps {:?}\n{next}",
        merge_outcomes(&case_dir)
    );
}
