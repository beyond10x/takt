//! Adversary pass 2 on story:run-record-and-story-evidence: the store guard around a judgement
//! step, both-or-neither for a store write and its line, and the record's end.

#[path = "support/own_home.rs"]
mod own_home;

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use anyhow::anyhow;
use takt::events::{EVENTS_FILE, Event, Events};
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

fn sh(tree: &Path, script: &str) -> anyhow::Result<()> {
    let output = Command::new("sh")
        .args(["-c", script])
        .current_dir(tree)
        .output()?;
    if !output.status.success() {
        return Err(anyhow!(
            "{script}: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
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
        .join(format!("run-record-adv2-{}-{name}", std::process::id()))
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

fn events(case_dir: &Path) -> Vec<serde_json::Value> {
    let text = fs::read_to_string(case_dir.join(EVENTS_FILE)).expect("events.jsonl");
    text.lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect()
}

/// An `implementation.edit` step that runs `script` in the tree the first time it is offered,
/// and changes nothing after.
struct Scripted {
    name: &'static str,
    script: &'static str,
    done: std::sync::atomic::AtomicBool,
}

impl Scripted {
    fn new(name: &'static str, script: &'static str) -> Self {
        Self {
            name,
            script,
            done: std::sync::atomic::AtomicBool::new(false),
        }
    }
}

impl Harness for Scripted {
    fn name(&self) -> &str {
        self.name
    }

    fn step(&self, context: &StepContext<'_>) -> Result<StepReport, anyhow::Error> {
        if !context.offered.iter().any(|a| a == "implementation.edit") {
            return Err(anyhow!("implementation.edit not offered"));
        }
        if !self.done.swap(true, std::sync::atomic::Ordering::SeqCst) {
            sh(context.tree, self.script)?;
        }
        Ok(StepReport {
            action: "implementation.edit".to_owned(),
            summary: "edited the check".to_owned(),
            tokens: Some(1),
        })
    }
}

/// Runs the delivery protocol on `story:ready-one` with failing tests and `harness`.
fn delivery_run(repo: &Path, harness: &dyn Harness) -> (anyhow::Result<Stop>, PathBuf, String) {
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(TestsRun::new("false").expect("a command"))];
    let store = Aep::default();
    let mut out = Vec::new();
    let mut run = Run::open_next(
        repo,
        options(DELIVERY, 2),
        &store,
        harness,
        &validators,
        &mut out,
    )
    .expect("the frontier answers")
    .expect("story:ready-one is taken");
    run.record_on(&store);
    run.continue_past_red_base();
    let stop = run.run(&mut out);
    (
        stop,
        run.case.dir.clone(),
        String::from_utf8_lossy(&out).into_owned(),
    )
}

/// The committed tree's changes under `.engineering/` since the first commit, cases excluded.
fn store_committed(repo: &Path) -> String {
    let first = git(repo, &["rev-list", "--max-parents=0", "HEAD"]);
    git(
        repo,
        &[
            "diff",
            "--stat",
            &first,
            "HEAD",
            "--",
            ".engineering",
            ":!.takt/cases",
        ],
    )
}

/// The fixture has no `.engineering/evidence/` yet, as a store with no evidence recorded does
/// not. The step records a `test_result` on the story into `shadow/` and plants
/// `.engineering/evidence` as a link to it. The guard snapshots the link's path, finds no
/// directory there before or after, and sees no change; `aep` reads the evidence through the
/// link.
#[test]
fn a_store_directory_planted_as_a_link_is_refused() {
    let repo = repository("evidence-link");
    let harness = Scripted::new(
        "links-the-evidence",
        "aep plan artifact evidence story:ready-one --kind test_result --source takt \
         --ref tests/all.txt --at 2026-10-18T00:00:00Z >/dev/null \
         && mv .engineering/evidence shadow && ln -s ../shadow .engineering/evidence \
         && echo fixed > check.txt",
    );
    let (stop, _, text) = delivery_run(&repo, &harness);
    let recorded = store_evidence(&repo, STORY, "test_result");
    assert!(
        recorded.is_empty(),
        "a judgement step recorded a test_result on the story through a link, committed: {}\n{stop:?}\n{text}",
        store_committed(&repo)
    );
}

/// `.engineering/project.yaml` says where the plan is kept. The step moves the store to a
/// SQLite file beside it, outside `.engineering/planning/` and `.engineering/evidence/`, and
/// makes the story `implemented` there with a `test_result` of its own.
#[test]
fn a_step_that_moves_the_store_elsewhere_is_refused() {
    let repo = repository("store-moved");
    let harness = Scripted::new(
        "moves-the-store",
        "sed -i 's/^  git: {}$/  sqlite: { path: plan.sqlite3 }/' .engineering/project.yaml \
         && aep plan artifact new story ready-one --title 'Make the check pass' >/dev/null \
         && aep plan artifact move story:ready-one --to proposed --executor agent:x >/dev/null \
         && aep plan artifact move story:ready-one --to active --executor agent:x >/dev/null \
         && aep plan artifact evidence story:ready-one --kind test_result --source takt \
            --ref tests/all.txt --at 2026-10-18T00:00:00Z >/dev/null \
         && aep plan artifact move story:ready-one --to implemented --executor agent:x >/dev/null \
         && echo fixed > check.txt",
    );
    let (stop, _, text) = delivery_run(&repo, &harness);
    let recorded = store_evidence(&repo, STORY, "test_result");
    assert!(
        recorded.is_empty(),
        "a judgement step moved the store and recorded a test_result on the story, committed: {}\n{stop:?}\n{text}",
        store_committed(&repo)
    );
}

/// The guard's snapshot keeps bytes, not modes: a step that makes a store file executable
/// changes the store and is committed.
#[test]
fn a_mode_change_in_the_store_is_refused() {
    let repo = repository("store-mode");
    let harness = Scripted::new(
        "chmods-the-store",
        "chmod +x .engineering/planning/story/ready-one.md && echo fixed > check.txt",
    );
    let (stop, _, text) = delivery_run(&repo, &harness);
    let mode = git(
        &repo,
        &[
            "ls-tree",
            "HEAD",
            "--",
            ".engineering/planning/story/ready-one.md",
        ],
    );
    assert!(
        mode.starts_with("100644"),
        "a judgement step changed a store file's mode and it was committed: {mode}\n{stop:?}\n{text}"
    );
}

/// Opens `fifo` for writing without waiting and closes it, so a reader blocked on it sees its
/// end; false when nobody reads it.
#[cfg(target_os = "linux")]
fn release_reader(fifo: &Path) -> bool {
    use std::os::unix::fs::OpenOptionsExt as _;
    const O_NONBLOCK: i32 = 0o4000;
    fs::OpenOptions::new()
        .write(true)
        .custom_flags(O_NONBLOCK)
        .open(fifo)
        .is_ok()
}

/// The guard reads every file under the store after the step. A step that leaves a FIFO there
/// stops the run on reading it, with nobody to write it: the run never ends.
#[cfg(target_os = "linux")]
#[test]
fn a_fifo_in_the_store_does_not_hang_the_run() {
    let repo = repository("store-fifo");
    let harness = Scripted::new(
        "plants-a-fifo",
        "mkdir -p .engineering/planning/x && mkfifo .engineering/planning/x/pipe \
         && echo fixed > check.txt",
    );
    let fifo = repo.join(".engineering/planning/x/pipe");
    let started = Instant::now();
    let mut released = 0;
    let (stop, text) = std::thread::scope(|scope| {
        let running = scope.spawn(|| {
            let (stop, _, text) = delivery_run(&repo, &harness);
            (stop, text)
        });
        while !running.is_finished() {
            std::thread::sleep(Duration::from_millis(100));
            if started.elapsed() > Duration::from_secs(10) && release_reader(&fifo) {
                released += 1;
            }
        }
        running.join().expect("the run does not panic")
    });
    assert_eq!(
        released,
        0,
        "the run blocked reading a FIFO the step planted in the store; released {released} time(s) after {:?}\n{stop:?}\n{text}",
        started.elapsed()
    );
}

/// A file in the store the run cannot read fails the guard's snapshot after the step. The run
/// ends on that error with the step's change still in the tree: neither committed nor put back.
#[cfg(unix)]
#[test]
fn an_unreadable_file_in_the_store_leaves_nothing_of_the_step() {
    let repo = repository("store-unreadable");
    let harness = Scripted::new(
        "locks-a-store-file",
        "echo fixed > check.txt && mkdir -p .engineering/evidence \
         && echo x > .engineering/evidence/locked && chmod 000 .engineering/evidence/locked",
    );
    let (stop, dir, text) = delivery_run(&repo, &harness);
    let left = git(
        &repo,
        &["status", "--porcelain", "--", ".", ":!.takt/cases"],
    );
    let last = events(&dir).last().cloned().expect("a line");
    assert!(
        left.is_empty(),
        "the step's change was left in the tree: {left}; record ends on {last}\n{stop:?}\n{text}"
    );
}

/// A store whose `test_result` write succeeds and then takes `events.jsonl` away from the run:
/// the line for the write cannot be written.
struct RecordLost {
    real: Aep,
    events: std::sync::Mutex<Option<PathBuf>>,
}

impl Frontier for RecordLost {
    fn name(&self) -> &str {
        "record-lost"
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
        let written = self.real.record_test_result(repo, story, reference, at)?;
        if let Some(path) = self.events.lock().expect("a lock").as_ref() {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt as _;
                fs::set_permissions(path, fs::Permissions::from_mode(0o444))?;
            }
        }
        Ok(written)
    }

    fn implemented(&self, repo: &Path, story: &str) -> anyhow::Result<Written> {
        self.real.implemented(repo, story)
    }
}

/// Both or neither: the store write is committed, its line cannot be written, and the commit
/// is taken back with nothing of the write left in the tree.
#[cfg(unix)]
#[test]
fn a_store_write_whose_line_cannot_be_written_is_taken_back() {
    let repo = repository("line-lost");
    let store = RecordLost {
        real: Aep::default(),
        events: std::sync::Mutex::new(None),
    };
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(TestsRun::new("true").expect("a command"))];
    let protocol = repo.join("tests-only.yaml").display().to_string();
    let mut opening = Vec::new();
    let mut run = Run::open_next(
        &repo,
        options(&protocol, 4),
        &store,
        &NoHarness,
        &validators,
        &mut opening,
    )
    .expect("the frontier answers")
    .expect("story:ready-one is taken");
    *store.events.lock().expect("a lock") = Some(run.case.dir.join(EVENTS_FILE));
    let branch = run.case.branch.clone();
    run.record_on(&store);
    let before = git(&repo, &["rev-parse", "HEAD"]);
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(
            run.case.dir.join(EVENTS_FILE),
            fs::Permissions::from_mode(0o644),
        )
        .expect("writable again");
    }
    let text = String::from_utf8_lossy(&out).into_owned();
    assert!(stop.is_err(), "{stop:?}\n{text}");
    assert_eq!(
        git(&repo, &["rev-parse", &format!("refs/heads/{branch}")]),
        before,
        "{stop:?}"
    );
    assert_eq!(
        git(
            &repo,
            &["status", "--porcelain", "--", ".", ":!.takt/cases"]
        ),
        ""
    );
    assert!(store_evidence(&repo, STORY, "test_result").is_empty());
}

/// A step that commits by itself and whose lines cannot be printed is discarded whole: `HEAD`
/// back where it was, nothing left, the record ending on `error`.
#[test]
fn a_step_that_cannot_be_printed_is_discarded_with_its_commit() {
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
    let repo = repository("unprinted-step");
    let start = git(&repo, &["rev-parse", "HEAD"]);
    let harness = Scripted::new(
        "commits-itself",
        "echo fixed > check.txt && git -c user.name=t -c user.email=t@localhost \
         commit -qam 'by the step'",
    );
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(TestsRun::new("false").expect("a command"))];
    let store = Aep::default();
    let mut opening = Vec::new();
    let mut run = Run::open_next(
        &repo,
        options(DELIVERY, 2),
        &store,
        &harness,
        &validators,
        &mut opening,
    )
    .expect("the frontier answers")
    .expect("story:ready-one is taken");
    run.record_on(&store);
    let taken = git(&repo, &["rev-parse", "HEAD"]);
    let mut out = FailsOn {
        written: Vec::new(),
        needle: "(commits-itself)",
    };
    run.continue_past_red_base();
    let stop = run.run(&mut out);
    assert!(stop.is_err(), "{stop:?}");
    assert_ne!(taken, start);
    assert_eq!(git(&repo, &["rev-parse", "HEAD"]), taken, "{stop:?}");
    assert_eq!(
        git(
            &repo,
            &["status", "--porcelain", "--", ".", ":!.takt/cases"]
        ),
        ""
    );
    let lines = events(&run.case.dir);
    assert_eq!(
        lines.last().expect("a line")["event"],
        "error",
        "{lines:#?}"
    );
}

/// A harness that panics: the run's record must still end on `stop` or `error`, and the case
/// must not stay held.
struct Panics;

impl Harness for Panics {
    fn name(&self) -> &str {
        "panics"
    }

    fn step(&self, _: &StepContext<'_>) -> Result<StepReport, anyhow::Error> {
        panic!("the harness broke")
    }
}

#[test]
fn a_harness_that_panics_still_ends_the_record() {
    let repo = repository("panics");
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(TestsRun::new("false").expect("a command"))];
    let store = Aep::default();
    let mut opening = Vec::new();
    let mut run = Run::open_next(
        &repo,
        options(DELIVERY, 2),
        &store,
        &Panics,
        &validators,
        &mut opening,
    )
    .expect("the frontier answers")
    .expect("story:ready-one is taken");
    run.record_on(&store);
    let mut out = Vec::new();
    run.continue_past_red_base();
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run.run(&mut out)));
    assert!(caught.is_err(), "the harness panicked");
    let lines = events(&run.case.dir);
    let last = lines.last().expect("a line");
    let held = run.case.dir.join("run.lock").exists();
    assert!(
        (last["event"] == "stop" || last["event"] == "error") && !held,
        "the run's record ends on {last}; run.lock left: {held}"
    );
}

/// The torn-line repair is itself one write. Cut short too (the process killed again while it
/// wrote), the first torn line is followed by a torn `skipped` line, and the record no longer
/// reads, whatever is appended after.
#[test]
fn a_torn_repair_does_not_make_the_record_unreadable() {
    let dir = scratch_dir("torn-twice");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("a directory");
    let _scratch = Scratch(dir.clone());
    let events = Events::new(&dir);
    events
        .emit(
            0,
            &Event::Error {
                error: "one".to_owned(),
            },
        )
        .expect("a line");
    let mut file = fs::OpenOptions::new()
        .append(true)
        .open(dir.join(EVENTS_FILE))
        .expect("events.jsonl");
    file.write_all(br#"{"at":"2026-10-18T00:00:00Z","step":0,"event":"deci"#)
        .expect("a torn line");
    file.write_all(b"\n{\"at\":\"2026-10-18T00:00:01Z\",\"step\":0,\"ev")
        .expect("a torn repair");
    drop(file);
    events
        .emit(
            0,
            &Event::Error {
                error: "two".to_owned(),
            },
        )
        .expect("a line");
    let read = events.read();
    assert!(
        read.is_ok(),
        "{read:?}\n{}",
        fs::read_to_string(dir.join(EVENTS_FILE)).expect("events.jsonl")
    );
}

/// The guard reads the whole store twice around every judgement step. With 1,000 files in the
/// store, a run of steps costs at most a second more per step than with none.
#[test]
fn a_store_of_a_thousand_files_adds_little_to_a_step() {
    let mut took = Vec::new();
    for files in [0_usize, 1000] {
        let repo = repository(&format!("bulk-{files}"));
        if files > 0 {
            let bulk = repo.join(".engineering/evidence/bulk");
            fs::create_dir_all(&bulk).expect("a directory");
            for n in 0..files {
                fs::write(bulk.join(format!("{n}.json")), vec![b'x'; 4096]).expect("a file");
            }
            git(&repo, &["add", "-A"]);
            git(&repo, &["commit", "-q", "-m", "bulk"]);
        }
        let harness = Scripted::new("edits", "echo fixed > check.txt");
        let started = Instant::now();
        let (stop, _, text) = delivery_run(&repo, &harness);
        took.push(started.elapsed());
        assert!(text.contains("(edits)"), "{stop:?}\n{text}");
    }
    eprintln!("store 0 files: {:?}; 1000 files: {:?}", took[0], took[1]);
    assert!(
        took[1] < took[0] + Duration::from_secs(2),
        "0 files {:?}, 1000 files {:?}",
        took[0],
        took[1]
    );
}
