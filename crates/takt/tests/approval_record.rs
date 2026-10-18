//! The approval's record reaches the base's planning store with the pull request. `takt
//! approve` commits its record on the case's branch after the revision the grant authorises
//! (`takt: approve <story> <capability>`, store paths only). The pull request carries those
//! commits: `pr.open` pushes the branch's `HEAD` over approve-only commits, and
//! `repository.merge` brings the pull request's head up to them before it merges, merging a
//! head that is the granted revision plus approve-only commits and refusing any other. The grant
//! still binds the implementation revision; the merge observation names it, and what it
//! retains names the head merged.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

use anyhow::anyhow;
use takt::case::{BRANCH_FILE, Branch, CASES_DIR};
use takt::evidence::Record;
use takt::forge::{Checks, Forge, Memory};
use takt::frontier::{Aep, approve};
use takt::harness::{Harness, StepContext, StepReport};
use takt::validator::{CiRead, PrMerged, PrOpen, Validator};
use takt::{Run, RunOptions, Stop};

/// The delivery protocol, relative to this crate.
const PROTOCOL: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../protocols/story-delivery/1.yaml"
);

/// The fixture stores, one directory each, relative to this crate.
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");

/// Where the store keeps the evidence on `story:ready-one`.
const EVIDENCE: &str = ".engineering/evidence/story/ready-one";

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

/// A scratch directory under this crate's build directory. Dropping it removes it; a removal
/// that fails fails the case, unless the case is already failing.
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
        .join(format!("approval-record-{}-{name}", std::process::id()));
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

/// A harness that requests the merge gate as soon as it is offered, changing nothing.
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

/// A harness the resumed run never reaches: every step it takes is a validator's.
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

/// The three forge validators over `forge`, in the order the binary registers them.
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
        "approval-record-test",
    )
}

/// The forge double, its checks passing.
fn double() -> Rc<Memory> {
    Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    })
}

/// `story:ready-one` taken from the store, its work committed on the case's branch, the tests
/// and the gate green there, and one run through the forge double's pull request and checks to
/// the gate: the case id and the revision the run stopped at, the one a grant authorises.
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
    assert_eq!(
        run.case.revision("implementation").as_deref(),
        Some(worked.as_str())
    );
    run.case
        .add_evidence(&record(1, "test_result", &worked))
        .expect("the record is written");
    run.case
        .add_evidence(&record(2, "gate_result", &worked))
        .expect("the record is written");
    let mut out = Vec::new();
    let stop = run.run(&mut out).expect("the run stops without an error");
    let text = String::from_utf8_lossy(&out).into_owned();
    assert_eq!(
        stop,
        Stop::ApprovalRequired(vec!["repository.merge".to_owned()]),
        "{text}"
    );
    (id, worked)
}

/// The paths under the story's evidence directory in the tree object `tree_ish`.
fn evidence_in(repo: &Path, tree_ish: &str) -> Vec<String> {
    git(
        repo,
        &["ls-tree", "-r", "--name-only", tree_ish, "--", EVIDENCE],
    )
    .lines()
    .filter(|line| !line.is_empty())
    .map(str::to_owned)
    .collect()
}

/// The base as a merge of `head` into `main` leaves it: the tree the merge writes, read
/// without a checkout. The forge double records a merge and moves no branch, so this is the
/// base's tree after it.
fn base_after_merging(repo: &Path, head: &str) -> String {
    git(repo, &["merge-tree", "--write-tree", "main", head])
        .lines()
        .next()
        .expect("a tree")
        .to_owned()
}

/// The records under the case's `evidence/` of `kind`, as JSON.
fn case_records(case_dir: &Path, kind: &str) -> Vec<serde_json::Value> {
    let Ok(entries) = fs::read_dir(case_dir.join("evidence")) else {
        return Vec::new();
    };
    let mut paths: Vec<PathBuf> = entries.map(|e| e.expect("an entry").path()).collect();
    paths.sort();
    paths
        .iter()
        .map(|path| {
            serde_json::from_str::<serde_json::Value>(&fs::read_to_string(path).expect("a record"))
                .expect("json")
        })
        .filter(|record| record["kind"] == kind)
        .collect()
}

/// The uncommitted paths of the tree outside the cases directory.
fn uncommitted(tree: &Path) -> Vec<String> {
    git(tree, &["status", "--porcelain", "--untracked-files=all"])
        .lines()
        .filter(|line| !line.contains(CASES_DIR))
        .map(str::to_owned)
        .collect()
}

/// The story's acceptance: with the forge double, a case approved, resumed and merged leaves
/// the story's approval evidence (approver, capability, revision) in the base's planning store
/// as the merge leaves it, because the head merged carries the approval's commit above the
/// granted revision. The merge observation names the granted revision, and what it retains
/// names the head merged. The tree is left as the approval left it.
#[test]
fn a_case_approved_and_merged_brings_its_approval_record_to_the_base() {
    let repo = repository("merged");
    let memory = double();
    let (id, granted) = at_the_gate(&repo, &memory);
    let branch = format!("takt/{id}");
    let dir = repo.join(CASES_DIR).join(&id);
    assert!(evidence_in(&repo, &base_after_merging(&repo, &granted)).is_empty());

    let approval = approve(&repo, &Aep::default(), &id, "repository.merge", "operator")
        .expect("the approval is given");
    assert_eq!(approval.revision, granted);
    let approved = git(&repo, &["rev-parse", "HEAD"]);
    assert_eq!(git(&repo, &["rev-parse", "HEAD^"]), granted);

    let forge: Rc<dyn Forge> = memory.clone();
    let validators = validators(forge);
    let mut run =
        Run::resume(&repo, options(), &id, &NoHarness, &validators).expect("the case resumes");
    assert_eq!(
        run.case.revision("implementation").as_deref(),
        Some(granted.as_str())
    );
    let mut out = Vec::new();
    let stop = run.run(&mut out).expect("the run stops without an error");
    let text = String::from_utf8_lossy(&out).into_owned();
    assert_eq!(stop, Stop::Completed("accepted".to_owned()), "{text}");
    drop(run);

    // Merged once, and the head merged is the approval's commit on the granted revision: the
    // double merges only the head last pushed for the branch.
    assert_eq!(memory.merged.borrow().as_slice(), [1], "{text}");
    let merged = memory
        .pushed
        .borrow()
        .get(&branch)
        .cloned()
        .expect("the branch was pushed");
    assert_eq!(merged, approved, "{text}");
    assert_eq!(
        Branch::read(&dir).expect("reads").pushed.as_deref(),
        Some(approved.as_str())
    );

    // The base, as the merge leaves it, holds the approval's record.
    let base = base_after_merging(&repo, &merged);
    let on_base = evidence_in(&repo, &base);
    assert_eq!(on_base.len(), 1, "{on_base:?} {text}");
    let recorded: serde_json::Value =
        serde_json::from_str(&git(&repo, &["show", &format!("{base}:{}", on_base[0])]))
            .expect("the record is json");
    assert_eq!(recorded["artifact"], "story:ready-one", "{recorded}");
    assert_eq!(recorded["change"]["kind"], "approval", "{recorded}");
    assert_eq!(
        recorded["change"]["source"],
        format!("takt approve repository.merge --by operator at {granted}").as_str(),
        "{recorded}"
    );

    // The merge observation is about the granted revision; what it retains names the head.
    let observed = case_records(&dir, "merge_observation");
    assert_eq!(observed.len(), 1, "{text}");
    assert_eq!(observed[0]["result"], "merged");
    assert_eq!(observed[0]["subject_revision"], granted.as_str());
    let retained: Vec<serde_json::Value> = fs::read_dir(dir.join("artifacts"))
        .expect("artifacts")
        .map(|e| e.expect("an entry").path())
        .filter(|path| {
            path.file_name()
                .is_some_and(|name| name.to_string_lossy().starts_with("merged-head-"))
        })
        .map(|path| serde_json::from_str(&fs::read_to_string(path).expect("a file")).expect("json"))
        .collect();
    assert_eq!(retained.len(), 1, "{text}");
    assert_eq!(retained[0]["granted"], granted.as_str(), "{}", retained[0]);
    assert_eq!(
        retained[0]["merged_head"],
        approved.as_str(),
        "{}",
        retained[0]
    );
    assert!(text.contains(&approved[..8]), "{text}");

    // The tree is where the approval left it.
    assert_eq!(git(&repo, &["branch", "--show-current"]), branch);
    assert_eq!(git(&repo, &["rev-parse", "HEAD"]), approved);
    assert_eq!(uncommitted(&repo), Vec::<String>::new());
}

/// A pull request whose head carries any commit above the granted revision that is not an
/// approval's own (here one changing a file outside the store, on top of the approval's commit)
/// is not merged: no merge, no record, and the head the forge has is not replaced by a push.
#[test]
fn a_head_carrying_a_commit_that_is_no_approvals_is_refused_with_nothing_merged() {
    let repo = repository("foreign");
    let memory = double();
    let (id, _) = at_the_gate(&repo, &memory);
    let branch = format!("takt/{id}");
    let dir = repo.join(CASES_DIR).join(&id);
    approve(&repo, &Aep::default(), &id, "repository.merge", "operator")
        .expect("the approval is given");

    // A commit beyond the approval's on another branch, and the forge's head moved to it, as a
    // push to the pull request's branch by someone else leaves it.
    git(&repo, &["checkout", "-q", "-b", "elsewhere"]);
    fs::write(repo.join("check.txt"), "changed after the grant\n").expect("a file");
    git(&repo, &["commit", "-q", "-am", "after the grant"]);
    let foreign = git(&repo, &["rev-parse", "HEAD"]);
    git(&repo, &["checkout", "-q", &branch]);
    memory
        .pushed
        .borrow_mut()
        .insert(branch.clone(), foreign.clone());

    let forge: Rc<dyn Forge> = memory.clone();
    let validators = validators(forge);
    let mut run =
        Run::resume(&repo, options(), &id, &NoHarness, &validators).expect("the case resumes");
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let text = String::from_utf8_lossy(&out).into_owned();
    drop(run);
    assert!(memory.merged.borrow().is_empty(), "{stop:?} {text}");
    assert!(
        !memory
            .calls
            .borrow()
            .iter()
            .any(|call| call.starts_with("merge")),
        "{:?}",
        memory.calls.borrow()
    );
    assert_eq!(
        memory.pushed.borrow().get(&branch),
        Some(&foreign),
        "the forge's head is not replaced: {text}"
    );
    assert!(case_records(&dir, "merge_observation").is_empty(), "{text}");
}

/// `pr.open` on a case whose branch holds the approval's own commits above the implementation
/// revision pushes the branch's `HEAD`, so the pull request's head carries the record; the
/// record it writes is still about the revision, and `ci.read` reads the checks on that head
/// as the revision's.
#[test]
fn pr_open_pushes_the_approvals_commits_with_the_revision() {
    let repo = repository("open");
    let memory = double();
    let (id, granted) = at_the_gate(&repo, &memory);
    let branch = format!("takt/{id}");
    let dir = repo.join(CASES_DIR).join(&id);
    approve(&repo, &Aep::default(), &id, "repository.merge", "operator")
        .expect("the approval is given");
    let approved = git(&repo, &["rev-parse", "HEAD"]);

    let forge: Rc<dyn Forge> = memory.clone();
    let opened = PrOpen::new(forge.clone())
        .observe(&repo, &granted, 90)
        .expect("pr.open observes");
    assert_eq!(
        memory.pushed.borrow().get(&branch),
        Some(&approved),
        "{}",
        opened.summary
    );
    let pushed = Branch::read(&dir).expect("reads").pushed;
    assert_eq!(pushed.as_deref(), Some(approved.as_str()));
    assert_eq!(opened.records.len(), 1, "{}", opened.summary);
    assert_eq!(opened.records[0].subject_revision, granted);
    assert_eq!(opened.records[0].result.as_deref(), Some("open"));

    let read = CiRead::new(forge)
        .observe(&repo, &granted, 91)
        .expect("ci.read observes");
    assert_eq!(read.records.len(), 1, "{}", read.summary);
    assert_eq!(read.records[0].subject_revision, granted);
    assert_eq!(read.records[0].result.as_deref(), Some("pass"));
    assert!(dir.join(BRANCH_FILE).is_file());
}
