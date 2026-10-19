//! Adversary pass 1 on story:records-reach-base. Each case is named for the defect it shows:
//! riders the rule admits that are not takt's records for the granted revision, records of an
//! earlier revision reaching the base with a later grant, and an approver the commit says is
//! refused that is not.

use std::cell::Cell;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

use anyhow::anyhow;
use takt::case::approvals_only;
use takt::evidence::Record;
use takt::forge::{Checks, Forge, Memory, PullRequest, PullRequestView};
use takt::frontier::{Aep, approve};
use takt::harness::{Harness, StepContext, StepReport};
use takt::validator::{ChangeRead, CiRead, Observation, PrMerged, PrOpen, TestsRun, Validator};
use takt::{Run, RunOptions, Stop};

const DELIVERY: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../protocols/story-delivery/1.yaml"
);
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");
const STORY_FILE: &str = ".engineering/planning/story/ready-one.md";
const EVIDENCE: &str = ".engineering/evidence/story/ready-one";
const APPROVER: &str = "human:operator";
const TEST_RESULT: &str = "takt: test_result story:ready-one";
const IMPLEMENTED: &str = "takt: implemented story:ready-one";

type Commit<'a> = (&'a str, Vec<(&'a str, Vec<u8>)>);

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
        "records-reach-base-adv1-{}-{name}",
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

/// The memory forge whose checks answer what `checks` holds now: pass at the revision, pending
/// once the riders are pushed, as a forge whose CI is slower than the merge step.
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
    approved: String,
}

/// A case of `story:ready-one` taken, worked and run to the merge gate through the switchable
/// forge, its checks passing: the case, not yet granted.
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
        granted: granted.clone(),
        approved: granted,
    };
    let (stop, text) = case.run_with(&Requester);
    assert_eq!(
        stop,
        Stop::ApprovalRequired(vec!["repository.merge".to_owned()]),
        "{text}"
    );
    case
}

/// [`gated`], then granted the merge through the store by [`APPROVER`].
fn approved(name: &str) -> Case {
    let mut case = gated(name);
    approve(
        &case.repo,
        &Aep::default(),
        &case.id,
        "repository.merge",
        APPROVER,
    )
    .expect("the approval is given");
    case.approved = git(&case.repo, &["rev-parse", "HEAD"]);
    case
}

impl Case {
    fn run_with(&self, harness: &dyn Harness) -> (Stop, String) {
        let forge: Rc<dyn Forge> = self.forge.clone();
        let validators = delivery(forge);
        let store = Aep::default();
        let mut run = Run::resume(&self.repo, options(), &self.id, harness, &validators)
            .expect("the case resumes");
        run.record_on(&store);
        run.poll = std::time::Duration::from_millis(5);
        let mut out = Vec::new();
        let stop = run.run(&mut out).expect("the run stops without an error");
        (stop, String::from_utf8_lossy(&out).into_owned())
    }

    fn pushed(&self) -> String {
        self.forge
            .memory
            .pushed
            .borrow()
            .get(&format!("takt/{}", self.id))
            .cloned()
            .expect("the branch was pushed")
    }

    fn rides(&self, head: &str) -> bool {
        approvals_only(&self.repo, &self.granted, head, &self.dir).expect("the rider rule reads")
    }

    fn commits(&self, base: &str, commits: &[Commit<'_>]) -> String {
        git(&self.repo, &["reset", "-q", "--hard", base]);
        for (subject, files) in commits {
            for (path, bytes) in files {
                let full = self.repo.join(path);
                fs::create_dir_all(full.parent().expect("a parent")).expect("a directory");
                fs::write(&full, bytes).expect("a file");
                git(&self.repo, &["add", path]);
            }
            git(&self.repo, &["commit", "-q", "-m", subject]);
        }
        git(&self.repo, &["rev-parse", "HEAD"])
    }
}

/// Every record in the story's evidence directory at `revision`: `(path, bytes, json)`.
fn evidence_at(repo: &Path, revision: &str) -> Vec<(String, Vec<u8>, serde_json::Value)> {
    git(
        repo,
        &["ls-tree", "--name-only", &format!("{revision}:{EVIDENCE}")],
    )
    .lines()
    .map(|name| {
        let path = format!("{EVIDENCE}/{name}");
        let bytes = git_bytes(repo, &["show", &format!("{revision}:{path}")]);
        let json = serde_json::from_slice(&bytes).expect("a json record");
        (path, bytes, json)
    })
    .collect()
}

fn test_result_at(repo: &Path, revision: &str) -> (String, Vec<u8>) {
    let found: Vec<(String, Vec<u8>)> = evidence_at(repo, revision)
        .into_iter()
        .filter(|(_, _, json)| json["change"]["kind"] == "test_result")
        .map(|(path, bytes, _)| (path, bytes))
        .collect();
    assert_eq!(found.len(), 1, "one test_result record: {found:?}");
    found.into_iter().next().expect("one")
}

/// The acceptance: the base holds the story at `implemented` *with its `test_result`*. The move
/// is takt's own bytes, but it was made on top of the `test_result` commit; a head carrying the
/// move without the `test_result` puts an `implemented` story on the base with no `test_result`
/// evidence, a state takt never wrote, and the rule lets it ride.
#[test]
fn a_move_without_its_test_result_rides() {
    let case = approved("move-alone");
    let (stop, text) = case.run_with(&NoHarness);
    assert_eq!(stop, Stop::Completed("accepted".to_owned()), "{text}");
    let merged = case.pushed();
    let story = git_bytes(&case.repo, &["show", &format!("{merged}:{STORY_FILE}")]);
    let head = case.commits(&case.approved, &[(IMPLEMENTED, vec![(STORY_FILE, story)])]);
    assert!(
        evidence_at(&case.repo, &head)
            .iter()
            .all(|(_, _, json)| json["change"]["kind"] != "test_result"),
        "the head holds no test_result"
    );
    assert!(
        !case.rides(&head),
        "a move to implemented with no test_result below it rides on {}",
        case.granted
    );
}

/// The kept record is compared by its bytes only, never by where it lands: the `test_result`
/// takt committed at one path rides when a commit adds the same bytes under any other name in
/// the story's evidence directory.
#[test]
fn a_test_result_under_another_name_rides() {
    let case = approved("renamed");
    let (stop, text) = case.run_with(&NoHarness);
    assert_eq!(stop, Stop::Completed("accepted".to_owned()), "{text}");
    let merged = case.pushed();
    let (path, bytes) = test_result_at(&case.repo, &merged);
    let other = format!("{EVIDENCE}/not-the-name-takt-wrote.json");
    assert_ne!(path, other);
    let head = case.commits(
        &case.approved,
        &[(TEST_RESULT, vec![(other.as_str(), bytes)])],
    );
    assert!(
        !case.rides(&head),
        "the test_result written at {path} rides at {other}"
    );
}

/// Write order across revisions: the records are written before the merge at the granted
/// revision R1; the merge waits on the checks of the riders' head; the operator commits a fix on
/// the case's branch (a commit between runs moves the revision, `CaseDir::resume`), the case
/// reaches the gate at R2 and is granted there. The R1 records stay in the history below R2 (no
/// force push removes them); the base merged at R2 must also hold a `test_result` about R2,
/// written for R2 and riding above it, and nothing written for R1 may ride above R2.
#[test]
fn records_of_an_earlier_revision_reach_the_base_with_a_later_grant() {
    let case = approved("stale");
    let first = case.granted.clone();
    case.forge.checks.set(Checks::Pending);
    let (stop, text) = case.run_with(&NoHarness);
    assert!(
        matches!(stop, Stop::Waiting { .. }),
        "the merge waits on the riders' checks: {stop:?}\n{text}"
    );
    assert!(case.forge.memory.merged.borrow().is_empty(), "{text}");
    assert!(
        git(
            &case.repo,
            &["log", "--format=%s", &format!("{first}..HEAD")]
        )
        .contains(TEST_RESULT),
        "the records were written before the merge: {text}"
    );

    fs::write(case.repo.join("check.txt"), "fixed again\n").expect("a file");
    git(&case.repo, &["commit", "-q", "-am", "the fix"]);
    let second = git(&case.repo, &["rev-parse", "HEAD"]);
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
    let merged = case.pushed();
    assert_eq!(case.forge.memory.merged.borrow().as_slice(), [1], "{text}");

    // The revision each `test_result` record in `records` is about, read from the case's
    // observation it references.
    let about = |records: Vec<(String, Vec<u8>, serde_json::Value)>| -> Vec<String> {
        records
            .into_iter()
            .filter(|(_, _, json)| json["change"]["kind"] == "test_result")
            .map(|(_, _, json)| {
                let reference = json["change"]["reference"].as_str().expect("a reference");
                let observed: serde_json::Value = serde_json::from_slice(
                    &fs::read(case.repo.join(reference)).expect("the case's record"),
                )
                .expect("json");
                observed["subject_revision"]
                    .as_str()
                    .expect("a subject revision")
                    .to_owned()
            })
            .collect()
    };
    // What rides above R2: the records the commits in `R2..merged` add to the evidence.
    let added = git(
        &case.repo,
        &[
            "diff",
            "--name-only",
            "--diff-filter=A",
            &second,
            &merged,
            "--",
            EVIDENCE,
        ],
    );
    let riding: Vec<(String, Vec<u8>, serde_json::Value)> = added
        .lines()
        .map(|path| {
            let bytes = git_bytes(&case.repo, &["show", &format!("{merged}:{path}")]);
            let json = serde_json::from_slice(&bytes).expect("a json record");
            (path.to_owned(), bytes, json)
        })
        .collect();
    let above = about(riding);
    let on_base = about(evidence_at(&case.repo, &merged));
    assert!(
        on_base.contains(&second),
        "the base merged at {second} holds no test_result about it, only about {on_base:?}\n{text}"
    );
    assert!(
        above.contains(&second),
        "no test_result written for {second} rides above it; riding: {above:?}\n{text}"
    );
    assert!(
        !above.contains(&first),
        "a test_result written for {first} rides above {second}: {above:?}\n{text}"
    );
}

/// The commit says a story-case approver the store does not accept as `human:<name>` or
/// `agent:<role>` is refused at approval with nothing recorded, and the refusal tells the
/// operator to say `human:<name>` or `agent:<role>`. `system` and `service:<name>` pass, and the
/// store records the operator's approval as the store's own `system` actor.
#[test]
fn an_approver_that_is_neither_human_nor_agent_is_not_refused() {
    let case = gated("system-approver");
    let mut accepted = Vec::new();
    for approver in ["system", "service:ci"] {
        let head = git(&case.repo, &["rev-parse", "HEAD"]);
        if approve(
            &case.repo,
            &Aep::default(),
            &case.id,
            "repository.merge",
            approver,
        )
        .is_ok()
        {
            accepted.push(approver);
            git(&case.repo, &["reset", "-q", "--hard", &head]);
            let _ = fs::remove_file(case.dir.join("authority.yaml"));
        }
    }
    assert!(
        accepted.is_empty(),
        "approved by {accepted:?}, neither human:<name> nor agent:<role>"
    );
}
