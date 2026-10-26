//! Adversary pass 2 on riding approval records: the story's revision the rider must carry is read
//! from the story file at the granted revision, by hand. Each case approves through the real store
//! (`takt approve`, the real `aep`) wherever the store accepts the story as committed, so a refusal
//! here is a refusal of the store's own record.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

use anyhow::anyhow;
use takt::case::{Authority, Branch, CASES_DIR, approvals_only};
use takt::evidence::Record;
use takt::forge::{Checks, Forge, Memory};
use takt::frontier::{Aep, approve};
use takt::harness::{Harness, StepContext, StepReport};
use takt::validator::{CiRead, PrMerged, PrOpen, Validator};
use takt::{Run, RunOptions, Stop};

const PROTOCOL: &str = "story.delivery/1";

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");

/// The story file of `story:ready-one`.
const STORY: &str = ".engineering/planning/story/ready-one.md";

/// Where the store keeps the evidence on `story:ready-one`.
const EVIDENCE: &str = ".engineering/evidence/story/ready-one";

/// The approval's subject for `story:ready-one` and `repository.merge`.
const SUBJECT: &str = "takt: approve story:ready-one repository.merge";

/// Git as it runs with no `core.autocrlf` configured (git's default), so a file is committed with
/// the line ends it was saved with, whatever the machine's global configuration says.
fn git(tree: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(["-c", "core.autocrlf=false"])
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
        "approval-riders-adv2-{}-{name}",
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
        "approval-riders-adv2",
    )
}

fn double() -> Rc<Memory> {
    Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    })
}

/// `story:ready-one` taken, its work committed with the story file rewritten by `story` (the work
/// commit is where a developer's editor would have touched it), the tests and the gate green, one
/// run to the gate: the case id and the granted revision.
fn at_the_gate(
    repo: &Path,
    memory: &Rc<Memory>,
    story: impl FnOnce(&str) -> String,
) -> (String, String) {
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
    let text = fs::read_to_string(repo.join(STORY)).expect("the story");
    fs::write(repo.join(STORY), story(&text)).expect("the story");
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

struct Approved {
    repo: Scratch,
    id: String,
    dir: PathBuf,
    granted: String,
    approved: String,
    record: serde_json::Value,
}

/// A case approved through the real store, its story file at the granted revision as `story`
/// leaves it.
fn approved_with(name: &str, memory: &Rc<Memory>, story: impl FnOnce(&str) -> String) -> Approved {
    let repo = repository(name);
    let (id, granted) = at_the_gate(&repo, memory, story);
    approve(
        &repo,
        &Aep::default(),
        &id,
        "repository.merge",
        "human:operator",
    )
    .expect("the store accepts the story and the approval is given");
    let approved = git(&repo, &["rev-parse", "HEAD"]);
    assert_eq!(git(&repo, &["rev-parse", "HEAD^"]), granted);
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

fn approved(name: &str, memory: &Rc<Memory>) -> Approved {
    approved_with(name, memory, str::to_owned)
}

impl Approved {
    /// One commit on `on` with the approval's subject, adding each `(name, text)` in the story's
    /// evidence and writing `story` as the story file when given: the commit.
    fn rider_on(&self, on: &str, files: &[(&str, String)], story: Option<&str>) -> String {
        git(&self.repo, &["reset", "-q", "--hard", on]);
        for (name, text) in files {
            let path = format!("{EVIDENCE}/{name}");
            fs::create_dir_all(self.repo.join(EVIDENCE)).expect("a directory");
            fs::write(self.repo.join(&path), text).expect("a file");
            git(&self.repo, &["add", &path]);
        }
        if let Some(story) = story {
            fs::write(self.repo.join(STORY), story).expect("the story");
            git(&self.repo, &["add", STORY]);
        }
        git(&self.repo, &["commit", "-q", "-m", SUBJECT]);
        git(&self.repo, &["rev-parse", "HEAD"])
    }

    fn rides_on(&self, revision: &str, head: &str) -> bool {
        approvals_only(&self.repo, revision, head, &self.dir).expect("approvals_only reads")
    }

    fn rides(&self, head: &str) -> bool {
        self.rides_on(&self.granted, head)
    }

    fn edited(&self, edit: impl FnOnce(&mut serde_json::Value)) -> String {
        let mut record = self.record.clone();
        edit(&mut record);
        serde_json::to_string_pretty(&record).expect("json")
    }

    /// The case moved to a new revision: a commit on the granted one writing the story file as
    /// `story` (or leaving it, or deleting it with `None`), the grant rebound to it in
    /// `authority.yaml`. The store's record of the grant at the new revision is the real record
    /// with that revision in its source. The new revision and that record.
    fn regranted(&self, story: Option<Option<&str>>) -> (String, serde_json::Value) {
        git(&self.repo, &["reset", "-q", "--hard", &self.granted]);
        match story {
            Some(Some(text)) => fs::write(self.repo.join(STORY), text).expect("the story"),
            Some(None) => fs::remove_file(self.repo.join(STORY)).expect("removed"),
            None => fs::write(self.repo.join("check.txt"), "fixed again\n").expect("a file"),
        }
        git(
            &self.repo,
            &["add", "-A", "--", "check.txt", ".engineering/planning"],
        );
        git(&self.repo, &["commit", "-q", "-m", "more work"]);
        let revision = git(&self.repo, &["rev-parse", "HEAD"]);
        let mut authority = Authority::read(&self.dir)
            .expect("reads")
            .expect("an authority");
        authority.decisions[0].revision = Some(revision.clone());
        authority.write(&self.dir).expect("writes");
        let mut record = self.record.clone();
        let source = record["change"]["source"]
            .as_str()
            .expect("a source")
            .replace(&self.granted, &revision);
        record["change"]["source"] = source.into();
        (revision, record)
    }
}

fn text(record: &serde_json::Value) -> String {
    serde_json::to_string_pretty(record).expect("json")
}

/// Every line of `text` ending in CRLF, as an editor on another platform saves it.
fn crlf(text: &str) -> String {
    text.replace('\n', "\r\n")
}

/// The story's revision, the number on its front matter's top-level `revision:` line (the first
/// line starting so; a transition's revision sits inside a flow mapping).
fn revision_of(story: &str) -> u64 {
    story
        .lines()
        .find_map(|line| line.strip_prefix("revision: "))
        .and_then(|value| value.trim().parse().ok())
        .expect("a revision line")
}

/// `story` with its top-level `revision:` line replaced by `line(revision)`.
fn respelled(story: &str, line: impl Fn(u64) -> String) -> String {
    let revision = revision_of(story);
    story.replacen(
        &format!("\nrevision: {revision}\n"),
        &format!("\n{}\n", line(revision)),
        1,
    )
}

/// The positive path the correction must not break: a real `takt approve`, then the resumed run
/// pushes and merges the approval's own commit.
#[test]
fn adv2_a_real_approval_rides_and_is_merged() {
    let memory = double();
    let case = approved("merged", &memory);
    assert!(case.rides(&case.approved), "the approval's own commit");
    let forge: Rc<dyn Forge> = memory.clone();
    let validators = validators(forge);
    let mut run =
        Run::resume(&case.repo, options(), &case.id, &NoHarness, &validators).expect("resumes");
    assert_eq!(
        run.case.revision("implementation").as_deref(),
        Some(case.granted.as_str())
    );
    let mut out = Vec::new();
    let stop = run.run(&mut out).expect("the run stops without an error");
    let text = String::from_utf8_lossy(&out).into_owned();
    drop(run);
    assert_eq!(stop, Stop::Completed("accepted".to_owned()), "{text}");
    assert_eq!(memory.merged.borrow().as_slice(), [1], "{text}");
    assert_eq!(
        Branch::read(&case.dir).expect("reads").pushed.as_deref(),
        Some(case.approved.as_str()),
        "{text}"
    );
}

/// The store `aep` writes an approval's record with these fields and no other; the rider check
/// refuses any other field, so a field the store adds would refuse every real approval.
#[test]
fn adv2_the_real_record_has_exactly_the_fields_the_rider_check_requires() {
    let memory = double();
    let case = approved("fields", &memory);
    let keys = |value: &serde_json::Value| -> Vec<String> {
        let mut keys: Vec<String> = value
            .as_object()
            .expect("an object")
            .keys()
            .cloned()
            .collect();
        keys.sort();
        keys
    };
    assert_eq!(
        keys(&case.record),
        ["actor", "artifact", "at", "change", "kind", "revision"],
        "{}",
        case.record
    );
    assert_eq!(
        keys(&case.record["change"]),
        ["change", "kind", "reference", "source"],
        "{}",
        case.record
    );
    let story = git(&case.repo, &["show", &format!("{}:{STORY}", case.granted)]);
    assert_eq!(
        case.record["revision"],
        revision_of(&story),
        "{}",
        case.record
    );
}

/// A story file saved with CRLF line ends is a store `aep` reads: it records the approval with the
/// story's revision. The store's own record of the grant must ride.
#[test]
fn adv2_a_story_saved_with_crlf_still_carries_its_real_approval() {
    let memory = double();
    let case = approved_with("crlf", &memory, crlf);
    let story = git(&case.repo, &["show", &format!("{}:{STORY}", case.granted)]);
    assert!(
        story.contains("\r\n"),
        "the story at the granted revision is CRLF"
    );
    assert_eq!(
        case.record["revision"],
        revision_of(&story.replace('\r', "")),
        "{}",
        case.record
    );
    assert!(
        case.rides(&case.approved),
        "the store's own record of the grant is refused on a CRLF story at {}",
        case.granted
    );
}

/// Spellings of the story's revision the store reads as that number (it records the number for
/// each): the store's own record of the grant must ride on every one.
#[test]
fn adv2_every_revision_spelling_the_store_reads_carries_its_real_approval() {
    type Spelling = fn(u64) -> String;
    let spellings: [(&str, Spelling); 6] = [
        ("hex", |n| format!("revision: {n:#x}")),
        ("octal", |n| format!("revision: 0o{n:o}")),
        ("plus", |n| format!("revision: +{n}")),
        ("tagged", |n| format!("revision: !!int {n}")),
        ("comment", |n| format!("revision: {n} # the story's")),
        ("trailing", |n| format!("revision: {n}   ")),
    ];
    let mut refused = Vec::new();
    for (name, spelling) in spellings {
        let memory = double();
        let mut spelled = String::new();
        let case = approved_with(name, &memory, |text| {
            spelled = spelling(revision_of(text));
            respelled(text, spelling)
        });
        let story = git(&case.repo, &["show", &format!("{}:{STORY}", case.granted)]);
        assert!(story.contains(spelled.trim_end()), "{name}: {story}");
        assert!(case.record["revision"].is_u64(), "{name}: {}", case.record);
        if !case.rides(&case.approved) {
            refused.push(spelled);
        }
    }
    assert!(
        refused.is_empty(),
        "the store's own record of the grant is refused on a story at `{refused:?}`"
    );
}

/// `revision:` in the story's body is not the story's revision; a rider carrying it does not ride,
/// and the real record does.
#[test]
fn adv2_a_revision_in_the_story_body_is_not_read() {
    let memory = double();
    let case = approved_with("body", &memory, |text| {
        format!("{text}\nrevision: 9\n---\nrevision: 8\n")
    });
    assert!(case.rides(&case.approved), "the store's own record");
    for revision in [9_u64, 8, 2] {
        let forged = case.edited(|r| r["revision"] = revision.into());
        let head = case.rider_on(&case.granted, &[("x.json", forged)], None);
        assert!(!case.rides(&head), "a record at revision {revision} rides");
    }
}

/// A story the store refuses at the granted revision (`revision:` twice, quoted, or no front matter
/// the store can open) has no revision a record could carry: no rider rides on it, whatever it says.
#[test]
fn adv2_a_story_the_store_refuses_carries_no_rider() {
    let memory = double();
    let case = approved("refused", &memory);
    let original = fs::read_to_string(case.repo.join(STORY)).expect("the story");
    let (control, record) = case.regranted(None);
    let head = case.rider_on(&control, &[("x.json", text(&record))], None);
    assert!(
        case.rides_on(&control, &head),
        "the control: a regranted record rides"
    );

    let n = revision_of(&original);
    let other = n + 2;
    for (what, story) in [
        (
            "revision twice, the story's first",
            respelled(&original, |n| format!("revision: {n}\nrevision: {other}")),
        ),
        (
            "revision twice, the story's last",
            respelled(&original, |n| format!("revision: {other}\nrevision: {n}")),
        ),
        (
            "a quoted revision",
            respelled(&original, |n| format!("revision: \"{n}\"")),
        ),
        (
            "a byte order mark before the front matter",
            format!("\u{feff}{original}"),
        ),
    ] {
        assert_ne!(story, original, "{what}");
        let (revision, record) = case.regranted(Some(Some(&story)));
        for at in [n, other] {
            let mut forged = record.clone();
            forged["revision"] = at.into();
            let head = case.rider_on(&revision, &[("x.json", text(&forged))], None);
            assert!(
                !case.rides_on(&revision, &head),
                "{what}: a record at revision {at} rides"
            );
        }
    }

    let (gone, record) = case.regranted(Some(None));
    let head = case.rider_on(&gone, &[("x.json", text(&record))], None);
    assert!(
        !case.rides_on(&gone, &head),
        "no story file at the revision"
    );
}

/// Whitespace before the opening `---` (a blank line, or spaces): `aep` refuses the whole store
/// ("has no frontmatter: it must open with a `---` line"), so it records nothing and no grant is
/// written. The rider check reads the story through `git::run`, which trims its output, and finds
/// the revision anyway: a rider rides on a story the store cannot open.
#[test]
fn adv2_a_story_with_whitespace_before_its_front_matter_carries_no_rider() {
    let memory = double();
    let case = approved("leading", &memory);
    let original = fs::read_to_string(case.repo.join(STORY)).expect("the story");
    let mut rode = Vec::new();
    for (what, story) in [
        ("a blank line", format!("\n{original}")),
        ("two spaces", format!("  {original}")),
    ] {
        let (revision, record) = case.regranted(Some(Some(&story)));
        let head = case.rider_on(&revision, &[("x.json", text(&record))], None);
        if case.rides_on(&revision, &head) {
            rode.push(what);
        }
    }
    assert!(
        rode.is_empty(),
        "a rider rides on a story the store refuses to open, with {rode:?} before its front matter"
    );
}

/// The rider commit that also changes the story (its revision bumped to what the record says, or
/// the story removed) is not the approval's own commit and does not ride.
#[test]
fn adv2_a_rider_that_changes_the_story_does_not_ride() {
    let memory = double();
    let case = approved("changes-story", &memory);
    let original = fs::read_to_string(case.repo.join(STORY)).expect("the story");
    let next = revision_of(&original) + 1;
    let bumped = respelled(&original, |_| format!("revision: {next}"));
    assert_ne!(bumped, original);
    let at_next = case.edited(|r| r["revision"] = next.into());
    let head = case.rider_on(&case.granted, &[("x.json", at_next)], Some(&bumped));
    assert!(
        !case.rides(&head),
        "the story bumped to {next} with a record at {next}"
    );
    let real = case.edited(|_| {});
    let head = case.rider_on(&case.granted, &[("x.json", real)], Some(&bumped));
    assert!(
        !case.rides(&head),
        "the story bumped to {next} with the real record"
    );
    let head = case.rider_on(&case.granted, &[], Some(&bumped));
    assert!(!case.rides(&head), "the story bumped alone");
}

/// `head` the granted revision: true with no rider, whether or not a grant is held, and whatever
/// the store holds at that revision; a head above it with no grant is false.
#[test]
fn adv2_the_revision_itself_rides_with_or_without_grants_or_a_story() {
    let memory = double();
    let case = approved("itself", &memory);
    assert!(case.rides(&case.granted), "the granted revision, granted");
    let (gone, _) = case.regranted(Some(None));
    assert!(case.rides_on(&gone, &gone), "a revision with no story file");
    let mut authority = Authority::read(&case.dir).expect("reads").expect("held");
    authority.decisions.clear();
    authority.write(&case.dir).expect("writes");
    assert!(
        case.rides_on(&case.granted, &case.granted),
        "the revision with no grant"
    );
    assert!(
        !case.rides(&case.approved),
        "the approval's commit with its grant gone"
    );
}

/// The record rides only with its approver as its actor: actors the store writes (`aep` accepts
/// exactly `-._@` beside ASCII letters and digits, any length, and sanitises `$USER` into them)
/// that are not the approver do not ride, and neither do actors it never writes.
#[test]
fn adv2_actors_ride_by_the_store_shape() {
    let memory = double();
    let case = approved("actors", &memory);
    let head = case.rider_on(
        &case.granted,
        &[(
            "x.json",
            case.edited(|r| r["actor"] = "human:operator".into()),
        )],
        None,
    );
    assert!(case.rides(&head), "the approver is refused");
    let long = format!("human:{}", "a".repeat(300));
    for actor in [
        "system",
        "service:ci",
        "human:unknown",
        "human:first-last-x",
        "agent:a.b@c_d-E9",
        "human:-a",
        "agent:operator",
        "human:operator ",
        long.as_str(),
        "human:a+b",
        "human:a/b",
        "human:a:b",
        "human:j\u{f6}ran",
        "robot:x",
        "System",
        "human",
        "",
    ] {
        let head = case.rider_on(
            &case.granted,
            &[("x.json", case.edited(|r| r["actor"] = actor.into()))],
            None,
        );
        assert!(!case.rides(&head), "{actor:?} rides");
    }
}
