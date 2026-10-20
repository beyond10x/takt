//! story:run-record-survives-a-torn-character: a write cut inside a multi-byte character (a full
//! disk, a process killed mid-write) leaves a line that is not valid UTF-8. `Events::read` skips
//! it the way it skips any torn line, the line after it naming it, and reads every other line;
//! `record_accepted` and the scorecard read such a record.

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::anyhow;
use takt::case::CASES_DIR;
use takt::events::{EVENTS_FILE, Event, Events};
use takt::frontier::Aep;
use takt::harness::{Harness, StepContext, StepReport};
use takt::scorecard::Scorecard;
use takt::validator::{TestsRun, Validator};
use takt::{Run, RunOptions, Stop};

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");
const STORY: &str = "story:ready-one";

/// `{"summary":"grün` cut after the first byte of `ü`: the line ends inside a character.
const CUT: &[u8] =
    b"{\"at\":\"2026-10-18T00:00:00Z\",\"step\":1,\"event\":\"step\",\"summary\":\"gr\xc3";

const TESTS_ONLY: &str = "format: protocol/1

protocol:
  id: tests.only
  revision: 1
  description: >-
    Run the tests against the implementation at its current revision; accepted once they pass.

artifacts:
  implementation:
    description: a revision of the code

evidence_kinds:
  test_result:
    description: the result of running tests against an implementation revision

claims:
  tests.pass:
    description: the tests pass for the current implementation revision
    true_when:
      evidence:
        kind: test_result
        result: pass
        subject: implementation

obligations:
  tests.required:
    description: the tests pass for the current implementation revision
    discharged_when:
      claim: tests.pass

actions:
  tests.run:
    description: run the tests against an implementation revision
    effect: read
    may_produce:
      - evidence: test_result

outcomes:
  accepted:
    description: the tests pass for the current implementation revision
    requires:
      claim: tests.pass
";

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

fn scratch(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("run-record-torn-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(&dir).expect("a scratch tree");
    Scratch(dir)
}

fn error(text: &str) -> Event {
    Event::Error {
        error: text.to_owned(),
    }
}

fn append(path: &Path, bytes: &[u8]) {
    fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .expect("the record")
        .write_all(bytes)
        .expect("the bytes are appended");
}

fn kinds(read: &[serde_json::Value]) -> Vec<&str> {
    read.iter()
        .map(|line| line["event"].as_str().expect("an event"))
        .collect()
}

/// The last line cut inside a multi-byte character, no later write: the record reads, every
/// line before it whole.
#[test]
fn a_last_line_cut_inside_a_character_reads_with_every_earlier_event() {
    let dir = scratch("last");
    let events = Events::new(&dir);
    events.emit(0, &error("first")).expect("a line");
    events.emit(0, &error("second")).expect("a line");
    append(events.path(), CUT);
    let read = events.read().expect("the record reads");
    assert_eq!(kinds(&read), ["error", "error"], "{read:#?}");
    assert_eq!(read[1]["error"], "second");
}

/// The story's acceptance: a valid line, then `{"summary":"gr\xc3` cut, then a newline and a
/// valid line, the `skipped` line naming the cut one. Two events, the skipped line named.
#[test]
fn a_line_cut_inside_a_character_and_named_reads_as_two_events() {
    let dir = scratch("acceptance");
    let events = Events::new(&dir);
    events.emit(0, &error("first")).expect("a line");
    append(events.path(), b"{\"summary\":\"gr\xc3\n");
    append(
        events.path(),
        br#"{"at":"2026-10-18T00:00:01Z","step":0,"event":"skipped","line":2,"reason":"the line was cut short, without its end, and is skipped"}
"#,
    );
    let read = events.read().expect("the record reads");
    assert_eq!(kinds(&read), ["error", "skipped"], "{read:#?}");
    assert_eq!(read[1]["line"], 2);
}

/// The next write after the cut ends the line and names it, as it does for any torn line; the
/// record reads on, the line in the middle skipped and named.
#[test]
fn a_line_cut_inside_a_character_in_the_middle_is_ended_named_and_skipped() {
    let dir = scratch("middle");
    let events = Events::new(&dir);
    events.emit(0, &error("first")).expect("a line");
    append(events.path(), CUT);
    events
        .emit(1, &error("third"))
        .expect("a line after the cut");
    let read = events.read().expect("the record reads");
    assert_eq!(kinds(&read), ["error", "skipped", "error"], "{read:#?}");
    assert_eq!(read[1]["line"], 2);
    assert_eq!(read[2]["error"], "third");
}

/// A whole line that is not valid UTF-8 and that nothing names was not cut short: it is an
/// error, and the error names that line, not the whole file.
#[test]
fn an_unnamed_line_that_is_not_utf8_is_an_error_naming_the_line() {
    let dir = scratch("unnamed");
    let events = Events::new(&dir);
    events.emit(0, &error("first")).expect("a line");
    append(events.path(), b"{\"summary\":\"gr\xc3\n");
    events.emit(1, &error("third")).expect("a line");
    let failure = format!(
        "{:#}",
        events.read().expect_err("the line was not cut short")
    );
    assert!(
        failure.contains("line 2 of"),
        "the error does not name the line: {failure}"
    );
}

/// The scorecard reads a case whose record holds a line cut inside a character.
#[test]
fn the_scorecard_reads_a_record_cut_inside_a_character() {
    let repo = scratch("scorecard");
    let dir = repo.join(CASES_DIR).join("scenario-1-trial-1");
    fs::create_dir_all(&dir).expect("a case directory");
    // A scenario case's outcome reads its held-out result, which the runner writes after the run.
    fs::write(dir.join("held-out.txt"), "PASS\n").expect("a held-out result");
    let events = Events::new(&dir);
    let opened = Event::Opened {
        case: "scenario-1-trial-1".to_owned(),
        protocol: "story.delivery/1".to_owned(),
        story: Some("story:scratch-one".to_owned()),
        inputs: Vec::new(),
        base: None,
        binary: None,
        layers: None,
    };
    events.emit(0, &opened).expect("the run opens");
    append(events.path(), CUT);
    events.emit(0, &opened).expect("the resumed run opens");
    events
        .emit(
            0,
            &Event::Stop {
                stop: "NoProgress".to_owned(),
                steps: 0,
                tokens: 0,
                completed: None,
            },
        )
        .expect("the resumed run stops");
    let card = Scorecard::read(&repo).expect("the scorecard reads");
    assert_eq!(card.rows.len(), 2, "{:?}", card.rows);
    assert!(card.no_record.is_empty(), "{:?}", card.no_record);
}

fn git(tree: &Path, args: &[&str]) {
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
    let dir = scratch(name);
    copy_dir(&Path::new(FIXTURES).join("one-ready"), &dir);
    fs::write(dir.join("tests-only.yaml"), TESTS_ONLY).expect("the protocol");
    fs::write(dir.join("check.txt"), "broken\n").expect("a file");
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "start"]);
    dir
}

fn options(protocol: &str) -> RunOptions {
    RunOptions {
        protocol: protocol.to_owned(),
        intent: String::new(),
        inputs: Vec::new(),
        iterations: Some(4),
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

fn store_evidence(repo: &Path, kind: &str) -> usize {
    let output = Command::new("aep")
        .args(["plan", "artifact", "history", STORY, "--format", "json"])
        .current_dir(repo)
        .output()
        .expect("aep runs");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let entries: Vec<serde_json::Value> =
        serde_json::from_slice(&output.stdout).expect("the history is json");
    entries
        .iter()
        .filter(|entry| entry["change"]["change"] == "evidence" && entry["change"]["kind"] == kind)
        .count()
}

/// A case whose record holds a line cut inside a character runs on to `accepted`, and
/// `record_accepted` reads the record and writes the story's `test_result`.
#[test]
fn an_accepted_case_with_a_line_cut_inside_a_character_is_recorded() {
    let repo = repository("accepted");
    let store = Aep::default();
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(TestsRun::new("true").expect("a command"))];
    let protocol = repo.join("tests-only.yaml").display().to_string();
    let mut out = Vec::new();
    let opened = Run::open_next(
        &repo,
        options(&protocol),
        &store,
        &NoHarness,
        &validators,
        &mut out,
    )
    .expect("the frontier answers")
    .expect("story:ready-one is taken");
    let id = opened.case.id.clone();
    let dir = opened.case.dir.clone();
    drop(opened);
    append(&dir.join(EVENTS_FILE), CUT);

    let mut run =
        Run::resume(&repo, options(&protocol), &id, &NoHarness, &validators).expect("resumes");
    run.record_on(&store);
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let text = String::from_utf8_lossy(&out).into_owned();
    assert!(
        matches!(stop, Ok(Stop::Completed(ref outcome)) if outcome == "accepted"),
        "{stop:?}\n{text}"
    );
    assert_eq!(
        store_evidence(&repo, "test_result"),
        1,
        "record_accepted wrote no test_result on the story:\n{text}"
    );
    let read = Events::new(&dir).read().expect("the record reads");
    assert!(
        read.iter().any(|line| line["event"] == "skipped"),
        "{read:#?}"
    );
    assert_eq!(
        read.last().expect("a line")["event"],
        "stop",
        "{read:#?}\n{text}"
    );
}
