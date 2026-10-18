//! Adversary pass 2 over story:approval-record-reaches-base, against correction 2: the narrowed
//! `approvals_only` (read from `git diff-tree --raw -z`), the head named in `branch.yaml` before
//! the push, and the checks read before the merge of a head carrying the approval's commits.

use std::cell::{Cell, RefCell};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

use anyhow::{anyhow, bail};
use takt::case::{Branch, CASES_DIR, CaseDir};
use takt::evidence::Record;
use takt::forge::{Checks, Forge, Memory, PullRequest, PullRequestView};
use takt::frontier::{Aep, approve};
use takt::harness::{Harness, StepContext, StepReport};
use takt::validator::{CiRead, PrMerged, PrOpen, Validator};
use takt::{Run, RunOptions, Stop};

const PROTOCOL: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../protocols/story-delivery/1.yaml"
);

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");

/// Where the store keeps the evidence on `story:ready-one`.
const EVIDENCE: &str = ".engineering/evidence/story/ready-one";

/// A record on `story:ready-one` the scratch repository holds from its first commit, below
/// every revision of the case: what a commit can change, rename or delete rather than add.
const SEEDED: &str = ".engineering/evidence/story/ready-one/20261018T000000Z-000-000000000000.json";

const SEEDED_TEXT: &str = r#"{
  "at": "2026-10-18T00:00:00Z",
  "actor": "agent:fixture",
  "artifact": "story:ready-one",
  "kind": "story",
  "revision": 3,
  "change": {
    "change": "evidence",
    "kind": "verification",
    "source": "fixture",
    "reference": "seeded before the case"
  }
}
"#;

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

/// A scratch repository on `main` holding the `one-ready` store; with `seed`, also a record on
/// the story from the first commit.
fn repository(name: &str, seed: bool) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "approval-record-adv2-{}-{name}",
        std::process::id()
    ));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    copy_dir(&Path::new(FIXTURES).join("one-ready"), &dir);
    fs::write(dir.join("check.txt"), "broken\n").expect("a file");
    if seed {
        fs::create_dir_all(dir.join(EVIDENCE)).expect("a directory");
        fs::write(dir.join(SEEDED), SEEDED_TEXT).expect("a file");
    }
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "start"]);
    Scratch(dir)
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
        Err(anyhow!("the gate was not offered"))
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

/// Inspects and changes nothing, so a run whose merge waits stops without an error.
struct Idle;

impl Harness for Idle {
    fn name(&self) -> &str {
        "idle"
    }

    /// It answers `repository.inspect`, which only a harness that keeps state is offered
    /// (story:no-inspect-for-stateless-harness).
    fn keeps_state(&self) -> bool {
        true
    }

    fn step(&self, _: &StepContext<'_>) -> Result<StepReport, anyhow::Error> {
        Ok(StepReport {
            action: "repository.inspect".to_owned(),
            summary: "nothing to change".to_owned(),
            tokens: None,
        })
    }
}

fn options() -> RunOptions {
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

fn validators(forge: Rc<dyn Forge>) -> Vec<Box<dyn Validator>> {
    vec![
        Box::new(PrOpen::new(forge.clone())),
        Box::new(CiRead::new(forge.clone())),
        Box::new(PrMerged::new(forge)),
    ]
}

fn record(n: usize, kind: &str, revision: &str) -> Record {
    Record::new(
        format!("e{n}"),
        kind,
        Some("pass".to_owned()),
        "implementation",
        revision,
        "approval-record-adv2",
    )
}

fn double() -> Rc<Memory> {
    Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    })
}

/// The case at the merge gate through the double: its id and the revision a grant authorises.
fn at_the_gate(repo: &Path, memory: &Rc<Memory>) -> (String, String) {
    let forge: Rc<dyn Forge> = memory.clone();
    let validators = validators(forge);
    let mut out = Vec::new();
    let opened = Run::open_next(
        repo,
        options(),
        &Aep::default(),
        &Requester,
        &validators,
        &mut out,
    )
    .expect("the frontier answers")
    .expect("story:ready-one is taken");
    let id = opened.case.id.clone();
    drop(opened);
    fs::write(repo.join("check.txt"), "fixed\n").expect("a file");
    git(repo, &["commit", "-q", "-am", "the work"]);
    let worked = git(repo, &["rev-parse", "HEAD"]);
    let mut run =
        Run::resume(repo, options(), &id, &Requester, &validators).expect("the case resumes");
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
    let mut out = Vec::new();
    let stop = run.run(&mut out).expect("the run stops without an error");
    assert_eq!(
        stop,
        Stop::ApprovalRequired(vec!["repository.merge".to_owned()]),
        "{}",
        String::from_utf8_lossy(&out)
    );
    (id, worked)
}

fn merge_run_with(
    repo: &Path,
    id: &str,
    forge: Rc<dyn Forge>,
    harness: &dyn Harness,
) -> (Result<Stop, String>, String) {
    let validators = validators(forge);
    let mut run = Run::resume(repo, options(), id, harness, &validators).expect("the case resumes");
    // A run that waits on the forge pauses between its observations; here only briefly.
    run.poll = std::time::Duration::from_millis(5);
    let mut out = Vec::new();
    let stop = run.run(&mut out).map_err(|error| format!("{error:#}"));
    (stop, String::from_utf8_lossy(&out).into_owned())
}

fn resumed_revision(repo: &Path, id: &str) -> String {
    CaseDir::resume(repo, id)
        .expect("the case resumes")
        .revision("implementation")
        .expect("a revision")
}

/// Commits what is staged, or `-a`, under the approval's subject; the commit.
fn approve_commit(repo: &Path, all: bool) -> String {
    let mut args = vec!["commit", "-q"];
    if all {
        args.push("-a");
    }
    args.extend(["-m", "takt: approve story:ready-one repository.merge"]);
    git(repo, &args);
    git(repo, &["rev-parse", "HEAD"])
}

/// A case approved through the double, the branch then put back at the granted revision so a
/// variant can stand in for the approval's commit: the one commit the one grant counts. Also
/// the text of the record the approval's commit added, as the store wrote it: what a variant
/// adds, so the only thing that differs from the approval's own commit is what it varies.
fn approved_case(name: &str) -> (Scratch, String, String, String) {
    let repo = repository(name, true);
    let memory = double();
    let (id, granted) = at_the_gate(&repo, &memory);
    approve(
        &repo,
        &Aep::default(),
        &id,
        "repository.merge",
        "human:operator",
    )
    .expect("the approval is given");
    assert_eq!(resumed_revision(&repo, &id), granted);
    let added = git(&repo, &["show", "--name-only", "--format=", "HEAD"]);
    assert!(
        added.starts_with(EVIDENCE) && added.lines().count() == 1,
        "{added}"
    );
    let record = git(&repo, &["show", &format!("HEAD:{added}")]);
    (repo, id, granted, record)
}

fn rewind(repo: &Path, id: &str, granted: &str) {
    git(repo, &["reset", "-q", "--hard", granted]);
    assert_eq!(resumed_revision(repo, id), granted);
}

/// Boundaries of what an approval's commit is: a new regular file directly in its story's
/// evidence, nothing else. One approve-subject commit on the granted revision, the one the
/// grant counts, that changes the record rather than adding one, or adds something that is not
/// a regular file, is no approval's: the resumed case moves to it. Each variant is a commit the
/// unit's own cases do not make (they add regular files only), so the mode and status checks of
/// `added_records` are otherwise unobserved. A file a variant adds holds the store's own record
/// of the grant, so the mode or the status is what refuses it, not the text.
#[test]
fn adv2_an_approve_commit_that_is_not_a_pure_addition_of_a_regular_file_is_no_approval() {
    let (repo, id, granted, record) = approved_case("not-added");
    let new_record = format!("{EVIDENCE}/20261018T999999Z-000-111111111111.json");

    // A new file, executable.
    rewind(&repo, &id, &granted);
    let path = repo.join(&new_record);
    fs::write(&path, &record).expect("a file");
    fs::set_permissions(&path, std::os::unix::fs::PermissionsExt::from_mode(0o755)).expect("chmod");
    git(&repo, &["add", &new_record]);
    let executable = approve_commit(&repo, false);
    assert_eq!(
        git(&repo, &["ls-tree", "HEAD", "--", &new_record])
            .split(' ')
            .next(),
        Some("100755")
    );
    assert_eq!(
        resumed_revision(&repo, &id),
        executable,
        "an executable file"
    );

    // A new symlink in the story's evidence whose target is the store's record of the grant: the
    // blob git holds is that text, so the mode is what refuses it.
    rewind(&repo, &id, &granted);
    std::os::unix::fs::symlink(&record, repo.join(&new_record)).expect("a symlink");
    git(&repo, &["add", &new_record]);
    assert_eq!(
        git(&repo, &["cat-file", "-p", &format!(":{new_record}")]),
        record,
        "the symlink's blob is the record"
    );
    let symlink = approve_commit(&repo, false);
    assert_eq!(resumed_revision(&repo, &id), symlink, "a symlink");

    // A record added, and the seeded record rewritten in the same commit.
    rewind(&repo, &id, &granted);
    fs::write(repo.join(&new_record), &record).expect("a file");
    fs::write(repo.join(SEEDED), "{\"rewritten\": true}\n").expect("a file");
    git(&repo, &["add", "-A", EVIDENCE]);
    let modifies = approve_commit(&repo, false);
    assert_eq!(
        resumed_revision(&repo, &id),
        modifies,
        "a record added beside a rewritten one"
    );

    // Only the seeded record rewritten.
    rewind(&repo, &id, &granted);
    fs::write(repo.join(SEEDED), "{\"rewritten\": true}\n").expect("a file");
    let rewrites = approve_commit(&repo, true);
    assert_eq!(resumed_revision(&repo, &id), rewrites, "a rewritten record");

    // The seeded record's mode changed, its content not.
    rewind(&repo, &id, &granted);
    fs::set_permissions(
        repo.join(SEEDED),
        std::os::unix::fs::PermissionsExt::from_mode(0o755),
    )
    .expect("chmod");
    let chmod = approve_commit(&repo, true);
    assert_eq!(resumed_revision(&repo, &id), chmod, "a mode change");

    // The seeded record renamed within the story's evidence: a deletion and an addition.
    rewind(&repo, &id, &granted);
    git(&repo, &["mv", SEEDED, &new_record]);
    let renamed = approve_commit(&repo, false);
    assert_eq!(resumed_revision(&repo, &id), renamed, "a rename");

    // The seeded record deleted.
    rewind(&repo, &id, &granted);
    git(&repo, &["rm", "-q", SEEDED]);
    let deleted = approve_commit(&repo, false);
    assert_eq!(resumed_revision(&repo, &id), deleted, "a deletion");
}

/// Names the store never writes but that are still a new regular file directly in the story's
/// evidence (a newline and a tab in the name) are read byte for byte and admitted; a sibling
/// directory whose name has the story's name as a prefix, and a file beside the story's
/// directory whose name starts with it, are not the story's evidence. Each file holds the store's
/// own record of the grant, so the name is what decides.
#[test]
fn adv2_names_are_read_byte_for_byte_and_a_story_name_prefix_is_not_the_story() {
    let (repo, id, granted, record) = approved_case("names");

    rewind(&repo, &id, &granted);
    let odd = format!("{EVIDENCE}/odd\nname\t.json");
    fs::write(repo.join(&odd), &record).expect("a file");
    git(&repo, &["add", &odd]);
    approve_commit(&repo, false);
    assert_eq!(
        resumed_revision(&repo, &id),
        granted,
        "a record named with a newline and a tab"
    );

    rewind(&repo, &id, &granted);
    let sibling = ".engineering/evidence/story/ready-one-x/record.json";
    fs::create_dir_all(repo.join(sibling).parent().expect("a parent")).expect("a directory");
    fs::write(repo.join(sibling), &record).expect("a file");
    git(&repo, &["add", sibling]);
    let other = approve_commit(&repo, false);
    assert_eq!(
        resumed_revision(&repo, &id),
        other,
        "a sibling story's directory"
    );

    rewind(&repo, &id, &granted);
    let beside = ".engineering/evidence/story/ready-onerecord.json";
    fs::write(repo.join(beside), &record).expect("a file");
    git(&repo, &["add", beside]);
    let prefixed = approve_commit(&repo, false);
    assert_eq!(
        resumed_revision(&repo, &id),
        prefixed,
        "a file beside the directory"
    );
}

/// The double, with a push that never reaches the forge on the first try: the bot route's child
/// failing before the remote moved.
struct NeverReaches {
    memory: Rc<Memory>,
    fail_next_push: Cell<bool>,
}

impl Forge for NeverReaches {
    fn name(&self) -> &str {
        "never-reaches"
    }

    fn push(&self, tree: &Path, branch: &str) -> anyhow::Result<()> {
        if self.fail_next_push.replace(false) {
            bail!("b10x-gates bot -- push exited 128: could not read from the remote");
        }
        self.memory.push(tree, branch)
    }

    fn push_revision(&self, tree: &Path, branch: &str, revision: &str) -> anyhow::Result<()> {
        if self.fail_next_push.replace(false) {
            bail!("b10x-gates bot -- push exited 128: could not read from the remote");
        }
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
        self.memory.merge(tree, dir, number, head)
    }
}

/// `branch.yaml` names the approval's head before the push; a push that never reached the forge
/// leaves the pull request at the granted revision and `branch.yaml` at the approval's head. The
/// next run must push again and merge the approval's head, not stop at the mismatch.
#[test]
fn adv2_a_push_that_never_reached_the_forge_is_pushed_again_and_merged_by_the_next_run() {
    let repo = repository("never-reached", false);
    let memory = double();
    let (id, granted) = at_the_gate(&repo, &memory);
    let branch = format!("takt/{id}");
    approve(
        &repo,
        &Aep::default(),
        &id,
        "repository.merge",
        "human:operator",
    )
    .expect("the approval is given");
    let approved = git(&repo, &["rev-parse", "HEAD"]);
    let forge = Rc::new(NeverReaches {
        memory: memory.clone(),
        fail_next_push: Cell::new(true),
    });

    let (first, first_text) = merge_run_with(&repo, &id, forge.clone(), &NoHarness);
    assert!(first.is_err(), "{first:?}\n{first_text}");
    assert_eq!(memory.pushed.borrow().get(&branch), Some(&granted));
    assert_eq!(
        Branch::read(&repo.join(CASES_DIR).join(&id))
            .expect("reads")
            .pushed
            .as_deref(),
        Some(approved.as_str())
    );

    let (second, second_text) = merge_run_with(&repo, &id, forge, &NoHarness);
    assert_eq!(
        memory.merged.borrow().as_slice(),
        [1],
        "{second:?}\n{second_text}"
    );
    assert_eq!(memory.pushed.borrow().get(&branch), Some(&approved));
}

/// The double, its checks answering per head from `checks_on` (the double's answer for any head
/// not listed), and, with `require_checks`, a merge of a head whose checks the forge holds as
/// not passing (`passing` lacks it) refused the way the API refuses one. With `stale_checks`,
/// `checks` answers for the head the pull request had before the last push, as a forge whose
/// checks lag its head does.
struct PerHead {
    memory: Rc<Memory>,
    checks_on: RefCell<Vec<(String, Checks)>>,
    require_checks: bool,
    passing: RefCell<Vec<String>>,
    stale_checks: bool,
    previous: RefCell<Option<String>>,
}

impl PerHead {
    fn new(memory: Rc<Memory>) -> Self {
        Self {
            memory,
            checks_on: RefCell::new(Vec::new()),
            require_checks: false,
            passing: RefCell::new(Vec::new()),
            stale_checks: false,
            previous: RefCell::new(None),
        }
    }
}

impl Forge for PerHead {
    fn name(&self) -> &str {
        "per-head"
    }

    fn push(&self, tree: &Path, branch: &str) -> anyhow::Result<()> {
        let before = self.memory.pushed.borrow().get(branch).cloned();
        *self.previous.borrow_mut() = before;
        self.memory.push(tree, branch)
    }

    fn push_revision(&self, tree: &Path, branch: &str, revision: &str) -> anyhow::Result<()> {
        let before = self.memory.pushed.borrow().get(branch).cloned();
        *self.previous.borrow_mut() = before;
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
        let current = self.memory.pull_request(tree, number)?.head.sha;
        let head = if self.stale_checks {
            self.previous.borrow().clone().unwrap_or(current)
        } else {
            current
        };
        Ok(self
            .checks_on
            .borrow()
            .iter()
            .find(|(on, _)| *on == head)
            .map_or(answer, |(_, checks)| *checks))
    }

    fn merge(&self, tree: &Path, dir: &Path, number: u64, head: &str) -> anyhow::Result<()> {
        if self.require_checks && !self.passing.borrow().iter().any(|on| on == head) {
            bail!(
                "b10x-gates api --method PUT --path /repos/o/r/pulls/{number}/merge exited 1: 405 Required status check \"takt / task check\" is expected"
            );
        }
        self.memory.merge(tree, dir, number, head)
    }
}

/// Checks that fail on the approval's head merge nothing; once they pass there (a re-run of the
/// failing check), the next run merges that head, with one merge observation about the granted
/// revision.
#[test]
fn adv2_checks_failing_then_passing_on_the_approval_head_merge_on_the_next_run() {
    let repo = repository("fail-then-pass", false);
    let memory = double();
    let (id, granted) = at_the_gate(&repo, &memory);
    let dir = repo.join(CASES_DIR).join(&id);
    approve(
        &repo,
        &Aep::default(),
        &id,
        "repository.merge",
        "human:operator",
    )
    .expect("the approval is given");
    let approved = git(&repo, &["rev-parse", "HEAD"]);
    let forge = Rc::new(PerHead::new(memory.clone()));
    forge
        .checks_on
        .borrow_mut()
        .push((approved.clone(), Checks::Fail));

    let (first, first_text) = merge_run_with(&repo, &id, forge.clone(), &Idle);
    assert!(first.is_ok(), "{first:?}\n{first_text}");
    assert!(memory.merged.borrow().is_empty(), "{first_text}");

    forge.checks_on.borrow_mut().clear();
    let (second, second_text) = merge_run_with(&repo, &id, forge, &NoHarness);
    assert_eq!(
        second.as_ref().ok(),
        Some(&Stop::Completed("accepted".to_owned())),
        "{second:?}\n{second_text}"
    );
    assert_eq!(memory.merged.borrow().as_slice(), [1]);
    let observed: Vec<serde_json::Value> = fs::read_dir(dir.join("evidence"))
        .expect("evidence")
        .map(|entry| {
            serde_json::from_str(
                &fs::read_to_string(entry.expect("an entry").path()).expect("a file"),
            )
            .expect("json")
        })
        .filter(|record: &serde_json::Value| record["kind"] == "merge_observation")
        .collect();
    assert_eq!(observed.len(), 1, "{second_text}");
    assert_eq!(observed[0]["subject_revision"], granted.as_str());
}

/// Correction 2 (J3) asks that a merge the forge refuses for pending required checks be
/// "nothing merged, no record", the run stopping without an error. The validator reads the
/// checks before the merge and trusts the answer; a forge whose checks still answer for the head
/// before the push (the granted revision, green) passes that read, and the merge of the
/// approval's head is then refused for its required check: the run ends in an error. The checks
/// read names no head (`Forge::checks(tree, number)`), so nothing ties the answer to the head
/// merged.
#[test]
fn adv2_checks_answering_for_the_head_before_the_push_do_not_end_the_run_in_an_error() {
    let repo = repository("stale-checks", false);
    let memory = double();
    let (id, granted) = at_the_gate(&repo, &memory);
    approve(
        &repo,
        &Aep::default(),
        &id,
        "repository.merge",
        "human:operator",
    )
    .expect("the approval is given");
    let approved = git(&repo, &["rev-parse", "HEAD"]);
    let mut forge = PerHead::new(memory.clone());
    forge.require_checks = true;
    forge.stale_checks = true;
    forge.passing.borrow_mut().push(granted.clone());
    let forge = Rc::new(forge);

    // The checks read names no head, so a forge whose answer lags the push lets the merge be
    // tried, and the forge refuses it for its required check. Since story:ci-pending-waits that
    // refusal is nothing merged and no record: the run waits on the forge, offering the harness
    // no step (`NoHarness` would end it in an error), and stops `Waiting` once its step budget
    // ends.
    let (first, first_text) = merge_run_with(&repo, &id, forge.clone(), &NoHarness);
    assert!(memory.merged.borrow().is_empty(), "{first_text}");
    assert!(
        first.is_ok(),
        "the merge of {} refused for its required check after a checks read that answered for {} ended the run in an error: {first:?}\n{first_text}",
        &approved[..8],
        &granted[..8]
    );
    assert_eq!(
        first,
        Ok(Stop::Waiting {
            on: vec!["repository.merge".to_owned()],
            budget: Some(Box::new(Stop::Iterations(8))),
        }),
        "{first_text}"
    );
    assert!(
        first_text.contains("for a required check that has not reported"),
        "{first_text}"
    );

    forge.passing.borrow_mut().push(approved.clone());
    let (second, second_text) = merge_run_with(&repo, &id, forge, &NoHarness);
    assert_eq!(
        memory.merged.borrow().as_slice(),
        [1],
        "{second:?}\n{second_text}"
    );
}
