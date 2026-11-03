//! story:fenced-forge-dispatch: `pr.open` and `repository.merge` record the outcome class of the
//! forge write they attempt on the step's line in `events.jsonl` (`forge_write`): `applied`,
//! `refused`, `not_attempted` or `unknown`. A write whose request may have reached the forge and
//! whose answer never came is `unknown`, and the next step resolves it by reading the forge,
//! never by sending the merge again on that ground.
//!
//! Every case but the last runs the built-in `story.delivery/1` with the in-memory forge
//! ([`Memory`]), its checks passing: a first run that stops at the gate, the operator's grant,
//! and the runs that follow; each reads the step's line of the record. The rider case takes its
//! story from a fixture store, so takt's riders for it are written above the head a merge
//! named. The last holds the
//! command forge to each way a merge fails, through a scripted runner and the process runner on
//! a local program. Nothing leaves the machine.

#[path = "support/own_home.rs"]
mod own_home;

use std::cell::RefCell;
use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;
use std::time::Duration;

use anyhow::anyhow;
use takt::case::{AUTHORITY_FILE, grant};
use takt::evidence::Record;
use takt::forge::command::{CommandForge, ForgeCommands};
use takt::forge::{
    Checks, Exit, Forge, Limits, Memory, MergeAnswer, Process, Runner, TimedOut, WriteOutcome,
};
use takt::frontier::{Aep, approve};
use takt::harness::{Harness, StepContext, StepReport};
use takt::scan::Scan;
use takt::validator::{ChangeRead, CiRead, Observation, PrMerged, PrOpen, TestsRun, Validator};
use takt::{Run, RunOptions, Stop};

/// The protocol under test, the engineering protocols' built-in.
const PROTOCOL: &str = "story.delivery/1";

/// What the forge says when it refuses a merge, as GitHub's API words a pull request it will not
/// merge.
const NOT_MERGEABLE: &str = "405 Method Not Allowed: Pull Request is not mergeable";

/// Runs `git <args>` in `tree`; its trimmed standard output.
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

/// A scratch tree with one commit on `main`.
fn scratch(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("fenced-{}-{name}", std::process::id()));
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

/// A harness the runs after the gate never need: every step they take is a validator's.
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

/// The three forge validators over `forge`, in the order the binary registers them.
fn validators(forge: Rc<dyn Forge>) -> Vec<Box<dyn Validator>> {
    vec![
        Box::new(PrOpen::new(forge.clone())),
        Box::new(CiRead::new(forge.clone())),
        Box::new(PrMerged::new(forge)),
    ]
}

/// The first run of the case `id` over `tree`: one commit beyond the base, the tests and the
/// gate green at it and the change handed over, the double's checks passing, the `Requester`
/// harness. It stops at the gate with the pull request open; the revision and the case
/// directory.
fn to_the_gate(tree: &Path, id: &str, memory: &Rc<Memory>) -> (String, PathBuf) {
    let forge: Rc<dyn Forge> = memory.clone();
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
        ("test_result", Some("pass"), "fenced-test"),
        ("gate_result", Some("pass"), "fenced-test"),
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
    (revision, run.case.dir.clone())
}

/// What one resumed run left: how it stopped (an error as its text) and what it printed.
struct Ran {
    stop: Result<Stop, String>,
    text: String,
}

impl Ran {
    fn said(&self) -> String {
        format!("{:?}\n{}", self.stop, self.text)
    }
}

/// The case `id` resumed over `memory`, as the next `takt run` on it would be, waiting on the
/// forge for `poll` between observations.
fn resumed(tree: &Path, id: &str, memory: &Rc<Memory>, poll: Duration) -> Ran {
    resumed_through(tree, id, memory.clone(), poll)
}

/// [`resumed`] through `forge`.
fn resumed_through(tree: &Path, id: &str, forge: Rc<dyn Forge>, poll: Duration) -> Ran {
    let validators = validators(forge);
    let mut run =
        Run::resume(tree, options(id), id, &NoHarness, &validators).expect("the case resumes");
    run.poll = poll;
    let mut out = Vec::new();
    let stop = run.run(&mut out).map_err(|error| format!("{error:#}"));
    Ran {
        stop,
        text: String::from_utf8(out).expect("utf-8"),
    }
}

/// The case at the gate, granted `repository.merge` by the operator: its revision and directory.
fn granted(tree: &Path, id: &str, memory: &Rc<Memory>) -> (String, PathBuf) {
    let (revision, case_dir) = to_the_gate(tree, id, memory);
    grant(tree, id, "repository.merge", "human:operator").expect("the grant is written");
    (revision, case_dir)
}

/// The record's lines, in order.
fn lines(case_dir: &Path) -> Vec<serde_json::Value> {
    fs::read_to_string(case_dir.join("events.jsonl"))
        .expect("the run record")
        .lines()
        .map(|line| serde_json::from_str(line).expect("a line of json"))
        .collect()
}

/// The validator step lines of `action` the record holds, in order.
fn steps(case_dir: &Path, action: &str) -> Vec<serde_json::Value> {
    lines(case_dir)
        .into_iter()
        .filter(|line| {
            line["event"] == "step" && line["by"] == "validator" && line["action"] == action
        })
        .collect()
}

/// The `merge_observation` records under the case's `evidence/`, their results.
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

/// How many merges the double was asked for.
fn merge_calls(memory: &Memory) -> usize {
    memory
        .calls
        .borrow()
        .iter()
        .filter(|call| call.starts_with("merge #"))
        .count()
}

/// How many pushes the double was asked for.
fn pushes(memory: &Memory) -> usize {
    memory
        .calls
        .borrow()
        .iter()
        .filter(|call| call.starts_with("push"))
        .count()
}

/// Acceptance 1: the forge applies the merge and its answer is lost to a timeout. The run records
/// the `repository.merge` step with outcome `unknown`, about pull request #1 at the head the
/// merge named, and no `merge_observation`; the next run on the case reads the pull request
/// merged and writes a `merge_observation` of `merged`, sending no merge: the double's call log
/// holds one merge across both runs.
#[test]
fn a_merge_applied_whose_answer_is_lost_is_unknown_and_the_next_run_reads_it_merged() {
    let tree = scratch("lost");
    let memory = Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    });
    let (revision, case_dir) = granted(&tree, "lost", &memory);
    *memory.merge_answer.borrow_mut() = MergeAnswer::MergedAnswerLost;

    let first = resumed(&tree, "lost", &memory, Duration::from_millis(5));
    let merges = steps(&case_dir, "repository.merge");
    assert_eq!(merges.len(), 1, "{}\n{merges:#?}", first.said());
    let write = &merges[0]["forge_write"];
    assert_eq!(write["outcome"], "unknown", "{}\n{merges:#?}", first.said());
    assert_eq!(write["pull_request"], 1, "{merges:#?}");
    assert_eq!(write["head"], revision.as_str(), "{merges:#?}");
    assert!(merge_observations(&case_dir).is_empty(), "{}", first.said());
    assert_eq!(
        memory.merged.borrow().as_slice(),
        [1],
        "the forge applied it"
    );
    assert_eq!(merge_calls(&memory), 1, "{:#?}", memory.calls.borrow());

    let asked = memory.calls.borrow().len();
    let next = resumed(&tree, "lost", &memory, Duration::from_millis(5));
    assert_eq!(
        next.stop,
        Ok(Stop::Completed("accepted".to_owned())),
        "{}",
        next.said()
    );
    assert_eq!(merge_observations(&case_dir), ["merged"], "{}", next.said());
    assert_eq!(
        merge_calls(&memory),
        1,
        "one merge call across both runs: {:#?}",
        memory.calls.borrow()
    );
    assert!(
        memory.calls.borrow()[asked..]
            .iter()
            .any(|call| call == "view #1"),
        "the next run read the pull request: {:#?}",
        memory.calls.borrow()
    );
    let merges = steps(&case_dir, "repository.merge");
    assert_eq!(merges.len(), 2, "{}\n{merges:#?}", next.said());
    assert_eq!(
        merges[1]["forge_write"]["outcome"], "not_attempted",
        "the read sent no merge: {merges:#?}"
    );
}

/// Acceptance 2: the forge refuses the merge with its 405. The step's line records outcome
/// `refused`, nothing is merged and no `merge_observation` is written.
#[test]
fn a_merge_the_forge_refuses_is_refused_and_writes_no_merge_observation() {
    let tree = scratch("refused");
    let memory = Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    });
    let (revision, case_dir) = granted(&tree, "refused", &memory);
    *memory.merge_answer.borrow_mut() = MergeAnswer::Refused(NOT_MERGEABLE.to_owned());

    let ran = resumed(&tree, "refused", &memory, Duration::from_millis(5));
    assert!(ran.said().contains(NOT_MERGEABLE), "{}", ran.said());
    let merges = steps(&case_dir, "repository.merge");
    assert_eq!(merges.len(), 1, "{}\n{merges:#?}", ran.said());
    let write = &merges[0]["forge_write"];
    assert_eq!(write["outcome"], "refused", "{}\n{merges:#?}", ran.said());
    assert_eq!(write["pull_request"], 1, "{merges:#?}");
    assert_eq!(write["head"], revision.as_str(), "{merges:#?}");
    assert!(merge_observations(&case_dir).is_empty(), "{}", ran.said());
    assert!(memory.merged.borrow().is_empty());
    assert_eq!(merge_calls(&memory), 1, "{:#?}", memory.calls.borrow());
}

/// Acceptance 3: another hand pushed the case's branch after the grant, so the forge reports the
/// pull request at a head that is not the case's revision. No merge is sent, and the step's line
/// records outcome `not_attempted` with the head the forge reported.
#[test]
fn a_merge_not_sent_because_the_head_moved_is_not_attempted() {
    let tree = scratch("moved");
    let memory = Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    });
    let (revision, case_dir) = granted(&tree, "moved", &memory);
    // Someone else's commit on the case's branch, pushed past takt: the double's head for the
    // branch is now that commit, which does not carry takt's revision alone.
    git(&tree, &["checkout", "-q", "-b", "elsewhere"]);
    fs::write(tree.join("elsewhere.txt"), "another hand\n").expect("a file");
    git(&tree, &["add", "elsewhere.txt"]);
    git(&tree, &["commit", "-q", "-m", "another hand's commit"]);
    let moved = git(&tree, &["rev-parse", "HEAD"]);
    git(&tree, &["checkout", "-q", "takt/moved"]);
    memory
        .pushed
        .borrow_mut()
        .insert("takt/moved".to_owned(), moved.clone());

    let ran = resumed(&tree, "moved", &memory, Duration::from_millis(5));
    let merges = steps(&case_dir, "repository.merge");
    assert!(!merges.is_empty(), "{}", ran.said());
    for merge in &merges {
        let write = &merge["forge_write"];
        assert_eq!(
            write["outcome"],
            "not_attempted",
            "{}\n{merges:#?}",
            ran.said()
        );
        assert_eq!(write["pull_request"], 1, "{merges:#?}");
        assert_eq!(write["head"], moved.as_str(), "{merges:#?}");
    }
    assert_ne!(moved, revision);
    assert_eq!(merge_calls(&memory), 0, "{:#?}", memory.calls.borrow());
    assert!(merge_observations(&case_dir).is_empty(), "{}", ran.said());
}

/// Acceptance 4: `pr.open` opens the pull request and the forge answers its number. The step's
/// line records outcome `applied`, with pull request #1 and the head pushed.
#[test]
fn a_pr_open_that_is_applied_records_its_number() {
    let tree = scratch("open");
    let memory = Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    });
    let (revision, case_dir) = to_the_gate(&tree, "open", &memory);
    let opens = steps(&case_dir, "pr.open");
    assert_eq!(opens.len(), 1, "{opens:#?}");
    let write = &opens[0]["forge_write"];
    assert_eq!(write["outcome"], "applied", "{opens:#?}");
    assert_eq!(write["pull_request"], 1, "{opens:#?}");
    assert_eq!(write["head"], revision.as_str(), "{opens:#?}");
    // A step that wrote nothing to the forge carries no forge write.
    for line in steps(&case_dir, "ci.read") {
        assert!(line.get("forge_write").is_none(), "{line:#}");
    }
}

/// The security boundary: a merge whose answer was lost without the forge applying it leaves the
/// pull request open at the head it named. No merge is sent for it again: every later step that
/// reads it so sends nothing, records `not_attempted` saying why and waits on the forge, and at
/// the budget the run stops waiting, naming the pull request and that its merge was sent with
/// its outcome unknown. Once a person merges it on the forge, the next run reads it merged and
/// records `merged`: one merge call across the three runs.
#[test]
fn an_unknown_merge_read_open_at_its_head_is_never_sent_again_and_says_why() {
    let tree = scratch("unapplied");
    let memory = Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    });
    let (revision, case_dir) = granted(&tree, "unapplied", &memory);
    *memory.merge_answer.borrow_mut() = MergeAnswer::AnswerLost;

    let first = resumed(&tree, "unapplied", &memory, Duration::from_millis(5));
    assert_eq!(
        steps(&case_dir, "repository.merge")[0]["forge_write"]["outcome"],
        "unknown",
        "{}",
        first.said()
    );
    assert!(memory.merged.borrow().is_empty());
    assert_eq!(merge_calls(&memory), 1);

    let next = resumed(&tree, "unapplied", &memory, Duration::from_millis(5));
    assert!(
        matches!(&next.stop, Ok(Stop::Waiting { on, .. }) if on == &["repository.merge"]),
        "the run stops waiting at its budget: {}",
        next.said()
    );
    let merges = steps(&case_dir, "repository.merge");
    let outcomes: Vec<&str> = merges
        .iter()
        .map(|line| line["forge_write"]["outcome"].as_str().unwrap_or("absent"))
        .collect();
    assert!(
        outcomes.len() > 2
            && outcomes[0] == "unknown"
            && outcomes[1..]
                .iter()
                .all(|outcome| *outcome == "not_attempted"),
        "story:fenced-forge-dispatch: every step after the unknown merge sends nothing: {outcomes:?}\n{}",
        next.said()
    );
    for merge in &merges[1..] {
        let why = merge["summary"].as_str().expect("a summary");
        assert!(
            why.contains("pull request #1")
                && why.contains("was sent and its outcome is not known")
                && why.contains("nothing sent"),
            "the step says why it sent nothing: {why}"
        );
        assert_eq!(merge["forge_write"]["head"], revision.as_str());
    }
    assert!(
        next.text.contains("pull request #1 at its head")
            && next.text.contains("was sent and its outcome is not known"),
        "what the run printed names the pull request and its unknown merge: {}",
        next.said()
    );
    assert_eq!(
        merge_calls(&memory),
        1,
        "story:fenced-forge-dispatch: a merge recorded unknown and read open at its head is never sent again: {:#?}",
        memory.calls.borrow()
    );
    assert!(merge_observations(&case_dir).is_empty(), "{}", next.said());

    // A person merges it on the forge; the next run reads it merged and sends nothing.
    memory.merged.borrow_mut().push(1);
    let last = resumed(&tree, "unapplied", &memory, Duration::from_millis(5));
    assert_eq!(
        last.stop,
        Ok(Stop::Completed("accepted".to_owned())),
        "{}",
        last.said()
    );
    assert_eq!(merge_observations(&case_dir), ["merged"], "{}", last.said());
    assert_eq!(
        merge_calls(&memory),
        1,
        "story:fenced-forge-dispatch: one merge call across the three runs: {:#?}",
        memory.calls.borrow()
    );
}

/// [`Memory`], every pull request read behind its base, as GitHub's `mergeable_state` `behind`
/// reads one whose base has commits its head does not hold.
struct Behind(Rc<Memory>);

impl Forge for Behind {
    fn name(&self) -> &str {
        "behind"
    }

    fn scan_outgoing(
        &self,
        tree: &Path,
        dir: &Path,
        outgoing: &takt::forge::Outgoing<'_>,
    ) -> anyhow::Result<()> {
        self.0.scan_outgoing(tree, dir, outgoing)
    }

    fn push(&self, tree: &Path, branch: &str) -> anyhow::Result<()> {
        self.0.push(tree, branch)
    }

    fn push_revision(&self, tree: &Path, branch: &str, revision: &str) -> anyhow::Result<()> {
        self.0.push_revision(tree, branch, revision)
    }

    fn open_pull_request(
        &self,
        tree: &Path,
        dir: &Path,
        request: &takt::forge::PullRequest,
    ) -> takt::forge::Written<u64> {
        self.0.open_pull_request(tree, dir, request)
    }

    fn pull_request(
        &self,
        tree: &Path,
        number: u64,
    ) -> anyhow::Result<takt::forge::PullRequestView> {
        self.0.pull_request(tree, number)
    }

    fn behind_base(&self, _: &Path, _: u64) -> anyhow::Result<Option<bool>> {
        Ok(Some(true))
    }

    fn checks(&self, tree: &Path, number: u64) -> anyhow::Result<Checks> {
        self.0.checks(tree, number)
    }

    fn merge(&self, tree: &Path, dir: &Path, number: u64, head: &str) -> takt::forge::Written<()> {
        self.0.merge(tree, dir, number, head)
    }

    fn target_tip(&self, tree: &Path, base: &str) -> anyhow::Result<String> {
        self.0.target_tip(tree, base)
    }
}

/// The fence holds against a base update: a merge whose answer was lost without the forge
/// applying it, then the base moves on and the forge reads the pull request behind it, open at
/// the head the merge named. While that merge may still land, the next run makes no update of
/// the base, pushes nothing and sends no merge: each step says why and waits, and the run stops
/// waiting at its budget. One merge call across the runs, the head the one first pushed.
#[test]
fn an_unknown_merge_is_not_sent_again_over_a_base_update() {
    let tree = scratch("behind");
    let memory = Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    });
    let (revision, case_dir) = granted(&tree, "behind", &memory);
    *memory.merge_answer.borrow_mut() = MergeAnswer::AnswerLost;
    let first = resumed(&tree, "behind", &memory, Duration::from_millis(5));
    assert_eq!(
        steps(&case_dir, "repository.merge")[0]["forge_write"]["outcome"],
        "unknown",
        "{}",
        first.said()
    );
    assert_eq!(merge_calls(&memory), 1, "{:#?}", memory.calls.borrow());

    // The base moves on by a commit the pull request's head does not hold.
    git(&tree, &["checkout", "-q", "main"]);
    fs::write(tree.join("upstream.txt"), "the base moved\n").expect("a file");
    git(&tree, &["add", "upstream.txt"]);
    git(&tree, &["commit", "-q", "-m", "the base moves on"]);
    git(&tree, &["checkout", "-q", "takt/behind"]);
    let pushed_before = pushes(&memory);

    let next = resumed_through(
        &tree,
        "behind",
        Rc::new(Behind(memory.clone())),
        Duration::from_millis(5),
    );
    let outcomes: Vec<String> = steps(&case_dir, "repository.merge")
        .iter()
        .map(|line| {
            line["forge_write"]["outcome"]
                .as_str()
                .unwrap_or("absent")
                .to_owned()
        })
        .collect();
    assert_eq!(
        merge_calls(&memory),
        1,
        "story:fenced-forge-dispatch: a merge recorded unknown and read open at its head is never \
         sent again, and no base update moves the head from under it; outcomes {outcomes:?}\n{}\n{:#?}",
        next.said(),
        memory.calls.borrow()
    );
    assert_eq!(
        pushes(&memory),
        pushed_before,
        "story:fenced-forge-dispatch: nothing is pushed while the merge may land: {:#?}",
        memory.calls.borrow()
    );
    assert_eq!(
        memory.pushed.borrow().get("takt/behind"),
        Some(&revision),
        "the pull request's head is the one the merge named"
    );
    assert!(
        matches!(&next.stop, Ok(Stop::Waiting { on, .. }) if on == &["repository.merge"]),
        "the run stops waiting at its budget: {}",
        next.said()
    );
    assert!(
        next.text.contains("was sent and its outcome is not known"),
        "{}",
        next.said()
    );
    assert!(merge_observations(&case_dir).is_empty(), "{}", next.said());
}

/// The fixture stores, one directory each, relative to this crate.
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");

/// The story the `one-ready` fixture store holds ready.
const STORY: &str = "story:ready-one";

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

/// A scratch tree on `main` holding a committed copy of the `one-ready` store and a
/// `check.txt` the story's work changes.
fn story_scratch(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("fenced-{}-{name}", std::process::id()));
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

/// The options of a run that takes the store's next story.
fn story_options() -> RunOptions {
    RunOptions {
        protocol: PROTOCOL.to_owned(),
        intent: String::new(),
        inputs: Vec::new(),
        iterations: Some(8),
        hours: None,
        tokens: None,
        case_id: None,
        gate: Some("repository.merge".to_owned()),
    }
}

/// Hands the work over and requests the merge gate as soon as each is offered, changing
/// nothing.
struct HandsOver;

impl Harness for HandsOver {
    fn name(&self) -> &str {
        "hands-over"
    }

    fn step(&self, context: &StepContext<'_>) -> Result<StepReport, anyhow::Error> {
        if context.approvals.contains(&"repository.merge".to_owned()) {
            return Ok(StepReport {
                action: "repository.merge".to_owned(),
                summary: "merge it".to_owned(),
                tokens: None,
            });
        }
        if context.offered.contains(&"change.handover".to_owned()) {
            return Ok(StepReport {
                action: "change.handover".to_owned(),
                summary: "hand it over".to_owned(),
                tokens: None,
            });
        }
        Err(anyhow!(
            "the gate was not offered; a judgement step among {} is left",
            context.offered.join(", ")
        ))
    }
}

/// `gate.run` without a gate: one `gate_result` pass at the revision.
struct GatePass;

impl Validator for GatePass {
    fn name(&self) -> &str {
        "record.gate-pass"
    }

    fn action(&self) -> &str {
        "gate.run"
    }

    fn kind(&self) -> &str {
        "gate_result"
    }

    fn subject(&self) -> &str {
        "implementation"
    }

    fn observe(&self, _: &Path, revision: &str, seq: usize) -> Result<Observation, anyhow::Error> {
        Ok(Observation {
            summary: "pass".to_owned(),
            records: vec![Record::new(
                format!("gate-result-{seq}"),
                self.kind(),
                Some("pass".to_owned()),
                self.subject(),
                revision,
                self.name(),
            )],
            artifact: None,
        })
    }
}

/// The validators of a story's delivery through `forge`, in the order the binary registers
/// them: the tests validator, its command `true`, `change.read`, a gate that passes, and the
/// forge's three.
fn story_validators(forge: Rc<dyn Forge>) -> Vec<Box<dyn Validator>> {
    vec![
        Box::new(TestsRun::new("true").expect("a command")),
        Box::new(ChangeRead),
        Box::new(GatePass),
        Box::new(PrOpen::new(forge.clone())),
        Box::new(CiRead::new(forge.clone())),
        Box::new(PrMerged::new(forge)),
    ]
}

/// The store's story taken in `repo`, worked, run to the gate through `memory` and granted the
/// merge through `store`: the case's id, its directory and the approval's commit, the head
/// that carries the approval's rider above the revision granted.
fn story_granted(repo: &Path, memory: &Rc<Memory>, store: &Aep) -> (String, PathBuf, String) {
    let forge: Rc<dyn Forge> = memory.clone();
    let validators = story_validators(forge);
    let mut out = Vec::new();
    let opened = Run::open_next(
        repo,
        story_options(),
        store,
        &HandsOver,
        &validators,
        &mut out,
    )
    .expect("the frontier answers")
    .expect("the story is taken");
    let id = opened.case.id.clone();
    let dir = opened.case.dir.clone();
    drop(opened);
    fs::write(repo.join("check.txt"), "fixed\n").expect("a file");
    git(repo, &["commit", "-q", "-am", "the work"]);
    let mut run =
        Run::resume(repo, story_options(), &id, &HandsOver, &validators).expect("the case resumes");
    run.record_on(store);
    let mut out = Vec::new();
    let stop = run.run(&mut out).expect("the run stops without an error");
    drop(run);
    assert_eq!(
        stop,
        Stop::ApprovalRequired(vec!["repository.merge".to_owned()]),
        "{}",
        String::from_utf8_lossy(&out)
    );
    approve(repo, store, &id, "repository.merge", "human:operator").expect("the approval is given");
    (id, dir, git(repo, &["rev-parse", "HEAD"]))
}

/// The story case `id` in `repo` resumed through `memory`, given `store` or not, waiting on the
/// forge for 5 ms between observations.
fn resumed_story(repo: &Path, id: &str, memory: &Rc<Memory>, store: Option<&Aep>) -> Ran {
    let forge: Rc<dyn Forge> = memory.clone();
    let validators = story_validators(forge);
    let mut run =
        Run::resume(repo, story_options(), id, &NoHarness, &validators).expect("the case resumes");
    if let Some(store) = store {
        run.record_on(store);
    }
    run.poll = Duration::from_millis(5);
    let mut out = Vec::new();
    let stop = run.run(&mut out).map_err(|error| format!("{error:#}"));
    Ran {
        stop,
        text: String::from_utf8(out).expect("utf-8"),
    }
}

/// The fence holds against takt's riders: a run not given the store sends the merge at the head
/// that carries the approval's rider, and its answer is lost without the forge applying it. The
/// next run, given the store, writes the story's `test_result` and its move above that head,
/// riders to push while the forge reads the pull request open at the head the merge named.
/// While that merge may still land, the run pushes no rider and sends no merge: each step says
/// why and waits, and the run stops waiting at its budget. One merge call across the runs, the
/// head the one the merge named.
#[test]
fn an_unknown_merge_is_not_sent_again_over_a_rider_push() {
    let repo = story_scratch("rider");
    let memory = Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    });
    let store = Aep::default();
    let (id, case_dir, approved) = story_granted(&repo, &memory, &store);
    let branch = format!("takt/{id}");
    *memory.merge_answer.borrow_mut() = MergeAnswer::AnswerLost;

    let first = resumed_story(&repo, &id, &memory, None);
    let merges = steps(&case_dir, "repository.merge");
    assert_eq!(
        merges[0]["forge_write"]["outcome"],
        "unknown",
        "{}\n{merges:#?}",
        first.said()
    );
    assert_eq!(
        merges[0]["forge_write"]["head"],
        approved.as_str(),
        "{merges:#?}"
    );
    assert_eq!(merge_calls(&memory), 1, "{:#?}", memory.calls.borrow());
    assert_eq!(
        memory.pushed.borrow().get(&branch),
        Some(&approved),
        "{}",
        first.said()
    );
    let pushed_before = pushes(&memory);

    let next = resumed_story(&repo, &id, &memory, Some(&store));
    let riders: Vec<String> = git(&repo, &["log", "--format=%s", &format!("{approved}..HEAD")])
        .lines()
        .map(str::to_owned)
        .collect();
    assert_eq!(
        riders,
        [
            takt::frontier::implemented_subject(STORY),
            takt::frontier::test_result_subject(STORY)
        ],
        "the store's riders are written above the head the merge named: {}",
        next.said()
    );
    let outcomes: Vec<String> = steps(&case_dir, "repository.merge")
        .iter()
        .map(|line| {
            line["forge_write"]["outcome"]
                .as_str()
                .unwrap_or("absent")
                .to_owned()
        })
        .collect();
    assert_eq!(
        merge_calls(&memory),
        1,
        "story:fenced-forge-dispatch: a merge recorded unknown and read open at its head is never \
         sent again, and no rider push moves the head from under it; outcomes {outcomes:?}\n{}\n{:#?}",
        next.said(),
        memory.calls.borrow()
    );
    assert_eq!(
        pushes(&memory),
        pushed_before,
        "story:fenced-forge-dispatch: no rider is pushed while the merge may land: {:#?}",
        memory.calls.borrow()
    );
    assert_eq!(
        memory.pushed.borrow().get(&branch),
        Some(&approved),
        "the pull request's head is the one the merge named"
    );
    assert!(
        matches!(&next.stop, Ok(Stop::Waiting { on, .. }) if on == &["repository.merge"]),
        "the run stops waiting at its budget: {}",
        next.said()
    );
    assert!(
        next.text.contains("was sent and its outcome is not known")
            && next.text.contains("riders above it")
            && next.text.contains("are not pushed"),
        "{}",
        next.said()
    );
    assert!(merge_observations(&case_dir).is_empty(), "{}", next.said());
}

/// The security boundary: resolving an `unknown` never merges without the grant. With the
/// case's grant gone after the merge whose answer was lost, the next run takes no merge step at
/// all and sends nothing.
#[test]
fn an_unknown_merge_is_never_resolved_into_a_merge_without_the_grant() {
    let tree = scratch("ungranted");
    let memory = Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    });
    let (_, case_dir) = granted(&tree, "ungranted", &memory);
    *memory.merge_answer.borrow_mut() = MergeAnswer::AnswerLost;
    let first = resumed(&tree, "ungranted", &memory, Duration::from_millis(5));
    assert_eq!(
        steps(&case_dir, "repository.merge")[0]["forge_write"]["outcome"],
        "unknown",
        "{}",
        first.said()
    );
    fs::remove_file(case_dir.join(AUTHORITY_FILE)).expect("the grant is withdrawn");

    let next = resumed(&tree, "ungranted", &memory, Duration::from_millis(5));
    assert_eq!(merge_calls(&memory), 1, "{:#?}", memory.calls.borrow());
    assert!(memory.merged.borrow().is_empty(), "{}", next.said());
    assert!(merge_observations(&case_dir).is_empty(), "{}", next.said());
    assert_eq!(
        steps(&case_dir, "repository.merge").len(),
        1,
        "{}",
        next.said()
    );
}

/// A command forge runner that answers each line from a script.
struct Scripted(RefCell<VecDeque<anyhow::Result<Exit>>>);

impl Runner for Scripted {
    fn run(&self, _tree: &Path, _argv: &[String]) -> anyhow::Result<Exit> {
        self.0
            .borrow_mut()
            .pop_front()
            .expect("the script has an answer for each line")
    }

    fn run_env(&self, tree: &Path, argv: &[String], _env: &[(&str, &str)]) -> anyhow::Result<Exit> {
        self.run(tree, argv)
    }
}

/// A command forge over `tree` whose `merge` entry is `merge`, `3` its one refusal exit, every
/// line run through `runner`.
fn command_forge(tree: &Path, merge: &[&str], runner: Box<dyn Runner>) -> CommandForge {
    let argv = |words: &[&str]| -> Option<Vec<String>> {
        Some(words.iter().map(|word| (*word).to_owned()).collect())
    };
    CommandForge::for_tree(
        tree,
        ForgeCommands {
            read: argv(&["tool", "read", "{number}"]),
            merge: argv(merge),
            refused_exit: Some(vec![3]),
            ..ForgeCommands::default()
        },
        &["repository.merge".to_owned()],
        tree.join("state"),
        Scan::None {
            declared_in: tree.join("run.yaml"),
        },
        runner,
    )
    .expect("a command forge")
}

/// The class of the merge `forge` fails, and its error.
fn failed_merge(forge: &CommandForge, tree: &Path) -> (WriteOutcome, String) {
    let error = forge
        .merge(tree, &tree.join("artifacts"), 7, "0123456789abcdef")
        .expect_err("the merge fails");
    (error.outcome(), format!("{error:#}"))
}

/// The command forge maps each way a merge fails to one of the four classes: an exit the run
/// file lists in `refused_exit` is `refused`; another exit, an end by a signal, and a program
/// the runner killed past its time limit are `unknown`, since the program may have sent the
/// merge first; a program that could not be started sent nothing, `not_attempted`.
#[test]
fn the_command_forge_maps_each_failed_merge_to_its_class() {
    let tree = scratch("command-classes");
    let exit = |code| {
        Ok(Exit {
            code,
            stdout: String::new(),
            stderr: "the forge said no".to_owned(),
        })
    };
    let scripted = command_forge(
        &tree,
        &["tool", "merge", "{number}", "{head}"],
        Box::new(Scripted(RefCell::new(VecDeque::from([
            exit(Some(3)),
            exit(Some(4)),
            exit(None),
        ])))),
    );
    for (expected, what) in [
        (WriteOutcome::Refused, "a refusal exit"),
        (WriteOutcome::Unknown, "another exit"),
        (WriteOutcome::Unknown, "an end by a signal"),
    ] {
        let (outcome, error) = failed_merge(&scripted, &tree);
        assert_eq!(outcome, expected, "{what}: {error}");
    }

    let timed = command_forge(
        &tree,
        &["sleep", "30"],
        Box::new(Process {
            limits: Limits {
                timeout: Duration::from_millis(200),
                output: 1 << 16,
            },
        }),
    );
    let error = timed
        .merge(&tree, &tree.join("artifacts"), 7, "0123456789abcdef")
        .expect_err("the merge outlives its limit");
    assert!(
        error.chain().any(|cause| cause.is::<TimedOut>()),
        "the runner killed the merge past its limit: {error:#}"
    );
    assert_eq!(error.outcome(), WriteOutcome::Unknown, "{error:#}");

    let missing = tree.join("no-such-program");
    let unstarted = command_forge(
        &tree,
        &[missing.to_str().expect("utf-8")],
        Box::new(Process::default()),
    );
    let (outcome, error) = failed_merge(&unstarted, &tree);
    assert_eq!(outcome, WriteOutcome::NotAttempted, "{error}");
}
