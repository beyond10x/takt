//! Adversary pass 2 on story:records-reach-base. The defects it showed: a store write the
//! per-revision done rule made again on every run once the case's revision moved, a merge
//! taken with none of the story's records after the store refused the `test_result`, and
//! takt's own actor accepted as the approver of a story's merge. Each case is named for what it
//! asserts now that the defect is closed.

use std::cell::Cell;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

use anyhow::anyhow;
use takt::case::Approval;
use takt::evidence::Record;
use takt::forge::{Checks, Forge, Memory, PullRequest, PullRequestView};
use takt::frontier::{Aep, Take, Written, approve};
use takt::harness::{Harness, StepContext, StepReport};
use takt::validator::{ChangeRead, CiRead, Observation, PrMerged, PrOpen, TestsRun, Validator};
use takt::{Frontier, Run, RunOptions, Stop, Story};

const DELIVERY: &str = "story.delivery/1";
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");
const APPROVER: &str = "human:operator";

fn git(tree: &Path, args: &[&str]) -> String {
    String::from_utf8(git_bytes(tree, args))
        .expect("utf-8")
        .trim()
        .to_owned()
}

fn git_bytes(tree: &Path, args: &[&str]) -> Vec<u8> {
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
    output.stdout
}

/// Adds `text` to the index of `tree` at `path`, byte for byte, through git alone: git holds a
/// name that is not UTF-8, which a file system that keeps names in UTF-8 only (APFS) refuses.
#[cfg(unix)]
fn stage(tree: &Path, path: &[u8], text: &[u8]) {
    use std::io::Write as _;
    use std::os::unix::ffi::OsStrExt as _;
    use std::process::Stdio;
    let mut hash = Command::new("git")
        .args(["hash-object", "-w", "--stdin"])
        .current_dir(tree)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("git runs");
    hash.stdin
        .take()
        .expect("its input")
        .write_all(text)
        .expect("the blob");
    let hashed = hash.wait_with_output().expect("git ends");
    assert!(hashed.status.success(), "git hash-object");
    let blob = String::from_utf8(hashed.stdout).expect("utf-8");
    let added = Command::new("git")
        .args([
            "update-index",
            "--add",
            "--cacheinfo",
            "100644",
            blob.trim(),
        ])
        .arg(std::ffi::OsStr::from_bytes(path))
        .current_dir(tree)
        .status()
        .expect("git runs");
    assert!(added.success(), "git update-index");
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
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "records-reach-base-adv2-{}-{name}",
        std::process::id()
    ));
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

fn options() -> RunOptions {
    RunOptions {
        protocol: DELIVERY.to_owned(),
        intent: String::new(),
        inputs: Vec::new(),
        iterations: Some(12),
        hours: None,
        tokens: None,
        case_id: None,
        gate: Some("repository.merge".to_owned()),
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

struct Requester;

impl Harness for Requester {
    fn name(&self) -> &str {
        "requester"
    }

    fn step(&self, context: &StepContext<'_>) -> Result<StepReport, anyhow::Error> {
        if context.approvals.contains(&"repository.merge".to_owned()) {
            return Ok(StepReport {
                action: "repository.merge".to_owned(),
                summary: "merge it".to_owned(),
                tokens: None,
            });
        }
        // The work committed is complete: hand it over, changing nothing.
        if context.offered.contains(&"change.handover".to_owned()) {
            return Ok(StepReport {
                action: "change.handover".to_owned(),
                summary: "hand it over".to_owned(),
                tokens: None,
            });
        }
        Err(anyhow!("the gate was not offered"))
    }
}

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

/// The memory forge whose checks answer what `checks` holds now.
struct Switch {
    memory: Rc<Memory>,
    checks: Cell<Checks>,
}

impl Forge for Switch {
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
        "memory"
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
        self.memory.checks(tree, number)?;
        Ok(self.checks.get())
    }

    fn merge(&self, tree: &Path, dir: &Path, number: u64, head: &str) -> anyhow::Result<()> {
        self.memory.merge(tree, dir, number, head)
    }
}

/// The planning store as `aep`, except that it refuses the first `test_result` write, as a
/// store refuses a write it will take a moment later.
struct RefusesTestResultOnce {
    aep: Aep,
    refused: Cell<bool>,
}

impl Frontier for RefusesTestResultOnce {
    fn name(&self) -> &str {
        "aep"
    }

    fn candidates(&self, repo: &Path) -> anyhow::Result<Vec<String>> {
        self.aep.candidates(repo)
    }

    fn active(&self, repo: &Path) -> anyhow::Result<Vec<String>> {
        self.aep.active(repo)
    }

    fn show(&self, repo: &Path, id: &str) -> anyhow::Result<Story> {
        self.aep.show(repo, id)
    }

    fn take(&self, repo: &Path, id: &str) -> anyhow::Result<Take> {
        self.aep.take(repo, id)
    }

    fn record_approval(&self, repo: &Path, story: &str, approval: &Approval) -> anyhow::Result<()> {
        self.aep.record_approval(repo, story, approval)
    }

    fn record_test_result(
        &self,
        repo: &Path,
        story: &str,
        reference: &str,
        at: &str,
    ) -> anyhow::Result<Written> {
        if !self.refused.replace(true) {
            return Ok(Written::Refused(
                "the store is held by another writer".to_owned(),
            ));
        }
        self.aep.record_test_result(repo, story, reference, at)
    }

    fn implemented(&self, repo: &Path, story: &str) -> anyhow::Result<Written> {
        self.aep.implemented(repo, story)
    }
}

fn delivery(forge: Rc<dyn Forge>) -> Vec<Box<dyn Validator>> {
    vec![
        Box::new(TestsRun::new("true").expect("a command")),
        Box::new(ChangeRead),
        Box::new(GatePass),
        Box::new(PrOpen::new(forge.clone())),
        Box::new(CiRead::new(forge.clone())),
        Box::new(PrMerged::new(forge)),
    ]
}

struct Case {
    repo: Scratch,
    forge: Rc<Switch>,
    id: String,
    dir: PathBuf,
    granted: String,
}

/// A case of `story:ready-one` taken, worked and run to the merge gate, its checks passing.
fn gated(name: &str) -> Case {
    let repo = repository(name);
    let forge = Rc::new(Switch {
        memory: Rc::new(Memory {
            checks: Checks::Pass,
            ..Memory::default()
        }),
        checks: Cell::new(Checks::Pass),
    });
    let validators = delivery(forge.clone());
    let store = Aep::default();
    let mut out = Vec::new();
    let opened = Run::open_next(&repo, options(), &store, &Requester, &validators, &mut out)
        .expect("the frontier answers")
        .expect("story:ready-one is taken");
    let id = opened.case.id.clone();
    let dir = opened.case.dir.clone();
    drop(opened);
    fs::write(repo.join("check.txt"), "fixed\n").expect("a file");
    git(&repo, &["commit", "-q", "-am", "the work"]);
    let granted = git(&repo, &["rev-parse", "HEAD"]);
    let case = Case {
        repo,
        forge,
        id,
        dir,
        granted,
    };
    let (stop, text) = case.run_with(&Requester);
    assert_eq!(
        stop,
        Stop::ApprovalRequired(vec!["repository.merge".to_owned()]),
        "{text}"
    );
    case
}

fn approved(name: &str) -> Case {
    let case = gated(name);
    approve(
        &case.repo,
        &Aep::default(),
        &case.id,
        "repository.merge",
        APPROVER,
    )
    .expect("the approval is given");
    case
}

impl Case {
    fn run_with(&self, harness: &dyn Harness) -> (Stop, String) {
        self.run_on(harness, &Aep::default())
    }

    fn run_on(&self, harness: &dyn Harness, store: &dyn Frontier) -> (Stop, String) {
        let forge: Rc<dyn Forge> = self.forge.clone();
        let validators = delivery(forge);
        let mut run = Run::resume(&self.repo, options(), &self.id, harness, &validators)
            .expect("the case resumes");
        run.record_on(store);
        run.poll = std::time::Duration::from_millis(5);
        let mut out = Vec::new();
        let stop = run.run(&mut out).expect("the run stops without an error");
        (stop, String::from_utf8_lossy(&out).into_owned())
    }

    /// The lines of the case's `events.jsonl`, parsed.
    fn events(&self) -> Vec<serde_json::Value> {
        fs::read_to_string(self.dir.join("events.jsonl"))
            .expect("the run record")
            .lines()
            .map(|line| serde_json::from_str(line).expect("a json line"))
            .collect()
    }
}

/// Correction 1's done rule: a write counts as done only when kept under the case's current
/// revision. Once the revision moved from R1 to R2, the R1 move to `implemented` sits in the
/// history below R2, so the story at R2 is already `implemented` and the store will not move it
/// again: nothing is ever kept as the move at R2, the move is never done, and every later run of
/// the accepted case (`takt run --case`) writes to the store again and records the store's
/// refusal again. The record says a refusal is recorded once and never retried, and an accepted
/// case resumed by id has nothing due.
#[test]
fn a_move_made_below_a_later_revision_is_not_tried_again_on_a_later_run() {
    let case = approved("move-retried");
    case.forge.checks.set(Checks::Pending);
    let (stop, text) = case.run_with(&NoHarness);
    assert!(matches!(stop, Stop::Waiting { .. }), "{stop:?}\n{text}");
    fs::write(case.repo.join("check.txt"), "fixed again\n").expect("a file");
    git(&case.repo, &["commit", "-q", "-am", "the fix"]);
    case.forge.checks.set(Checks::Pass);
    let (stop, text) = case.run_with(&Requester);
    assert_eq!(
        stop,
        Stop::ApprovalRequired(vec!["repository.merge".to_owned()]),
        "{text}"
    );
    approve(
        &case.repo,
        &Aep::default(),
        &case.id,
        "repository.merge",
        APPROVER,
    )
    .expect("the approval at the second revision");
    let (stop, text) = case.run_with(&NoHarness);
    assert_eq!(stop, Stop::Completed("accepted".to_owned()), "{text}");

    let before = case.events().len();
    let (stop, text) = case.run_with(&NoHarness);
    assert_eq!(stop, Stop::Completed("accepted".to_owned()), "{text}");
    let again: Vec<serde_json::Value> = case
        .events()
        .into_iter()
        .skip(before)
        .filter(|line| {
            ["refusal", "store_move", "store_evidence"]
                .contains(&line["event"].as_str().unwrap_or(""))
        })
        .collect();
    assert!(
        again.is_empty() && !text.contains("store:"),
        "a run of the accepted case wrote to the store again: {again:?}\n{text}"
    );
}

/// The acceptance: after a story's pull request merges under a grant, the base holds the story
/// at `implemented` with its `test_result`. The defect: the store refuses the `test_result`
/// write once, just before the merge (a refusal the run relays and records); the move, which
/// the store allows only once the story holds a `test_result`, is refused too, and the merge was
/// taken anyway, the base holding the story `active` with no `test_result`. Now the run fails
/// closed for exactly that reason: it stops `StepRefused` because the records are not both
/// written, and nothing merges. A stop for any other reason fails the case.
#[test]
fn a_refused_test_result_stops_the_merge_as_records_not_both_written() {
    let case = approved("refused-once");
    let store = RefusesTestResultOnce {
        aep: Aep::default(),
        refused: Cell::new(false),
    };
    let (stop, text) = case.run_on(&NoHarness, &store);
    let merged = case.forge.memory.merged.borrow().clone();
    assert!(
        merged.is_empty(),
        "merged {merged:?} above {} without the store's records: {stop:?}\n{text}",
        case.granted
    );
    assert!(
        matches!(&stop, Stop::StepRefused(why) if why.contains("not both written")),
        "the merge was not refused for the missing records: {stop:?}\n{text}"
    );
}

/// An approver must be `human:<name>` or `agent:<role>`: who grants, apart from the store's and
/// tools' own actors. `agent:takt` is takt's own actor, the one its `test_result` and its move
/// are written as; granted as it, the store records the operator's grant of the merge as takt
/// approving its own merge, indistinguishable from takt's own writes.
#[test]
fn takt_own_actor_is_refused_as_the_approver_of_a_story_merge() {
    let case = gated("takt-approves");
    let approved = approve(
        &case.repo,
        &Aep::default(),
        &case.id,
        "repository.merge",
        takt::frontier::ACTOR,
    );
    assert!(
        approved.is_err(),
        "approved by {}, takt's own actor: {approved:?}",
        takt::frontier::ACTOR
    );
}

/// A store write is kept with the path git committed it at, byte for byte, which the rider rule
/// compares later: the path is read from git byte for byte (`-z`), never through a call that
/// trims or decodes it. A path that is not UTF-8 does not read as a path at all, and nothing is
/// kept; it is never kept as a different, decoded path.
#[cfg(unix)]
#[test]
fn a_store_write_at_a_path_that_is_not_utf8_is_kept_as_committed_or_not_at_all() {
    let repo = repository("not-utf8");
    let revision = git(&repo, &["rev-parse", "HEAD"]);
    let committed: &[u8] = b".engineering/evidence/story/ready-one/r\xff.json";
    stage(&repo, committed, b"{}\n");
    git(&repo, &["commit", "-q", "-m", "a store write"]);
    let commit = git(&repo, &["rev-parse", "HEAD"]);
    let case_dir = repo.join(".takt/cases/probe");
    fs::create_dir_all(&case_dir).expect("a directory");
    match takt::case::keep_rider(&repo, &case_dir, &revision, &commit, "test_result") {
        Err(error) => {
            assert!(
                !case_dir.join(takt::case::RIDERS_DIR).exists(),
                "refused ({error:#}) with something kept"
            );
        }
        Ok([_, at]) => {
            let kept = fs::read(&at).expect("the kept path");
            assert_eq!(
                kept,
                committed,
                "kept as {:?}, not the committed path",
                String::from_utf8_lossy(&kept)
            );
        }
    }
}
