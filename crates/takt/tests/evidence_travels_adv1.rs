//! Adversary pass 1 on story:the-pull-request-and-the-store-carry-the-evidence. Each case
//! attacks the reference the store records for a `test_result` (`<path>#sha256=…&result=…
//! &revision=…&producer=…`, built with no escaping) or the bound on the pull request's body.
//! Every repository is a scratch one this file builds under the crate's build directory.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;
use std::time::Duration;

use anyhow::anyhow;
use takt::case::CASES_DIR;
use takt::evidence::Record;
use takt::forge::{Checks, Forge, Memory};
use takt::frontier::{Aep, approve, decode_reference_part, test_result_reference};
use takt::harness::{Harness, StepContext, StepReport};
use takt::validator::{ChangeRead, CiRead, PrMerged, PrOpen, TestsRun, Validator};
use takt::{Run, RunOptions, Stop};

const DELIVERY: &str = "story.delivery/1";
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");
const EVIDENCE: &str = ".engineering/evidence/story/ready-one";
const APPROVER: &str = "human:operator";
const INTENT: &str = "deliver the story\n\nThe story's body.";

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

fn git(tree: &Path, args: &[&str]) -> String {
    String::from_utf8(git_bytes(tree, args))
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

fn fresh(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "evidence-travels-adv1-{}-{name}",
        std::process::id()
    ));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    dir
}

/// A scratch repository on `main` holding the `one-ready` store and a `check.txt`.
fn repository(name: &str) -> Scratch {
    let dir = fresh(name);
    copy_dir(&Path::new(FIXTURES).join("one-ready"), &dir);
    fs::write(dir.join("check.txt"), "broken\n").expect("a file");
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "start"]);
    Scratch(dir)
}

/// A scratch git tree with one commit on `main`, for a pull request run.
fn scratch(name: &str) -> Scratch {
    let dir = fresh(name);
    fs::create_dir_all(&dir).expect("scratch dir");
    fs::write(dir.join("README.md"), "a story\n").expect("a file");
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "start"]);
    Scratch(dir)
}

fn options(case_id: Option<&str>) -> RunOptions {
    RunOptions {
        protocol: DELIVERY.to_owned(),
        intent: String::new(),
        inputs: Vec::new(),
        iterations: Some(12),
        hours: None,
        tokens: None,
        case_id: case_id.map(str::to_owned),
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

    fn observe(
        &self,
        _: &Path,
        revision: &str,
        seq: usize,
    ) -> Result<takt::validator::Observation, anyhow::Error> {
        Ok(takt::validator::Observation {
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

/// The case `id` of `story:ready-one`, opened under the id the operator gives (`takt run
/// --case <id>`), worked, run to the gate and granted the merge, then run past it to its end.
/// The repository, the forge, the stop and what the last run printed.
fn delivered(name: &str, id: &str) -> (Scratch, Rc<Memory>, Stop, String) {
    let repo = repository(name);
    let memory = Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    });
    let forge: Rc<dyn Forge> = memory.clone();
    let validators = delivery(forge);
    let store = Aep::default();
    let mut out = Vec::new();
    let opened = Run::open_next(
        &repo,
        options(Some(id)),
        &store,
        &Requester,
        &validators,
        &mut out,
    )
    .expect("the frontier answers")
    .expect("story:ready-one is taken");
    assert_eq!(opened.case.id, id, "the case keeps the id it was given");
    drop(opened);
    fs::write(repo.join("check.txt"), "fixed\n").expect("a file");
    git(&repo, &["commit", "-q", "-am", "the work"]);
    let mut run =
        Run::resume(&repo, options(None), id, &Requester, &validators).expect("the case resumes");
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
    approve(&repo, &store, id, "repository.merge", APPROVER).expect("the approval is given");

    let forge: Rc<dyn Forge> = memory.clone();
    let validators = delivery(forge);
    let mut run =
        Run::resume(&repo, options(None), id, &NoHarness, &validators).expect("the case resumes");
    run.record_on(&store);
    run.poll = Duration::from_millis(5);
    let mut out = Vec::new();
    let stop = run.run(&mut out).expect("the run stops without an error");
    drop(run);
    (
        repo,
        memory,
        stop,
        String::from_utf8_lossy(&out).into_owned(),
    )
}

/// The `test_result` reference the store holds at `revision`.
fn stored_reference(repo: &Path, revision: &str) -> String {
    let names = git(
        repo,
        &["ls-tree", "--name-only", &format!("{revision}:{EVIDENCE}")],
    );
    names
        .lines()
        .find_map(|name| {
            let bytes = git_bytes(repo, &["show", &format!("{revision}:{EVIDENCE}/{name}")]);
            let record: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
            (record["change"]["kind"] == "test_result").then(|| {
                record["change"]["reference"]
                    .as_str()
                    .expect("a reference")
                    .to_owned()
            })
        })
        .expect("the store holds a test_result record")
}

/// The story's own form, `<path>#<field>=<value>&…`, read as a reader of the store reads it:
/// the path before the first `#`, then each `&`-separated `key=value`, each part decoded with
/// [`decode_reference_part`]. The parts are encoded, so no `#`, `&` or `=` of a part is a
/// separator (story:the-pull-request-and-the-store-carry-the-evidence); the decoding is the
/// reader's, which the premise that the raw path is the file's path made necessary.
fn parse(reference: &str) -> (String, Vec<(String, String)>) {
    let (path, fragment) = reference
        .split_once('#')
        .expect("the reference is a path, then its fields");
    let fields = fragment
        .split('&')
        .map(|field| {
            let (key, value) = field.split_once('=').unwrap_or((field, ""));
            (
                key.to_owned(),
                decode_reference_part(value).expect("an encoded value"),
            )
        })
        .collect();
    (
        decode_reference_part(path).expect("an encoded path"),
        fields,
    )
}

/// A case id may hold `#` (the case id rule and git both admit it, so `takt run --case
/// issue#42` opens one). The path in the reference is then cut at that `#`: the store's
/// record names a file that does not exist, and its fields start inside the path.
#[test]
fn a_case_id_holding_a_hash_leaves_the_reference_path_whole() {
    let id = "issue#42";
    let (repo, memory, stop, text) = delivered("hash-id", id);
    assert_eq!(stop, Stop::Completed("accepted".to_owned()), "{text}");
    let merged = memory
        .pushed
        .borrow()
        .get(&format!("takt/{id}"))
        .cloned()
        .expect("the branch was pushed");
    let reference = stored_reference(&repo, &merged);
    let (path, fields) = parse(&reference);
    assert!(
        path.starts_with(&format!("{CASES_DIR}/{id}/evidence/")) && repo.join(&path).is_file(),
        "the reference's path names the case record; it reads as `{path}` from `{reference}`"
    );
    let keys: Vec<&str> = fields.iter().map(|(key, _)| key.as_str()).collect();
    assert_eq!(
        keys,
        ["sha256", "result", "revision", "producer"],
        "{reference}"
    );
}

/// The fields of the reference read back as the record's own, whatever a field holds: a
/// producer holding `&`, `=` or `#` adds no field and changes none.
#[test]
fn a_producer_cannot_forge_a_reference_field() {
    let producer = "lab.tests&result=fail&sha256=00#x=y";
    let reference = test_result_reference(
        ".takt/cases/c/evidence/0001-t.json",
        b"{}",
        "pass",
        "0123456789abcdef0123456789abcdef01234567",
        producer,
    );
    let (path, fields) = parse(&reference);
    assert_eq!(path, ".takt/cases/c/evidence/0001-t.json", "{reference}");
    let keys: Vec<&str> = fields.iter().map(|(key, _)| key.as_str()).collect();
    assert_eq!(
        keys,
        ["sha256", "result", "revision", "producer"],
        "a producer added fields: {reference}"
    );
    assert_eq!(fields[3].1, producer, "{reference}");
}

/// Opens a case with the pull request validator alone, records `records` at the
/// implementation revision it commits, and runs it to its pull request: the stop, what it
/// printed, and the body the forge was asked to open.
fn pr_body(
    tree: &Path,
    id: &str,
    records: impl FnOnce(&str) -> Vec<Record>,
) -> (Result<Stop, String>, String, String) {
    let memory = Rc::new(Memory::default());
    let forge: Rc<dyn Forge> = memory.clone();
    let validators: Vec<Box<dyn Validator>> = vec![Box::new(PrOpen::new(forge))];
    let options = RunOptions {
        protocol: DELIVERY.to_owned(),
        intent: INTENT.to_owned(),
        inputs: Vec::new(),
        iterations: Some(1),
        hours: None,
        tokens: None,
        case_id: Some(id.to_owned()),
        gate: Some("repository.merge".to_owned()),
    };
    let mut run = Run::open(tree, options, &NoHarness, &validators).expect("the case opens");
    fs::write(tree.join("implemented.txt"), "done\n").expect("a file");
    git(tree, &["add", "implemented.txt"]);
    git(tree, &["commit", "-q", "-m", "implemented by the test"]);
    let revision = git(tree, &["rev-parse", "HEAD"]);
    run.case
        .set_revision("implementation", &revision)
        .expect("the revision moves");
    for record in records(&revision) {
        run.case
            .add_evidence(&record)
            .expect("the record is written");
    }
    let mut out = Vec::new();
    let stop = run.run(&mut out).map_err(|error| format!("{error:#}"));
    drop(run);
    let body = memory
        .opened
        .borrow()
        .first()
        .map(|opened| opened.body.clone())
        .unwrap_or_default();
    (stop, String::from_utf8(out).expect("utf-8"), body)
}

/// A long case's body is bounded, and what it keeps is what a reviewer needs: the latest
/// records, among them the `test_result` pass the claims and the store's record rest on. The
/// records a run writes are in file-name order oldest first; the bound must not drop the newest.
#[test]
fn a_bounded_body_keeps_the_latest_records() {
    let tree = scratch("latest");
    let observed = std::cell::RefCell::new(String::new());
    let (stop, text, body) = pr_body(&tree, "latest", |revision| {
        observed.replace(revision.to_owned());
        // The earlier tries: failing tests at the revision before the one that passes.
        let earlier = git(&tree, &["rev-parse", &format!("{revision}^")]);
        let mut records: Vec<Record> = (1..=500)
            .map(|n| {
                Record::new(
                    format!("old-{n}"),
                    "test_result",
                    Some("fail".to_owned()),
                    "implementation",
                    earlier.as_str(),
                    "forge-test",
                )
            })
            .collect();
        records.push(Record::new(
            "test-final",
            "test_result",
            Some("pass".to_owned()),
            "implementation",
            revision,
            "forge-test",
        ));
        records.push(Record::new(
            "gate-1",
            "gate_result",
            Some("pass".to_owned()),
            "implementation",
            revision,
            "forge-test",
        ));
        records.push(Record::new(
            "change-1",
            "change_record",
            Some("ahead".to_owned()),
            "implementation",
            revision,
            "takt.change-read",
        ));
        records.push(Record::new(
            "handover",
            "handover",
            None,
            "implementation",
            revision,
            takt::run::CHOICE_PRODUCER,
        ));
        records
    });
    assert_eq!(stop, Ok(Stop::Iterations(1)), "{text}");
    assert!(body.len() < 60_000, "{}", body.len());
    assert!(
        body.contains("left out of this body"),
        "the case is long enough to be bounded:\n{body}"
    );
    let observed = observed.into_inner();
    let short = &observed[..8];
    let pass =
        format!("- `test_result`, result `pass`, revision `{short}`, producer `forge-test`.");
    assert!(
        body.contains(&pass),
        "the latest test_result pass is left out of the body; the body lists only the oldest records:\n{}",
        body.get(body.len().saturating_sub(600)..).unwrap_or(&body)
    );
}
