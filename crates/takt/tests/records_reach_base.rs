//! story:records-reach-base: takt's records for a story reach the base with its merge. Above the
//! revision a grant authorises, the pull request carries the approval's record, the story's
//! `test_result` record and its move to `implemented`, each file byte-equal to the record takt
//! kept in the case directory when it wrote it, the move last; the head merged is the base's.
//! A riding commit with any file that is not byte-equal to such a record, a move that is not
//! last, or a second move, rides on nothing: the case moves to it and nothing is merged. The
//! rule is the one `CaseDir::resume` reads, so an accepted case resumed by id stays at its
//! granted revision and nothing becomes due again.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

use anyhow::anyhow;
use takt::case::{Authority, AuthorityDecision, CASES_DIR, CaseDir, Grant, approvals_only};
use takt::evidence::Record;
use takt::forge::{Checks, Forge, Memory};
use takt::frontier::{Aep, approve};
use takt::harness::{Harness, StepContext, StepReport};
use takt::validator::{ChangeRead, CiRead, Observation, PrMerged, PrOpen, TestsRun, Validator};
use takt::{Run, RunOptions, Stop};

/// The delivery protocol, the engineering protocols' built-in.
const DELIVERY: &str = "story.delivery/1";

/// The fixture stores, one directory each, relative to this crate.
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");

/// The story the fixture store holds ready.
const STORY: &str = "story:ready-one";

/// The story's file in the planning store.
const STORY_FILE: &str = ".engineering/planning/story/ready-one.md";

/// Where the store keeps the evidence on the story.
const EVIDENCE: &str = ".engineering/evidence/story/ready-one";

/// Who grants the merge: an actor the store records.
const APPROVER: &str = "human:operator";

/// The directory under the case directory that holds what takt's store writes for the case
/// committed, by the revision the case was at.
const KEPT: &str = "riders";

/// The subjects of takt's commits for the story.
const APPROVE: &str = "takt: approve story:ready-one repository.merge";
const TEST_RESULT: &str = "takt: test_result story:ready-one";
const IMPLEMENTED: &str = "takt: implemented story:ready-one";

/// One commit to make: its subject and each `(path, bytes)` it writes.
type Commit<'a> = (&'a str, Vec<(&'a str, Vec<u8>)>);

/// Runs `git <args>` in `tree`; its standard output, trimmed.
fn git(tree: &Path, args: &[&str]) -> String {
    String::from_utf8(git_bytes(tree, args))
        .expect("utf-8")
        .trim()
        .to_owned()
}

/// Runs `git <args>` in `tree`; its standard output, byte for byte.
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

/// A scratch tree under this crate's build directory, removed when dropped.
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

/// A scratch repository on `main` holding a committed copy of the `one-ready` store and a
/// `check.txt` the story's work changes.
fn repository(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("records-reach-base-{}-{name}", std::process::id()));
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

/// A harness the runs past the gate never reach: every step they take is a validator's.
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

/// The validators of a delivery run through `forge`, in the order the binary registers them:
/// the real tests validator, its command `true`, `change.read`, a gate that passes, and the
/// forge's three.
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

/// A case of `story:ready-one` taken, worked, run to the gate through a forge double whose
/// checks pass, and granted the merge through the store by [`APPROVER`].
struct Approved {
    repo: Scratch,
    memory: Rc<Memory>,
    id: String,
    dir: PathBuf,
    /// The revision the grant authorises.
    granted: String,
    /// The approval's own commit above it.
    approved: String,
}

fn approved(name: &str) -> Approved {
    let repo = repository(name);
    let memory = Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    });
    let forge: Rc<dyn Forge> = memory.clone();
    let validators = delivery(forge);
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
    let mut run =
        Run::resume(&repo, options(), &id, &Requester, &validators).expect("the case resumes");
    run.record_on(&store);
    let mut out = Vec::new();
    let stop = run.run(&mut out).expect("the run stops without an error");
    drop(run);
    assert_eq!(
        stop,
        Stop::ApprovalRequired(vec!["repository.merge".to_owned()]),
        "{}",
        String::from_utf8_lossy(&out)
    );
    approve(&repo, &store, &id, "repository.merge", APPROVER).expect("the approval is given");
    let approved = git(&repo, &["rev-parse", "HEAD"]);
    Approved {
        repo,
        memory,
        id,
        dir,
        granted,
        approved,
    }
}

impl Approved {
    /// Runs the case once more, given the store, past the gate: the stop and what it printed.
    fn run_with(&self, harness: &dyn Harness) -> (Stop, String) {
        let forge: Rc<dyn Forge> = self.memory.clone();
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

    /// The case's branch.
    fn branch(&self) -> String {
        format!("takt/{}", self.id)
    }

    /// The head the forge holds for the case's branch: the one merged, once merged.
    fn pushed(&self) -> String {
        self.memory
            .pushed
            .borrow()
            .get(&self.branch())
            .cloned()
            .expect("the branch was pushed")
    }

    /// Whether `head` rides on the granted revision.
    fn rides(&self, head: &str) -> bool {
        approvals_only(&self.repo, &self.granted, head, &self.dir).expect("the rider rule reads")
    }

    /// The branch put back at `base`, then one commit per `(subject, files)`, each writing every
    /// `(path, bytes)` and committing exactly those paths: the last commit.
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

    /// The files the case directory keeps of takt's store writes at the granted revision.
    fn kept(&self) -> Vec<Vec<u8>> {
        let dir = self.dir.join(KEPT).join(&self.granted);
        let mut kept = Vec::new();
        for entry in fs::read_dir(&dir).unwrap_or_else(|error| {
            panic!(
                "the case keeps its store writes at {}: {error}",
                dir.display()
            )
        }) {
            kept.push(fs::read(entry.expect("an entry").path()).expect("a kept record"));
        }
        kept
    }
}

/// The paths a commit adds or changes against its parent.
fn changed(repo: &Path, commit: &str) -> Vec<String> {
    git(
        repo,
        &["diff-tree", "--no-commit-id", "--name-only", "-r", commit],
    )
    .lines()
    .map(str::to_owned)
    .collect()
}

/// The records in the story's evidence directory at `revision`, each `(path, bytes)`.
fn evidence_at(repo: &Path, revision: &str) -> Vec<(String, Vec<u8>)> {
    git(
        repo,
        &["ls-tree", "--name-only", &format!("{revision}:{EVIDENCE}")],
    )
    .lines()
    .map(|name| {
        let path = format!("{EVIDENCE}/{name}");
        let bytes = git_bytes(repo, &["show", &format!("{revision}:{path}")]);
        (path, bytes)
    })
    .collect()
}

/// The record among `records` whose evidence kind is `kind`: exactly one.
fn one_of(records: &[(String, Vec<u8>)], kind: &str) -> (String, Vec<u8>, serde_json::Value) {
    let found: Vec<(String, Vec<u8>, serde_json::Value)> = records
        .iter()
        .map(|(path, bytes)| {
            let record: serde_json::Value = serde_json::from_slice(bytes).expect("a json record");
            (path.clone(), bytes.clone(), record)
        })
        .filter(|(_, _, record)| record["change"]["kind"] == kind)
        .collect();
    assert_eq!(found.len(), 1, "one {kind} record: {found:?}");
    found.into_iter().next().expect("one")
}

/// The story's status in the text of its file.
fn status_in(text: &str) -> &str {
    text.lines()
        .find_map(|line| line.strip_prefix("status: "))
        .expect("a status")
}

/// The Expected before and the acceptance: a story taken to `Completed (accepted)` through a
/// forge leaves the base, the head merged, holding the story at `implemented` with its
/// `test_result` and approval records, the records and the moved story each byte-equal to the
/// record the case directory keeps of takt's write; the move is the last rider, by
/// `agent:takt`, and the approval's record names its approver as its actor.
#[test]
fn the_base_holds_the_story_implemented_with_its_records_after_the_merge() {
    let case = approved("acceptance");
    let (stop, text) = case.run_with(&NoHarness);
    assert_eq!(stop, Stop::Completed("accepted".to_owned()), "{text}");
    assert_eq!(case.memory.merged.borrow().as_slice(), [1], "{text}");
    let merged = case.pushed();

    let story = git_bytes(&case.repo, &["show", &format!("{merged}:{STORY_FILE}")]);
    let story_text = String::from_utf8(story.clone()).expect("utf-8");
    assert_eq!(status_in(&story_text), "implemented", "{text}");
    let records = evidence_at(&case.repo, &merged);
    let (_, test_result, _) = one_of(&records, "test_result");
    let (_, _, approval) = one_of(&records, "approval");
    assert_eq!(approval["actor"], APPROVER, "{approval}");

    let kept = case.kept();
    assert!(kept.contains(&test_result), "the test_result is kept");
    assert!(kept.contains(&story), "the moved story is kept");

    assert_eq!(
        git(
            &case.repo,
            &["log", "--format=%s", &format!("{}..{merged}", case.granted)]
        )
        .lines()
        .collect::<Vec<_>>(),
        [IMPLEMENTED, TEST_RESULT, APPROVE],
        "{text}"
    );
    assert_eq!(changed(&case.repo, &merged), [STORY_FILE]);
    let moved = story_text
        .lines()
        .find(|line| line.contains("to: \"implemented\""))
        .expect("the move's transition");
    assert!(moved.contains("actor: \"agent:takt\""), "{moved}");
    // Nothing is written after the merge: the branch is the head merged.
    assert_eq!(git(&case.repo, &["rev-parse", "HEAD"]), merged, "{text}");
}

/// The resume gap: an accepted case resumed by id stays at its granted revision, and a run on it
/// takes no step, pushes nothing and merges nothing again.
#[test]
fn an_accepted_case_resumed_by_id_stays_at_its_granted_revision() {
    let case = approved("resume");
    let (stop, text) = case.run_with(&NoHarness);
    assert_eq!(stop, Stop::Completed("accepted".to_owned()), "{text}");
    let head = git(&case.repo, &["rev-parse", "HEAD"]);
    let calls = case.memory.calls.borrow().len();

    let resumed = CaseDir::resume(&case.repo, &case.id).expect("the case resumes");
    assert_eq!(
        resumed.revision("implementation").as_deref(),
        Some(case.granted.as_str())
    );
    drop(resumed);
    let (stop, text) = case.run_with(&NoHarness);
    assert_eq!(stop, Stop::Completed("accepted".to_owned()), "{text}");
    assert!(!text.contains("step "), "{text}");
    assert_eq!(case.memory.calls.borrow().len(), calls, "{text}");
    assert_eq!(case.memory.merged.borrow().as_slice(), [1]);
    assert_eq!(git(&case.repo, &["rev-parse", "HEAD"]), head);
}

/// The rider rule: takt's own riders ride; a `test_result` that differs from the kept record by
/// one byte, a move that is not last, a second move, a rider with another file beside its
/// record, an approval whose actor is not its approver, and a record the case directory does
/// not keep ride on nothing.
#[test]
fn only_riders_byte_equal_to_the_kept_records_ride_and_the_move_last() {
    let case = approved("rule");
    let (stop, text) = case.run_with(&NoHarness);
    assert_eq!(stop, Stop::Completed("accepted".to_owned()), "{text}");
    let merged = case.pushed();
    assert!(case.rides(&merged), "takt's own riders");

    let records = evidence_at(&case.repo, &merged);
    let (test_path, test_bytes, _) = one_of(&records, "test_result");
    let (approval_path, approval_bytes, approval) = one_of(&records, "approval");
    let story = git_bytes(&case.repo, &["show", &format!("{merged}:{STORY_FILE}")]);
    let before = git_bytes(
        &case.repo,
        &["show", &format!("{}:{STORY_FILE}", case.approved)],
    );
    let test = |bytes: Vec<u8>| (TEST_RESULT, vec![(test_path.as_str(), bytes)]);
    let moved = |bytes: Vec<u8>| (IMPLEMENTED, vec![(STORY_FILE, bytes)]);

    let head = case.commits(
        &case.approved,
        &[test(test_bytes.clone()), moved(story.clone())],
    );
    assert!(case.rides(&head), "the same riders made again");
    let head = case.commits(&case.approved, &[test(test_bytes.clone())]);
    assert!(case.rides(&head), "the test_result alone");

    let mut longer = test_bytes.clone();
    longer.push(b'\n');
    let mut story_longer = story.clone();
    story_longer.push(b'\n');
    let mut actor = approval.clone();
    actor["actor"] = "human:intruder".into();
    let mut beside = test(test_bytes.clone());
    beside.1.push((
        ".engineering/evidence/story/ready-one/x.json",
        b"{}".to_vec(),
    ));
    let mut also = moved(story.clone());
    also.1.push(("check.txt", b"changed\n".to_vec()));
    let variants: Vec<(&str, &str, Vec<Commit<'_>>)> = vec![
        (
            "a test_result one byte longer",
            &case.approved,
            vec![test(longer)],
        ),
        (
            "a move below the test_result",
            &case.approved,
            vec![moved(story.clone()), test(test_bytes.clone())],
        ),
        (
            "a second move",
            &case.approved,
            vec![
                test(test_bytes.clone()),
                moved(story.clone()),
                moved(before.clone()),
                moved(story.clone()),
            ],
        ),
        (
            "a test_result with a file beside it",
            &case.approved,
            vec![beside],
        ),
        (
            "a move with a file beside it",
            &case.approved,
            vec![test(test_bytes.clone()), also],
        ),
        (
            "a moved story one byte longer",
            &case.approved,
            vec![test(test_bytes.clone()), moved(story_longer)],
        ),
        (
            "an approval whose actor is not its approver",
            &case.granted,
            vec![(
                APPROVE,
                vec![(
                    approval_path.as_str(),
                    serde_json::to_vec_pretty(&actor).expect("json"),
                )],
            )],
        ),
    ];
    for (what, base, commits) in variants {
        let head = case.commits(base, &commits);
        assert!(!case.rides(&head), "{what} rides on {}", case.granted);
    }
    let head = case.commits(
        &case.granted,
        &[(APPROVE, vec![(approval_path.as_str(), approval_bytes)])],
    );
    assert!(case.rides(&head), "the approval's record made again");

    let head = case.commits(&case.approved, &[test(test_bytes.clone())]);
    fs::remove_dir_all(case.dir.join(KEPT)).expect("the kept records go");
    assert!(!case.rides(&head), "a test_result the case does not keep");
}

/// One rule counts a grant, the one `Authority::grants_at` reads: the first decision on a
/// capability. A denial before a grant of the same capability grants nothing, so the record of
/// that grant rides on nothing.
#[test]
fn a_grant_behind_a_denial_of_its_capability_records_nothing_that_rides() {
    let case = approved("denied-first");
    assert!(case.rides(&case.approved), "the approval's own commit");
    let mut authority = Authority::read(&case.dir)
        .expect("reads")
        .expect("an authority");
    let granted = authority.decisions[0].clone();
    authority.decisions.insert(
        0,
        AuthorityDecision {
            decision: Grant::Denied,
            ..granted
        },
    );
    authority.write(&case.dir).expect("writes");
    assert!(!authority.grants_at("repository.merge", &case.granted));
    assert!(!case.rides(&case.approved), "a grant behind a denial");
}

/// A `test_result` under takt's subject that is no record takt kept, committed above the
/// approval: the case moves to it, and the run merges nothing.
#[test]
fn a_rider_takt_did_not_keep_makes_the_merge_merge_nothing() {
    let case = approved("not-kept");
    let record = serde_json::to_vec_pretty(&serde_json::json!({
        "at": "2026-10-18T13:04:29Z",
        "actor": "agent:takt",
        "artifact": STORY,
        "kind": "story",
        "revision": 4,
        "change": {
            "change": "evidence",
            "kind": "test_result",
            "source": "takt",
            "reference": format!("{CASES_DIR}/{}/evidence/0001-e1.json", case.id)
        }
    }))
    .expect("json");
    let path = format!("{EVIDENCE}/forged.json");
    let head = case.commits(
        &case.approved,
        &[(TEST_RESULT, vec![(path.as_str(), record)])],
    );
    assert!(!case.rides(&head));
    let (stop, text) = case.run_with(&Requester);
    assert!(case.memory.merged.borrow().is_empty(), "{stop:?}\n{text}");
    let resumed = CaseDir::resume(&case.repo, &case.id).expect("the case resumes");
    assert_eq!(
        resumed.revision("implementation").as_deref(),
        Some(head.as_str())
    );
}
