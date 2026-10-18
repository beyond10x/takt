//! Only approval records of granted capabilities ride on a granted revision. A commit above the
//! revision a grant was given at rides with it to the base only when every file it adds is the
//! planning store's approval record for the case's story, naming a capability and a grantor the
//! case's `authority.yaml` holds as granted at that revision, one record per grant. A
//! `test_result`, a record of another kind, story, capability, grantor or revision, or a grant
//! recorded twice is no approval's: the case moves to it, and the push and the merge take nothing
//! above the granted revision. The records are read from the commit, never from the tree.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

use anyhow::anyhow;
use takt::case::{Branch, CASES_DIR, CaseDir, approvals_only};
use takt::evidence::Record;
use takt::forge::{Checks, Forge, Memory};
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

/// The approval's subject for `story:ready-one` and `repository.merge`.
const SUBJECT: &str = "takt: approve story:ready-one repository.merge";

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

/// A scratch directory under this crate's build directory, removed when dropped.
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

/// A scratch repository on `main` holding a committed copy of the `one-ready` store.
fn repository(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("approval-riders-{}-{name}", std::process::id()));
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

/// Requests the merge gate as soon as it is offered, changing nothing.
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

/// Inspects and changes nothing.
struct Idle;

impl Harness for Idle {
    fn name(&self) -> &str {
        "idle"
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
        "approval-riders-test",
    )
}

fn double() -> Rc<Memory> {
    Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    })
}

/// `story:ready-one` taken, its work committed, the tests and the gate green, and one run to the
/// gate: the case id and the revision a grant authorises.
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

/// A case approved through the store (`takt approve`, the real `aep`): the repository, the case
/// id, its directory, the granted revision, the approval's own commit and the record that commit
/// added, as the store wrote it.
struct Approved {
    repo: Scratch,
    id: String,
    dir: PathBuf,
    granted: String,
    approved: String,
    record: serde_json::Value,
}

fn approved(name: &str, memory: &Rc<Memory>) -> Approved {
    let repo = repository(name);
    let (id, granted) = at_the_gate(&repo, memory);
    approve(
        &repo,
        &Aep::default(),
        &id,
        "repository.merge",
        "human:operator",
    )
    .expect("the approval is given");
    let approved = git(&repo, &["rev-parse", "HEAD"]);
    let added = git(&repo, &["show", "--name-only", "--format=", "HEAD"]);
    assert!(
        added.starts_with(EVIDENCE) && added.lines().count() == 1,
        "{added}"
    );
    let record: serde_json::Value =
        serde_json::from_str(&git(&repo, &["show", &format!("HEAD:{added}")]))
            .expect("the store's record is json");
    assert_eq!(record["change"]["kind"], "approval", "{record}");
    let dir = repo.join(CASES_DIR).join(&id);
    Approved {
        repo,
        id,
        dir,
        granted,
        approved,
        record,
    }
}

impl Approved {
    /// The branch put back at the granted revision, then one commit with the subject `subject`
    /// adding each `(name, text)` directly in the story's evidence: the commit.
    fn rider(&self, subject: &str, files: &[(&str, String)]) -> String {
        git(&self.repo, &["reset", "-q", "--hard", &self.granted]);
        for (name, text) in files {
            let path = format!("{EVIDENCE}/{name}");
            fs::create_dir_all(self.repo.join(EVIDENCE)).expect("a directory");
            fs::write(self.repo.join(&path), text).expect("a file");
            git(&self.repo, &["add", &path]);
        }
        git(&self.repo, &["commit", "-q", "-m", subject]);
        git(&self.repo, &["rev-parse", "HEAD"])
    }

    /// Whether `head` rides on the granted revision.
    fn rides(&self, head: &str) -> bool {
        approvals_only(&self.repo, &self.granted, head, &self.dir).expect("approvals_only reads")
    }

    /// The store's record with `edit` applied, as text.
    fn edited(&self, edit: impl FnOnce(&mut serde_json::Value)) -> String {
        let mut record = self.record.clone();
        edit(&mut record);
        serde_json::to_string_pretty(&record).expect("json")
    }

    /// The store's record, as text.
    fn real(&self) -> String {
        self.edited(|_| {})
    }
}

/// A `test_result` record in the store's format on `story:ready-one`.
fn test_result_record() -> String {
    serde_json::to_string_pretty(&serde_json::json!({
        "at": "2026-10-18T13:04:29Z",
        "actor": "agent:takt",
        "artifact": "story:ready-one",
        "kind": "story",
        "revision": 4,
        "change": {
            "change": "evidence",
            "kind": "test_result",
            "source": "cargo test --workspace",
            "reference": "all passed"
        }
    }))
    .expect("json")
}

/// The story's Expected before: a commit with the approval's subject on the granted revision
/// that adds a `test_result` record to the story's evidence is no approval's, and the resumed
/// case moves to it.
#[test]
fn a_test_result_under_the_approve_subject_does_not_ride() {
    let memory = double();
    let case = approved("test-result", &memory);
    let head = case.rider(SUBJECT, &[("x.json", test_result_record())]);
    assert!(
        !case.rides(&head),
        "a test_result rides on {}",
        case.granted
    );
    let resumed = CaseDir::resume(&case.repo, &case.id).expect("the case resumes");
    assert_eq!(
        resumed.revision("implementation").as_deref(),
        Some(head.as_str())
    );
}

/// The store's own record of the grant rides, under any name, in place of the approval's own
/// commit; every record that differs from it in what it says about the grant does not.
#[test]
fn only_the_record_of_a_granted_capability_by_its_grantor_rides() {
    let memory = double();
    let case = approved("variants", &memory);
    assert!(case.rides(&case.approved), "the approval's own commit");
    let head = case.rider(SUBJECT, &[("renamed.json", case.real())]);
    assert!(case.rides(&head), "the store's record under another name");

    let source = case.record["change"]["source"]
        .as_str()
        .expect("a source")
        .to_owned();
    let variants: Vec<(&str, String)> = vec![
        (
            "a capability not granted",
            case.edited(|r| {
                r["change"]["source"] =
                    source.replace("repository.merge", "repository.push").into();
            }),
        ),
        (
            "another grantor",
            case.edited(|r| {
                r["change"]["source"] = source
                    .replace("--by human:operator", "--by intruder")
                    .into();
            }),
        ),
        (
            "another revision",
            case.edited(|r| {
                r["change"]["source"] = source
                    .replace(&case.granted, &"0".repeat(case.granted.len()))
                    .into();
            }),
        ),
        (
            "another story",
            case.edited(|r| r["artifact"] = "story:blocked-one".into()),
        ),
        (
            "another case's grant",
            case.edited(|r| r["change"]["reference"] = "elsewhere/authority.yaml".into()),
        ),
        (
            "another instant than the grant's",
            case.edited(|r| r["at"] = "1970-01-01T00:00:00Z".into()),
        ),
        (
            "a review outcome",
            case.edited(|r| r["change"]["kind"] = "review_outcome".into()),
        ),
        (
            "a verification",
            case.edited(|r| r["change"]["kind"] = "verification".into()),
        ),
        (
            "a record of no evidence",
            case.edited(|r| r["change"]["change"] = "move".into()),
        ),
        (
            "another kind of artifact",
            case.edited(|r| r["kind"] = "epic".into()),
        ),
        (
            "another revision of the story",
            case.edited(|r| {
                r["revision"] = (r["revision"].as_u64().expect("a revision") + 1).into();
            }),
        ),
        (
            "an actor the store does not write",
            case.edited(|r| r["actor"] = "bot:x".into()),
        ),
        (
            "an actor name the store does not write",
            case.edited(|r| r["actor"] = "human:a b".into()),
        ),
        (
            "no actor",
            case.edited(|r| {
                r.as_object_mut().expect("an object").remove("actor");
            }),
        ),
        (
            "no reference",
            case.edited(|r| {
                r["change"]
                    .as_object_mut()
                    .expect("an object")
                    .remove("reference");
            }),
        ),
        (
            "a field the store does not write on an approval",
            case.edited(|r| r["note"] = "more".into()),
        ),
        (
            "a field the store does not write on an approval's change",
            case.edited(|r| r["change"]["note"] = "more".into()),
        ),
        ("no record at all", "{}\n".to_owned()),
        ("not json", "kind: approval\n".to_owned()),
    ];
    for (what, text) in variants {
        let head = case.rider(SUBJECT, &[("x.json", text)]);
        assert!(!case.rides(&head), "{what} rides on {}", case.granted);
    }

    // The record of the grant under a subject naming another capability is not that
    // capability's approval.
    let head = case.rider(
        "takt: approve story:ready-one repository.push",
        &[("x.json", case.real())],
    );
    assert!(!case.rides(&head), "a subject naming another capability");
}

/// One record per grant: the store's record of the one grant, twice in one commit, or once in
/// each of two commits, is a grant recorded twice and does not ride.
#[test]
fn a_grant_recorded_twice_does_not_ride() {
    let memory = double();
    let case = approved("twice", &memory);
    let head = case.rider(SUBJECT, &[("a.json", case.real()), ("b.json", case.real())]);
    assert!(!case.rides(&head), "two records in one commit");

    git(&case.repo, &["reset", "-q", "--hard", &case.approved]);
    let again = format!("{EVIDENCE}/again.json");
    fs::write(case.repo.join(&again), case.real()).expect("a file");
    git(&case.repo, &["add", &again]);
    git(&case.repo, &["commit", "-q", "-m", SUBJECT]);
    let head = git(&case.repo, &["rev-parse", "HEAD"]);
    assert!(!case.rides(&head), "a second commit with the same record");
}

/// The record is read from the commit: a `test_result` committed under the approval's subject
/// does not ride because the tree holds the store's record at that path.
#[test]
fn a_record_is_read_from_the_commit_not_the_tree() {
    let memory = double();
    let case = approved("from-commit", &memory);
    let head = case.rider(SUBJECT, &[("x.json", test_result_record())]);
    fs::write(case.repo.join(EVIDENCE).join("x.json"), case.real()).expect("a file");
    assert!(!case.rides(&head), "the tree's text was read");
}

/// The merge merges nothing when the pull request's head, stored and reported, is the granted
/// revision with a `test_result` under the approval's subject above it: no merge, no merge
/// observation.
#[test]
fn a_head_with_a_test_result_rider_is_not_merged() {
    let memory = double();
    let case = approved("merge", &memory);
    let branch = format!("takt/{}", case.id);
    git(
        &case.repo,
        &["checkout", "-q", "-b", "elsewhere", &case.granted],
    );
    let path = format!("{EVIDENCE}/x.json");
    fs::create_dir_all(case.repo.join(EVIDENCE)).expect("a directory");
    fs::write(case.repo.join(&path), test_result_record()).expect("a file");
    git(&case.repo, &["add", &path]);
    git(&case.repo, &["commit", "-q", "-m", SUBJECT]);
    let forged = git(&case.repo, &["rev-parse", "HEAD"]);
    git(&case.repo, &["checkout", "-q", &branch]);
    memory
        .pushed
        .borrow_mut()
        .insert(branch.clone(), forged.clone());
    let mut stored = Branch::read(&case.dir).expect("reads");
    stored.pushed = Some(forged.clone());
    stored.write(&case.dir).expect("writes");

    let forge: Rc<dyn Forge> = memory.clone();
    let validators = validators(forge);
    let mut run =
        Run::resume(&case.repo, options(), &case.id, &Idle, &validators).expect("the case resumes");
    run.poll = std::time::Duration::from_millis(5);
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let text = String::from_utf8_lossy(&out).into_owned();
    drop(run);
    assert!(
        memory.merged.borrow().is_empty(),
        "{} merged: {stop:?}\n{text}",
        &forged[..8]
    );
    let observed = fs::read_dir(case.dir.join("evidence"))
        .expect("evidence")
        .map(|entry| fs::read_to_string(entry.expect("an entry").path()).expect("a record"))
        .filter(|text| text.contains("merge_observation"))
        .count();
    assert_eq!(observed, 0, "{text}");
}
