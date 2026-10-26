//! Adversary pass 1 on story:run-record-and-story-evidence: the run record (`events.jsonl`) and
//! the `test_result` evidence and move a run writes on the story.

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use anyhow::anyhow;
use takt::events::EVENTS_FILE;
use takt::forge::Limits;
use takt::frontier::{Aep, Frontier, Story, Take, Written};
use takt::harness::{Harness, StepContext, StepReport};
use takt::validator::{TestsRun, Validator};
use takt::{Run, RunOptions, Stop};

const DELIVERY: &str = "story.delivery/1";
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");
const STORY: &str = "story:ready-one";

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

fn scratch_dir(name: &str) -> PathBuf {
    Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("run-record-adv1-{}-{name}", std::process::id()))
}

fn repository(name: &str) -> Scratch {
    let dir = scratch_dir(name);
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    copy_dir(&Path::new(FIXTURES).join("one-ready"), &dir);
    fs::write(dir.join("tests-only.yaml"), TESTS_ONLY).expect("the protocol");
    fs::write(dir.join("check.txt"), "broken\n").expect("a file");
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "start"]);
    Scratch(dir)
}

fn options(protocol: &str, iterations: u64) -> RunOptions {
    RunOptions {
        protocol: protocol.to_owned(),
        intent: String::new(),
        inputs: Vec::new(),
        iterations: Some(iterations),
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

fn tests_pass() -> Box<dyn Validator> {
    Box::new(TestsRun::new("true").expect("a command"))
}

fn store_evidence(repo: &Path, id: &str, kind: &str) -> Vec<serde_json::Value> {
    let output = Command::new("aep")
        .args(["plan", "artifact", "history", id, "--format", "json"])
        .current_dir(repo)
        .output()
        .expect("aep runs");
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let entries: Vec<serde_json::Value> =
        serde_json::from_slice(&output.stdout).expect("the history is json");
    entries
        .into_iter()
        .filter(|entry| entry["change"]["change"] == "evidence" && entry["change"]["kind"] == kind)
        .collect()
}

/// The lines of the case's `events.jsonl` that read as JSON; a line that does not is skipped.
fn events(case_dir: &Path) -> Vec<serde_json::Value> {
    let text = fs::read_to_string(case_dir.join(EVENTS_FILE)).expect("events.jsonl");
    text.lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect()
}

fn of<'a>(lines: &'a [serde_json::Value], event: &str) -> Vec<&'a serde_json::Value> {
    lines.iter().filter(|line| line["event"] == event).collect()
}

/// An implementation step that records a `test_result` on the story itself, through the store's
/// own verb, as a model with a shell can: `aep plan artifact evidence` in the tree.
struct RecordsItsOwnEvidence;

impl Harness for RecordsItsOwnEvidence {
    fn name(&self) -> &str {
        "records-its-own-evidence"
    }

    fn step(&self, context: &StepContext<'_>) -> Result<StepReport, anyhow::Error> {
        if !context.offered.iter().any(|a| a == "implementation.edit") {
            return Err(anyhow!("implementation.edit not offered"));
        }
        let output = Command::new("aep")
            .args([
                "plan",
                "artifact",
                "evidence",
                STORY,
                "--kind",
                "test_result",
                "--source",
                "takt",
                "--ref",
                "tests/all.txt",
                "--at",
                "2026-10-18T00:00:00Z",
            ])
            .current_dir(context.tree)
            .output()?;
        if !output.status.success() {
            return Err(anyhow!(
                "aep refused: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        Ok(StepReport {
            action: "implementation.edit".to_owned(),
            summary: "the story is done; recorded its tests".to_owned(),
            tokens: Some(3),
        })
    }
}

/// Lead 1: evidence comes from validators only (AGENTS.md). The tests fail; a judgement step
/// writes a `test_result` on the story in the planning store with the store's own verb. The
/// step must be refused and put back; the story must hold no `test_result`.
#[test]
fn a_judgement_step_cannot_record_a_test_result_on_the_story() {
    let repo = repository("forged-store");
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(TestsRun::new("false").expect("a command"))];
    let store = Aep::default();
    let mut out = Vec::new();
    let mut run = Run::open_next(
        &repo,
        options(DELIVERY, 2),
        &store,
        &RecordsItsOwnEvidence,
        &validators,
        &mut out,
    )
    .expect("the frontier answers")
    .expect("story:ready-one is taken");
    run.record_on(&store);
    let stop = run.run(&mut out);
    let text = String::from_utf8_lossy(&out).into_owned();
    let recorded = store_evidence(&repo, STORY, "test_result");
    assert!(
        recorded.is_empty(),
        "the tests failed and a judgement step recorded a test_result on the story, committed as {}: {recorded:#?}\n{stop:?}\n{text}",
        git(&repo, &["log", "-1", "--format=%h %s", "--name-only"])
    );
}

/// A writer that fails once what was written contains `needle`, as a closed standard output
/// does (`takt run | head`).
struct FailsOn {
    written: Vec<u8>,
    needle: &'static str,
}

impl std::io::Write for FailsOn {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.written.extend_from_slice(bytes);
        if String::from_utf8_lossy(&self.written).contains(self.needle) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "the reader went away",
            ));
        }
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Opens the case of `story:ready-one` on the tests-only protocol and runs it with `out`.
fn accepted_run(
    repo: &Path,
    store: &dyn Frontier,
    out: &mut dyn std::io::Write,
) -> (anyhow::Result<Stop>, PathBuf, String) {
    let validators: Vec<Box<dyn Validator>> = vec![tests_pass()];
    let protocol = repo.join("tests-only.yaml").display().to_string();
    let mut opening = Vec::new();
    let mut run = Run::open_next(
        repo,
        options(&protocol, 4),
        store,
        &NoHarness,
        &validators,
        &mut opening,
    )
    .expect("the frontier answers")
    .expect("story:ready-one is taken");
    run.record_on(store);
    let stop = run.run(out);
    (stop, run.case.dir.clone(), run.case.id.clone())
}

/// Lead 5/6: the store's write is made, then the line printed, then the event recorded. When the
/// print fails the record never names the write it made, and the run's record ends on neither
/// `stop` nor `error` (ess takt.yaml: each run "ending with `stop` or `error`").
#[test]
fn a_store_write_the_run_made_is_named_in_the_record() {
    let repo = repository("lost-line");
    let store = Aep::default();
    let mut out = FailsOn {
        written: Vec::new(),
        needle: "test_result recorded on",
    };
    let (stop, dir, _) = accepted_run(&repo, &store, &mut out);
    let lines = events(&dir);
    let in_store = store_evidence(&repo, STORY, "test_result").len();
    assert_eq!(
        of(&lines, "store_evidence").len(),
        in_store,
        "the store holds {in_store} test_result the record does not name; run: {stop:?}; record: {lines:#?}"
    );
    let last = lines.last().expect("a line");
    assert!(
        last["event"] == "stop" || last["event"] == "error",
        "the run ended on {last}"
    );
}

/// Lead 6: the duplicate guard reads only `events.jsonl`. A write whose line was lost is made
/// again when the case is run again: two `test_result` records on the story for one case.
#[test]
fn an_accepted_case_whose_store_line_was_lost_is_not_recorded_twice() {
    let repo = repository("lost-line-twice");
    let store = Aep::default();
    let mut out = FailsOn {
        written: Vec::new(),
        needle: "test_result recorded on",
    };
    let (first, _, id) = accepted_run(&repo, &store, &mut out);
    assert_eq!(store_evidence(&repo, STORY, "test_result").len(), 1);

    let validators: Vec<Box<dyn Validator>> = vec![tests_pass()];
    let protocol = repo.join("tests-only.yaml").display().to_string();
    let mut run =
        Run::resume(&repo, options(&protocol, 4), &id, &NoHarness, &validators).expect("resumes");
    run.record_on(&store);
    let mut out = Vec::new();
    let second = run.run(&mut out);
    let text = String::from_utf8_lossy(&out).into_owned();
    let recorded = store_evidence(&repo, STORY, "test_result");
    assert_eq!(
        recorded.len(),
        1,
        "first run {first:?}; second run {second:?}\n{text}\n{recorded:#?}"
    );
}

/// Lead 5: a line cut short (a write that hit a full disk, a process killed mid-write) is left
/// without its newline; every later line of every later run is appended to it. A run on that
/// case that terminates accepted must still stop accepted, and its record must end on `stop` or
/// `error`.
#[test]
fn a_torn_line_does_not_turn_an_accepted_stop_into_an_unrecorded_error() {
    let repo = repository("torn");
    let store = Aep::default();
    let validators: Vec<Box<dyn Validator>> = vec![tests_pass()];
    let protocol = repo.join("tests-only.yaml").display().to_string();
    let mut out = Vec::new();
    let opened = Run::open_next(
        &repo,
        options(&protocol, 4),
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
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join(EVENTS_FILE))
        .expect("events.jsonl");
    file.write_all(br#"{"at":"2026-10-18T00:00:00Z","step":0,"event":"deci"#)
        .expect("a torn line");
    drop(file);

    let mut run =
        Run::resume(&repo, options(&protocol, 4), &id, &NoHarness, &validators).expect("resumes");
    run.record_on(&store);
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let text = String::from_utf8_lossy(&out).into_owned();
    let lines = events(&dir);
    let last = lines.last().expect("a line");
    assert!(
        last["event"] == "stop" || last["event"] == "error",
        "the run ended on {last}; run: {stop:?}\n{text}"
    );
    assert!(
        matches!(stop, Ok(Stop::Completed(ref outcome)) if outcome == "accepted"),
        "{stop:?}\n{text}"
    );
}

/// A store whose writes go to `slow`, every read and the take to the real `aep`.
struct SlowWrites {
    real: Aep,
    slow: Aep,
}

impl Frontier for SlowWrites {
    fn name(&self) -> &str {
        "slow-writes"
    }

    fn candidates(&self, repo: &Path) -> anyhow::Result<Vec<String>> {
        self.real.candidates(repo)
    }

    fn active(&self, repo: &Path) -> anyhow::Result<Vec<String>> {
        self.real.active(repo)
    }

    fn show(&self, repo: &Path, id: &str) -> anyhow::Result<Story> {
        self.real.show(repo, id)
    }

    fn take(&self, repo: &Path, id: &str) -> anyhow::Result<Take> {
        self.real.take(repo, id)
    }

    fn record_approval(
        &self,
        _: &Path,
        story: &str,
        _: &takt::case::Approval,
    ) -> anyhow::Result<()> {
        panic!("an approval was recorded on {story}")
    }

    fn record_test_result(
        &self,
        repo: &Path,
        story: &str,
        reference: &str,
        at: &str,
    ) -> anyhow::Result<Written> {
        self.slow.record_test_result(repo, story, reference, at)
    }

    fn implemented(&self, repo: &Path, story: &str) -> anyhow::Result<Written> {
        self.slow.implemented(repo, story)
    }
}

/// Lead 3: an `aep` that hangs, or is not there, is held to its limit and relayed; the stop is
/// unchanged and the record ends on it.
#[test]
fn a_hanging_or_missing_aep_leaves_the_stop_and_ends_the_record() {
    let bin = scratch_dir("bin");
    fs::create_dir_all(&bin).expect("a directory");
    let hang = bin.join("aep-hangs");
    fs::write(&hang, "#!/bin/sh\nsleep 30\n").expect("a script");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(&hang, fs::Permissions::from_mode(0o755)).expect("executable");
    }
    for (name, command) in [
        ("hang", hang.display().to_string()),
        ("missing", bin.join("no-such-aep").display().to_string()),
    ] {
        let repo = repository(name);
        let store = SlowWrites {
            real: Aep::default(),
            slow: Aep {
                command,
                limits: Limits {
                    timeout: Duration::from_secs(2),
                    output: 1 << 20,
                },
            },
        };
        let started = Instant::now();
        let mut out = Vec::new();
        let (stop, dir, _) = accepted_run(&repo, &store, &mut out);
        let text = String::from_utf8_lossy(&out).into_owned();
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "{name}: {text}"
        );
        assert!(
            matches!(stop, Ok(Stop::Completed(ref outcome)) if outcome == "accepted"),
            "{name}: {stop:?}\n{text}"
        );
        let lines = events(&dir);
        assert_eq!(of(&lines, "refusal").len(), 2, "{name}: {lines:#?}");
        assert_eq!(lines.last().expect("a line")["event"], "stop", "{name}");
        assert_eq!(
            git(
                &repo,
                &["status", "--porcelain", "--", ".", ":!.engineering/cases"]
            ),
            "",
            "{name}"
        );
    }
    let _ = fs::remove_dir_all(&bin);
}
