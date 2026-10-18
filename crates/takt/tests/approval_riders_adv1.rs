//! Adversary pass 1 on riding approval records. A rider reaches the base with the merge, and the
//! planning store there reads every field of it, not only the ones `approvals_only` compares. Each
//! case builds a case approved through the real store (`takt approve`, the real `aep`), then a
//! commit above the granted revision, and asks whether it rides.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

use anyhow::anyhow;
use takt::case::{Authority, CASES_DIR, CaseDir, Grant, approvals_only};
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

/// The largest record `approvals_only` reads, 64 KiB.
const CAP: usize = 64 << 10;

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

fn repository(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "approval-riders-adv1-{}-{name}",
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
        "approval-riders-adv1",
    )
}

fn at_the_gate(repo: &Path) -> (String, String) {
    let memory = Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    });
    let forge: Rc<dyn Forge> = memory;
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
    // A commit beyond the case's base at the revision, as `change.read` records it.
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

struct Approved {
    repo: Scratch,
    id: String,
    dir: PathBuf,
    granted: String,
    approved: String,
    record: serde_json::Value,
}

fn approved_by(name: &str, approver: &str) -> Approved {
    let repo = repository(name);
    let (id, granted) = at_the_gate(&repo);
    approve(&repo, &Aep::default(), &id, "repository.merge", approver)
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

fn approved(name: &str) -> Approved {
    approved_by(name, "operator")
}

impl Approved {
    /// The branch put back at the granted revision, then one commit with the approval's subject
    /// adding each `(name, text)` directly in the story's evidence: the commit.
    fn rider(&self, files: &[(&str, String)]) -> String {
        git(&self.repo, &["reset", "-q", "--hard", &self.granted]);
        for (name, text) in files {
            let path = format!("{EVIDENCE}/{name}");
            fs::create_dir_all(self.repo.join(EVIDENCE)).expect("a directory");
            fs::write(self.repo.join(&path), text).expect("a file");
            git(&self.repo, &["add", &path]);
        }
        git(&self.repo, &["commit", "-q", "-m", SUBJECT]);
        git(&self.repo, &["rev-parse", "HEAD"])
    }

    fn rides(&self, head: &str) -> bool {
        approvals_only(&self.repo, &self.granted, head, &self.dir).expect("approvals_only reads")
    }

    fn edited(&self, edit: impl FnOnce(&mut serde_json::Value)) -> String {
        let mut record = self.record.clone();
        edit(&mut record);
        serde_json::to_string_pretty(&record).expect("json")
    }

    fn compact(&self) -> String {
        serde_json::to_string(&self.record).expect("json")
    }

    fn resumed(&self) -> String {
        CaseDir::resume(&self.repo, &self.id)
            .expect("the case resumes")
            .revision("implementation")
            .expect("a revision")
    }
}

/// The store reads `review` and `outcome` off any evidence record, whatever its kind, as the
/// outcome of that review (`aep plan artifact validate` stops reporting the review as
/// unanswered, `review-value` tallies the outcome). `takt approve` never writes them on an
/// approval, and `aep plan artifact evidence` refuses `--review` on any kind but
/// `review_outcome`. A rider that is the store's record of the grant plus those two fields says
/// something about the plan no grant authorised, and must not ride.
#[test]
fn adv1_a_record_that_also_answers_a_review_does_not_ride() {
    let case = approved("review");
    let forged = case.edited(|r| {
        r["change"]["review"] = "review-result:approval-riders-adv1".into();
        r["change"]["outcome"] = "fixed".into();
    });
    let head = case.rider(&[("x.json", forged)]);
    assert!(
        !case.rides(&head),
        "an approval record that also records a review outcome rides on {}",
        case.granted
    );
    assert_eq!(case.resumed(), head);
}

/// Real approvals by grantors whose names carry runs of spaces, edge spaces or a combining mark
/// ride: the comparison must equal what the store wrote for them, not refuse the operator's own
/// grant.
#[test]
fn adv1_real_approvals_by_unusual_grantors_ride() {
    for (name, approver) in [
        ("spaces", "Jane  Doe"),
        ("edges", " lead "),
        ("combining", "Zoe\u{308} O\u{308}lm"),
    ] {
        let case = approved_by(name, approver);
        assert_eq!(
            case.record["change"]["source"],
            format!(
                "takt approve repository.merge --by {approver} at {}",
                case.granted
            ),
            "{approver:?}"
        );
        assert!(case.rides(&case.approved), "{approver:?} is refused");
        assert_eq!(case.resumed(), case.granted, "{approver:?}");
    }
}

/// At the cap the store's record, padded inside the object, still parses and rides; one byte
/// above it is refused, never read cut.
#[test]
fn adv1_a_record_at_the_cap_rides_and_one_byte_above_it_does_not() {
    let case = approved("cap");
    let compact = case.compact();
    let padded = |size: usize| {
        let text = format!("{{{}{}", " ".repeat(size - compact.len()), &compact[1..]);
        assert_eq!(text.len(), size);
        text
    };
    let at = case.rider(&[("x.json", padded(CAP))]);
    assert!(case.rides(&at), "a record of exactly {CAP} bytes");
    let above = case.rider(&[("x.json", padded(CAP + 1))]);
    assert!(!case.rides(&above), "a record of {} bytes", CAP + 1);
}

/// Look-alikes of the story, the grantor and the instant, and duplicate keys whichever value comes
/// last, are not the store's record of the grant.
#[test]
fn adv1_look_alikes_and_duplicate_keys_do_not_ride() {
    let case = approved("look-alikes");
    let source = case.record["change"]["source"]
        .as_str()
        .expect("a source")
        .to_owned();
    let at = case.record["at"].as_str().expect("an instant").to_owned();
    let compact = case.compact();
    assert!(compact.contains("\"kind\":\"approval\""), "{compact}");
    let variants: Vec<(&str, String)> = vec![
        (
            "another namespace, same name",
            case.edited(|r| r["artifact"] = "finding:ready-one".into()),
        ),
        (
            "a zero-width space in the story",
            case.edited(|r| r["artifact"] = "story:ready-one\u{200b}".into()),
        ),
        (
            "a capital in the story",
            case.edited(|r| r["artifact"] = "story:Ready-one".into()),
        ),
        (
            "a Cyrillic o in the grantor",
            case.edited(|r| {
                r["change"]["source"] = source
                    .replace("--by operator", "--by \u{43e}perator")
                    .into();
            }),
        ),
        (
            "the instant with milliseconds",
            case.edited(|r| r["at"] = at.replace('Z', ".000Z").into()),
        ),
        (
            "the instant with an offset",
            case.edited(|r| r["at"] = at.replace('Z', "+00:00").into()),
        ),
        (
            "a test_result kind before the approval kind",
            compact.replace(
                "\"kind\":\"approval\"",
                "\"kind\":\"test_result\",\"kind\":\"approval\"",
            ),
        ),
        (
            "a test_result kind after the approval kind",
            compact.replace(
                "\"kind\":\"approval\"",
                "\"kind\":\"approval\",\"kind\":\"test_result\"",
            ),
        ),
    ];
    for (what, text) in variants {
        let head = case.rider(&[("x.json", text)]);
        assert!(!case.rides(&head), "{what} rides on {}", case.granted);
    }
}

/// A symlink whose target text is the store's own record of the grant is refused for its mode
/// alone: the existing symlink case points at a path, which the text check refuses anyway. A
/// gitlink in the story's evidence is refused too.
#[test]
fn adv1_a_symlink_holding_the_record_and_a_gitlink_do_not_ride() {
    let case = approved("links");
    git(&case.repo, &["reset", "-q", "--hard", &case.granted]);
    let link = format!("{EVIDENCE}/link.json");
    fs::create_dir_all(case.repo.join(EVIDENCE)).expect("a directory");
    std::os::unix::fs::symlink(case.compact(), case.repo.join(&link)).expect("a symlink");
    git(&case.repo, &["add", &link]);
    git(&case.repo, &["commit", "-q", "-m", SUBJECT]);
    let mode = git(&case.repo, &["ls-tree", "HEAD", "--", &link]);
    assert!(mode.starts_with("120000 blob"), "{mode}");
    let blob = git(&case.repo, &["cat-file", "-p", &format!("HEAD:{link}")]);
    assert_eq!(blob, case.compact());
    let symlink = git(&case.repo, &["rev-parse", "HEAD"]);
    assert!(
        !case.rides(&symlink),
        "a symlink whose target is the record"
    );

    git(&case.repo, &["reset", "-q", "--hard", &case.granted]);
    let gitlink = format!("{EVIDENCE}/module");
    git(
        &case.repo,
        &[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("160000,{},{gitlink}", case.granted),
        ],
    );
    git(&case.repo, &["commit", "-q", "-m", SUBJECT]);
    let module = git(&case.repo, &["rev-parse", "HEAD"]);
    assert!(!case.rides(&module), "a gitlink");
}

/// The grant denied, or moved to another revision, in `authority.yaml` after its record was
/// committed: the approval's own commit no longer rides.
#[test]
fn adv1_a_grant_denied_or_rebound_after_its_record_does_not_ride() {
    let case = approved("revoked");
    assert!(case.rides(&case.approved), "the approval's own commit");
    let original = Authority::read(&case.dir)
        .expect("reads")
        .expect("an authority");

    let mut denied = original.clone();
    denied.decisions[0].decision = Grant::Denied;
    denied.write(&case.dir).expect("writes");
    assert!(!case.rides(&case.approved), "a denied grant's record");

    let mut rebound = original.clone();
    rebound.decisions[0].revision = Some("0".repeat(case.granted.len()));
    rebound.write(&case.dir).expect("writes");
    assert!(
        !case.rides(&case.approved),
        "a record of a grant at another revision"
    );

    original.write(&case.dir).expect("writes");
    assert!(case.rides(&case.approved), "the grant put back");
}
