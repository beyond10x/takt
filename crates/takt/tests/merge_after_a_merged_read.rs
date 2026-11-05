//! story:merge-is-not-sent-after-a-merged-read: `repository.merge` reads the pull request twice,
//! its view and again right before the merge. When that second read finds the pull request
//! merged, closed or open at another head, no merge is sent: a merged or closed pull request is
//! observed as the first read would observe it (`merged`, `closed`), and one at another head
//! merges nothing and leaves the claim unknown, saying why.
//!
//! Every case runs the built-in `story.delivery/1` over the in-memory forge ([`Memory`]),
//! wrapped by [`Second`]: a forge that answers the first read of a run as the double holds it
//! and every later read as the case sets it. A first run stops at the gate, the operator grants
//! the merge, and the next run takes the merge step. Nothing leaves the machine.

#[path = "support/own_home.rs"]
mod own_home;

use std::cell::{Cell, RefCell};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;
use std::time::Duration;

use anyhow::anyhow;
use takt::case::grant;
use takt::evidence::Record;
use takt::forge::{
    Checks, Forge, IdentityScan, Memory, MergeMessage, Outgoing, PullRequest, PullRequestState,
    PullRequestView, Written,
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

/// A scratch git tree under this crate's build directory, removed when dropped.
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

/// A scratch tree with one commit on `main`.
fn scratch(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("merged-read-{}-{name}", std::process::id()));
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

/// A harness that requests the merge gate, changing nothing, as soon as it is offered.
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

/// What [`Second`] answers for a read after the first once it is armed: the state, and the
/// head's commit when it is not the one the double holds.
#[derive(Clone)]
struct Later {
    state: PullRequestState,
    sha: Option<String>,
}

/// [`Memory`], whose reads of a pull request answer as the double holds it until `first` more
/// reads are taken, and from then on as `later` says, once `later` is set. Every call is logged
/// in the double's `calls`.
struct Second {
    inner: Rc<Memory>,
    first: Cell<u32>,
    later: RefCell<Option<Later>>,
}

impl Second {
    fn new(inner: Rc<Memory>) -> Self {
        Self {
            inner,
            first: Cell::new(0),
            later: RefCell::new(None),
        }
    }

    /// From the next read on: `first` reads as the double holds it, then every read as `later`.
    fn arm(&self, first: u32, later: Later) {
        self.first.set(first);
        self.later.replace(Some(later));
    }
}

impl Forge for Second {
    fn name(&self) -> &str {
        "second-read-memory"
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
        self.inner.open_pull_request(tree, dir, request)
    }

    fn pull_request(&self, tree: &Path, number: u64) -> anyhow::Result<PullRequestView> {
        let mut view = self.inner.pull_request(tree, number)?;
        let Some(later) = self.later.borrow().clone() else {
            return Ok(view);
        };
        if self.first.get() > 0 {
            self.first.set(self.first.get() - 1);
            return Ok(view);
        }
        view.state = later.state;
        if let Some(sha) = later.sha {
            view.head.sha = sha;
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
        self.inner.merge(tree, dir, number, head)
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

/// The first run of case `id`, to the gate, as `fenced_forge_dispatch.rs` takes it, and the
/// operator's grant of the merge after it. The case directory.
fn granted(tree: &Path, id: &str, forge: &Rc<Second>) -> PathBuf {
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
        ("test_result", Some("pass"), "merged-read-test"),
        ("gate_result", Some("pass"), "merged-read-test"),
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
    grant(tree, id, "repository.merge", "human:operator").expect("the grant is written");
    run.case.dir.clone()
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

fn resumed(tree: &Path, id: &str, forge: &Rc<Second>) -> Ran {
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

fn steps(case_dir: &Path, action: &str) -> Vec<serde_json::Value> {
    fs::read_to_string(case_dir.join("events.jsonl"))
        .expect("the run record")
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("a line of json"))
        .filter(|line| {
            line["event"] == "step" && line["by"] == "validator" && line["action"] == action
        })
        .collect()
}

fn outcomes(case_dir: &Path) -> Vec<String> {
    steps(case_dir, "repository.merge")
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

/// What the merge step of a granted case did when the read right before the merge answered as
/// `later` says, the first read answering as the double holds the pull request: the merge calls,
/// the `merge_observation` results, the merge step lines' outcomes and what the run said.
fn second_read(name: &str, later: impl FnOnce(&Path) -> Later) -> (usize, Vec<String>, Ran) {
    let tree = scratch(name);
    let memory = Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    });
    let forge = Rc::new(Second::new(memory.clone()));
    let case_dir = granted(&tree, name, &forge);
    let asked = memory.calls.borrow().len();
    forge.arm(1, later(&tree));
    let ran = resumed(&tree, name, &forge);
    let calls = memory.calls.borrow()[asked..].to_vec();
    assert!(
        calls
            .iter()
            .filter(|call| call.starts_with("view #"))
            .count()
            >= 2,
        "the merge step read the pull request twice: {calls:#?}\n{}",
        ran.said()
    );
    let said = format!(
        "{}\nmerge outcomes: {:?}\ncalls: {calls:#?}",
        ran.said(),
        outcomes(&case_dir)
    );
    (
        merge_calls(&memory),
        merge_observations(&case_dir),
        Ran {
            stop: ran.stop,
            text: said,
        },
    )
}

/// The read right before the merge finds the pull request merged (by another hand, after the
/// view): no merge is sent, and the observation records `merged`, as a first read finding it
/// merged would.
#[test]
fn a_pull_request_read_merged_right_before_the_merge_is_not_merged_again() {
    let (calls, observed, ran) = second_read("merged", |_| Later {
        state: PullRequestState::Merged,
        sha: None,
    });
    assert_eq!(
        calls,
        0,
        "no merge is sent after the read before it finds the pull request merged\n{}",
        ran.said()
    );
    assert_eq!(observed, ["merged"], "{}", ran.said());
}

/// The read right before the merge finds the pull request closed unmerged: no merge is sent,
/// and the observation records `closed`, as a first read finding it closed would.
#[test]
fn a_pull_request_read_closed_right_before_the_merge_is_not_merged() {
    let (calls, observed, ran) = second_read("closed", |_| Later {
        state: PullRequestState::Closed,
        sha: None,
    });
    assert_eq!(
        calls,
        0,
        "no merge is sent after the read before it finds the pull request closed\n{}",
        ran.said()
    );
    assert_eq!(observed, ["closed"], "{}", ran.said());
}

/// The read right before the merge finds the pull request open at another head (another hand
/// pushed the branch after the view): no merge is sent, no `merge_observation` is written, and
/// the step says why.
#[test]
fn a_pull_request_read_at_another_head_right_before_the_merge_is_not_merged() {
    let (calls, observed, ran) = second_read("moved", |tree| Later {
        state: PullRequestState::Open,
        sha: Some(git(tree, &["rev-parse", "main"])),
    });
    assert_eq!(
        calls,
        0,
        "no merge is sent after the read before it finds the pull request at another head\n{}",
        ran.said()
    );
    assert!(observed.is_empty(), "{}", ran.said());
    assert!(
        ran.text.contains("right before the merge"),
        "the step says why nothing was merged\n{}",
        ran.said()
    );
}
