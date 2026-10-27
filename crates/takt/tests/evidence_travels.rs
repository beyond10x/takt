//! story:the-pull-request-and-the-store-carry-the-evidence: what a reviewer reads in the planning
//! store and in the pull request stands without the case directory. The `test_result` record a
//! story's run writes names its case record by path, sha256, result, subject revision and
//! producer, and the record's `StoreEvidence` event names the same reference. The pull request's
//! body summarises the case's evidence records, the steps and the tokens `events.jsonl` states,
//! stays under 60,000 bytes however many records the case holds, and is the text the bot route
//! scans before it opens the pull request.
//!
//! The scan itself is `b10x-gates`' and needs its policy, which CI does not hold: the last test
//! proves the body is what the scan reads and that the pull request opens behind a scan that
//! accepts it, with a stand-in for the program. It does not prove the policy's rules.

use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;
use std::time::Duration;

use anyhow::anyhow;
use sha2::{Digest as _, Sha256};
use takt::case::CASES_DIR;
use takt::evidence::Record;
use takt::forge::{BotRoute, Checks, Exit, Forge, Memory, Runner};
use takt::frontier::{Aep, approve};
use takt::harness::{Harness, StepContext, StepReport};
use takt::validator::{ChangeRead, CiRead, PrMerged, PrOpen, TestsRun, Validator};
use takt::{Run, RunOptions, Stop};

/// The delivery protocol, the engineering protocols' built-in.
const DELIVERY: &str = "story.delivery/1";

/// The protocol a pull request run opens its case under.
const PROTOCOL: &str = "story.delivery/1";

/// The fixture stores, relative to this crate.
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");

/// Where the story's evidence lives in the planning store.
const EVIDENCE: &str = ".engineering/evidence/story/ready-one";

/// Who grants the merge: an actor the store records.
const APPROVER: &str = "human:operator";

/// The intent a pull request run is given: its title is the first line.
const INTENT: &str = "deliver the story\n\nThe story's body.";

/// The file the bot route writes the pull request's title and body to, as published.
const PUBLISHED: &str = "pull-request.published.txt";

/// The policy file the forge is given; the stand-in scan only checks that a line names it.
const POLICY: &str = "/policies/gates.json";

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

/// Runs `git <args>` in `tree`; its standard output, trimmed.
fn git(tree: &Path, args: &[&str]) -> String {
    String::from_utf8(git_bytes(tree, args))
        .expect("utf-8")
        .trim()
        .to_owned()
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
        .join(format!("evidence-travels-{}-{name}", std::process::id()));
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

/// A scratch git tree with one commit on `main`, for a pull request run.
fn scratch(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("evidence-travels-pr-{}-{name}", std::process::id()));
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

fn options(case_id: Option<&str>, iterations: Option<u64>) -> RunOptions {
    RunOptions {
        protocol: DELIVERY.to_owned(),
        intent: String::new(),
        inputs: Vec::new(),
        iterations,
        hours: None,
        tokens: None,
        case_id: case_id.map(str::to_owned),
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

/// Requests the merge gate as soon as it is offered, or hands the finished change over,
/// changing nothing. It reports no tokens, so a body states them as not stated.
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

/// The validators of a delivery run through `forge`, in the order the binary registers them.
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

/// A case of `story:ready-one` taken, worked, and run to the gate, granted the merge through the
/// store by [`APPROVER`].
struct Granted {
    repo: Scratch,
    memory: Rc<Memory>,
    id: String,
    dir: PathBuf,
    /// The revision the grant authorises: the implementation.
    granted: String,
}

fn granted(name: &str) -> Granted {
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
        options(None, Some(12)),
        &store,
        &Requester,
        &validators,
        &mut out,
    )
    .expect("the frontier answers")
    .expect("story:ready-one is taken");
    let id = opened.case.id.clone();
    let dir = opened.case.dir.clone();
    drop(opened);
    fs::write(repo.join("check.txt"), "fixed\n").expect("a file");
    git(&repo, &["commit", "-q", "-am", "the work"]);
    let granted = git(&repo, &["rev-parse", "HEAD"]);
    let mut run = Run::resume(&repo, options(None, Some(12)), &id, &Requester, &validators)
        .expect("the case resumes");
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
    Granted {
        repo,
        memory,
        id,
        dir,
        granted,
    }
}

impl Granted {
    /// Runs the case once more past the gate: the stop and what it printed.
    fn finish(&self, harness: &dyn Harness) -> (Stop, String) {
        let forge: Rc<dyn Forge> = self.memory.clone();
        let validators = delivery(forge);
        let store = Aep::default();
        let mut run = Run::resume(
            &self.repo,
            options(None, Some(12)),
            &self.id,
            harness,
            &validators,
        )
        .expect("the case resumes");
        run.record_on(&store);
        run.poll = Duration::from_millis(5);
        let mut out = Vec::new();
        let stop = run.run(&mut out).expect("the run stops without an error");
        (stop, String::from_utf8_lossy(&out).into_owned())
    }

    /// The head the forge holds for the case's branch: the one merged.
    fn merged(&self) -> String {
        self.memory
            .pushed
            .borrow()
            .get(&format!("takt/{}", self.id))
            .cloned()
            .expect("the branch was pushed")
    }
}

/// The files under the story's evidence directory at `revision`, each `(path, bytes)`.
fn story_evidence_at(repo: &Path, revision: &str) -> Vec<(String, Vec<u8>)> {
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

/// The evidence records a case holds under its `evidence/`, each as JSON, in file-name order.
fn case_records(case_dir: &Path) -> Vec<serde_json::Value> {
    let mut paths: Vec<PathBuf> = fs::read_dir(case_dir.join("evidence"))
        .expect("the case holds evidence")
        .map(|entry| entry.expect("an entry").path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .collect();
    paths.sort();
    paths
        .iter()
        .map(|path| serde_json::from_slice(&fs::read(path).expect("a record")).expect("json"))
        .collect()
}

/// The evidence records a case held when its pull request's body was made: all of them but the
/// record the opening writes once the pull request is open, which follows the body.
fn case_records_at_body(case_dir: &Path) -> Vec<serde_json::Value> {
    case_records(case_dir)
        .into_iter()
        .filter(|record| record["kind"] != "pull_request")
        .collect()
}

/// The first eight characters of a revision, as a pull request body prints one.
fn short(revision: &str) -> &str {
    revision.get(..8).unwrap_or(revision)
}

/// The lower-case hex of `bytes`.
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// The `events.jsonl` lines of the case, each as JSON.
fn events_of(case_dir: &Path) -> Vec<serde_json::Value> {
    fs::read_to_string(case_dir.join("events.jsonl"))
        .expect("the case keeps its run record")
        .lines()
        .map(|line| serde_json::from_str(line).expect("a line of json"))
        .collect()
}

/// Opens a case under `forge` with the pull request validator alone, records the evidence
/// `records` names at the implementation revision it commits, and runs it to its pull request.
/// The stop, what it printed, and the case directory.
fn pr_run(
    tree: &Path,
    id: &str,
    forge: Rc<dyn Forge>,
    records: impl FnOnce(&str) -> Vec<Record>,
) -> (Result<Stop, String>, String, PathBuf) {
    let validators: Vec<Box<dyn Validator>> = vec![Box::new(PrOpen::new(forge))];
    let options = RunOptions {
        protocol: PROTOCOL.to_owned(),
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
    let dir = tree.join(CASES_DIR).join(id);
    let mut out = Vec::new();
    let stop = run.run(&mut out).map_err(|error| format!("{error:#}"));
    (stop, String::from_utf8(out).expect("utf-8"), dir)
}

/// The records every pull request run holds: a test result, a gate result, the change the
/// handover names, and the handover itself, a choice record with no result.
fn three_records(revision: &str) -> Vec<Record> {
    vec![
        Record::new(
            "test-1",
            "test_result",
            Some("pass".to_owned()),
            "implementation",
            revision,
            "forge-test",
        ),
        Record::new(
            "gate-1",
            "gate_result",
            Some("pass".to_owned()),
            "implementation",
            revision,
            "forge-test",
        ),
        Record::new(
            "change-1",
            "change_record",
            Some("ahead".to_owned()),
            "implementation",
            revision,
            "takt.change-read",
        ),
        Record::new(
            "handover",
            "handover",
            None,
            "implementation",
            revision,
            takt::run::CHOICE_PRODUCER,
        ),
    ]
}

/// A stand-in for `b10x-gates`: it records every line, reads the file each `scan-text` line
/// names (the last words that are not options), accepts every scan and answers `api` with pull
/// request #7 at its `--output`.
struct Scans {
    lines: RefCell<Vec<String>>,
    scanned: RefCell<Vec<String>>,
}

impl Scans {
    fn new() -> Rc<Self> {
        Rc::new(Self {
            lines: RefCell::new(Vec::new()),
            scanned: RefCell::new(Vec::new()),
        })
    }
}

/// Lends a shared [`Scans`] to a forge that owns its runner.
struct Lend(Rc<Scans>);

impl Runner for Lend {
    fn run_env(&self, tree: &Path, argv: &[String], _: &[(&str, &str)]) -> anyhow::Result<Exit> {
        self.run(tree, argv)
    }

    fn run(&self, _: &Path, argv: &[String]) -> anyhow::Result<Exit> {
        self.0.lines.borrow_mut().push(argv.join(" "));
        let command = argv.get(1).map(String::as_str);
        if command == Some("scan-text") {
            let mut words = argv.iter().skip(2);
            while let Some(word) = words.next() {
                if word.starts_with("--") {
                    words.next();
                    continue;
                }
                let text = fs::read_to_string(word)
                    .unwrap_or_else(|error| panic!("scan-text names {word}: {error}"));
                self.0.scanned.borrow_mut().push(text);
            }
        }
        if command == Some("api") {
            let output = argv
                .iter()
                .position(|a| a == "--output")
                .map(|i| &argv[i + 1])
                .expect("--output");
            fs::write(output, r#"{"number": 7, "state": "open"}"#)?;
        }
        Ok(Exit {
            code: Some(0),
            stdout: String::new(),
            stderr: String::new(),
        })
    }
}

/// `test_result` evidence: the store's record names the case record it refers to by path,
/// sha256 and fields, and the run's `StoreEvidence` event names the same reference. The source
/// stays `takt`, and the story resumes as done: nothing is written again.
#[test]
fn store_record_names_the_evidence() {
    let case = granted("store-record");
    let (stop, text) = case.finish(&NoHarness);
    assert_eq!(stop, Stop::Completed("accepted".to_owned()), "{text}");
    let merged = case.merged();

    let evidence = story_evidence_at(&case.repo, &merged);
    let (_, bytes) = evidence
        .iter()
        .find(|(_, bytes)| {
            let record: serde_json::Value = serde_json::from_slice(bytes).expect("a json record");
            record["change"]["kind"] == "test_result"
        })
        .expect("the store holds a test_result record");
    let stored: serde_json::Value = serde_json::from_slice(bytes).expect("a json record");
    assert_eq!(stored["change"]["source"], "takt", "{stored}");
    let reference = stored["change"]["reference"].as_str().expect("a reference");

    let (path, fields) = reference
        .split_once('#')
        .expect("the reference is a path, then its fields");
    assert!(
        path.starts_with(&format!("{CASES_DIR}/{}/evidence/", case.id)),
        "{path}"
    );
    let record_bytes = fs::read(case.repo.join(path)).expect("the case record is in the tree");
    let record: serde_json::Value = serde_json::from_slice(&record_bytes).expect("json");
    assert_eq!(record["kind"], "test_result", "{record}");
    assert_eq!(
        record["subject_revision"],
        case.granted.as_str(),
        "{record}"
    );
    let expected = format!(
        "sha256={}&result={}&revision={}&producer={}",
        hex(&Sha256::digest(&record_bytes)),
        record["result"].as_str().expect("a result"),
        record["subject_revision"].as_str().expect("a revision"),
        record["producer"].as_str().expect("a producer"),
    );
    assert_eq!(
        fields, expected,
        "the reference names the record's bytes and fields"
    );

    let stored_event = events_of(&case.dir)
        .into_iter()
        .rev()
        .find(|line| line["event"] == "store_evidence" && line["kind"] == "test_result")
        .expect("the run records the store evidence");
    assert_eq!(stored_event["reference"].as_str(), Some(reference));

    let validate = Command::new("aep")
        .args(["plan", "artifact", "validate"])
        .current_dir(&*case.repo)
        .output()
        .expect("aep runs");
    assert!(
        validate.status.success(),
        "{}",
        String::from_utf8_lossy(&validate.stderr)
    );

    let commits = git(&case.repo, &["rev-list", "--count", &merged]);
    let (stop, text) = case.finish(&NoHarness);
    assert_eq!(stop, Stop::Completed("accepted".to_owned()), "{text}");
    assert!(!text.contains("step "), "{text}");
    assert_eq!(git(&case.repo, &["rev-list", "--count", &merged]), commits);
    assert_eq!(
        story_evidence_at(&case.repo, &merged)
            .iter()
            .filter(|(_, bytes)| {
                let record: serde_json::Value =
                    serde_json::from_slice(bytes).expect("a json record");
                record["change"]["kind"] == "test_result"
            })
            .count(),
        1,
        "one test_result is recorded"
    );
}

/// The pull request's body: each evidence record of the case by its kind, result, short revision
/// and producer, the steps and the tokens `events.jsonl` states when the body is made, and
/// "not stated" for a figure no line holds.
#[test]
fn pull_request_body_summarises_the_case() {
    let tree = scratch("summary");
    let memory = Rc::new(Memory::default());
    let forge: Rc<dyn Forge> = memory.clone();
    let (stop, text, dir) = pr_run(&tree, "summary", forge, three_records);
    assert_eq!(stop, Ok(Stop::Iterations(1)), "{text}");
    let opened = memory.opened.borrow();
    assert_eq!(opened.len(), 1, "{text}");
    let body = &opened[0].body;

    let records = case_records_at_body(&dir);
    assert!(records.len() >= 3, "{records:?}");
    for record in &records {
        let figure = |name: &str| record[name].as_str().unwrap_or("not stated").to_owned();
        let revision = record["subject_revision"].as_str().map_or_else(
            || "not stated".to_owned(),
            |revision| short(revision).to_owned(),
        );
        let line = format!(
            "- `{}`, result `{}`, revision `{revision}`, producer `{}`.",
            figure("kind"),
            figure("result"),
            figure("producer"),
        );
        assert!(body.contains(&line), "{line} is not in the body:\n{body}");
    }

    let events = events_of(&dir);
    let opening = events
        .iter()
        .position(|line| line["event"] == "step" && line["action"] == "pr.open")
        .expect("the run records the step that opens the pull request");
    let steps = events[..opening]
        .iter()
        .filter(|line| line["event"] == "step")
        .count();
    assert!(
        body.contains(&format!("Steps: {steps}. Tokens: not stated.")),
        "the body states the steps and says the tokens are not stated:\n{body}"
    );
}

/// The body of a case with 500 evidence records is under 60,000 bytes, lists the records that
/// fit, and says how many it left out.
#[test]
fn body_is_bounded() {
    let tree = scratch("bounded");
    let memory = Rc::new(Memory::default());
    let forge: Rc<dyn Forge> = memory.clone();
    let (stop, text, dir) = pr_run(&tree, "bounded", forge, |revision| {
        (1..=500)
            .map(|n| {
                Record::new(
                    format!("bulk-{n}"),
                    "gate_result",
                    Some("pass".to_owned()),
                    "implementation",
                    revision,
                    "forge-test",
                )
            })
            .chain(three_records(revision))
            .collect()
    });
    assert_eq!(stop, Ok(Stop::Iterations(1)), "{text}");
    let opened = memory.opened.borrow();
    let body = &opened[0].body;

    let held = case_records_at_body(&dir).len();
    assert!(held >= 500, "the case holds {held} records");
    assert!(body.len() < 60_000, "the body is {} bytes", body.len());
    let listed = body.lines().filter(|line| line.starts_with("- `")).count();
    assert!(listed > 0 && listed < held, "{listed} of {held} listed");
    let left_out = held - listed;
    assert!(
        body.contains(&format!(
            "{left_out} of {held} evidence records are left out"
        )),
        "the body counts the records it left out:\n{}",
        body.get(body.len().saturating_sub(400)..).unwrap_or(body)
    );
}

/// The body is the text the bot route scans before it opens the pull request: the scan reads
/// the title and the body as published, the pull request opens behind it, and the body is what
/// was scanned. The scan's own rules are the policy's; see the module's note.
#[test]
fn body_passes_scan_text() {
    let tree = scratch("scan");
    let scans = Scans::new();
    let forge = Rc::new(BotRoute::new(
        "owner/repo",
        Some(PathBuf::from(POLICY)),
        Box::new(Lend(scans.clone())),
    ));
    let (stop, text, dir) = pr_run(&tree, "scan", forge, three_records);
    assert_eq!(stop, Ok(Stop::Iterations(1)), "{text}");
    assert!(text.contains("opened pull request #7"), "{text}");

    let lines = scans.lines.borrow();
    let scan = lines
        .iter()
        .position(|line| line.starts_with("b10x-gates scan-text ") && line.contains(PUBLISHED))
        .expect("the title and body are scanned");
    let open = lines
        .iter()
        .position(|line| line.starts_with("b10x-gates api --method POST "))
        .expect("the pull request is opened");
    assert!(scan < open, "{lines:?}");

    let published = fs::read_to_string(dir.join("artifacts").join(PUBLISHED))
        .expect("the published title and body are kept");
    assert!(
        published.starts_with("deliver the story\n\n"),
        "{published}"
    );
    assert!(published.contains("Steps: "), "{published}");
    assert!(published.len() < 60_000, "{}", published.len());
    assert_eq!(scans.scanned.borrow().last(), Some(&published));
}
