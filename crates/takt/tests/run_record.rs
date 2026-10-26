//! story:run-record-and-story-evidence: a run leaves `events.jsonl` in its case directory, one
//! line per evaluation, step, evidence record and stop, appended across the case's runs; and a
//! run on a story whose case terminates `accepted` records the `test_result` pass record a tests
//! validator wrote as `test_result` evidence on the story, source takt, at the record's own
//! `observed_at`, and moves the story to `implemented` through the store's verbs. Both writes are
//! commits takt makes on the case's own branch, store paths only, just before the merge when the
//! case has a pull request, so both ride to the base with it. A refusal of the store is relayed
//! verbatim and the stop is unchanged.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;
use std::time::Duration;

use anyhow::anyhow;
use takt::case::CASES_DIR;
use takt::events::EVENTS_FILE;
use takt::evidence::Record;
use takt::forge::{Checks, Forge, Memory};
use takt::frontier::{Aep, Frontier, Story, Take, Written, approve};
use takt::harness::{Harness, StepContext, StepReport};
use takt::validator::{ChangeRead, CiRead, Observation, PrMerged, PrOpen, TestsRun, Validator};
use takt::{Run, RunOptions, Stop};

/// The delivery protocol, the engineering protocols' built-in.
const DELIVERY: &str = "story.delivery/1";

/// The fixture stores, one directory each, relative to this crate.
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");

/// The story the fixture store holds ready.
const STORY: &str = "story:ready-one";

/// One validator-owned action, `tests.run`; accepted once the tests pass.
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

/// A scratch tree under this crate's build directory, removed when dropped; a removal that
/// fails fails the case, unless the case is already failing.
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

/// A scratch repository on `main` holding a committed copy of the `one-ready` store, the
/// protocol above as `tests-only.yaml` and a `check.txt` the story's work changes.
fn repository(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("run-record-{}-{name}", std::process::id()));
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

/// The options of a run on `protocol` with a step budget of `iterations`.
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

/// A harness the runs never reach: every step they take is a validator's.
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

/// A harness that requests the merge gate as soon as it is offered, and otherwise hands the
/// change over (`change.handover`), changing nothing either way.
struct Requester;

impl Harness for Requester {
    fn name(&self) -> &str {
        "requester"
    }

    fn step(&self, context: &StepContext<'_>) -> Result<StepReport, anyhow::Error> {
        if context.approvals.contains(&"repository.merge".to_owned()) {
            return Ok(StepReport {
                action: "repository.merge".to_owned(),
                summary: "the story holds; merge it".to_owned(),
                tokens: Some(7),
            });
        }
        // The work committed is complete: hand it over, changing nothing.
        if context.offered.contains(&"change.handover".to_owned()) {
            return Ok(StepReport {
                action: "change.handover".to_owned(),
                summary: "the story holds; hand it over".to_owned(),
                tokens: Some(7),
            });
        }
        Err(anyhow!(
            "the gate was not offered; a judgement step among {} is left",
            context.offered.join(", ")
        ))
    }
}

/// A harness that reports `tests.write` and changes a path that is no test: a step the kernel
/// refuses and puts back.
struct Overreach;

impl Harness for Overreach {
    fn name(&self) -> &str {
        "overreach"
    }

    fn step(&self, context: &StepContext<'_>) -> Result<StepReport, anyhow::Error> {
        fs::write(context.tree.join("check.txt"), "changed by a test step\n")?;
        Ok(StepReport {
            action: "tests.write".to_owned(),
            summary: "wrote a test".to_owned(),
            tokens: Some(11),
        })
    }
}

/// The real tests validator, its command `true`: one `test_result` pass at the revision.
fn tests_pass() -> Box<dyn Validator> {
    Box::new(TestsRun::new("true").expect("a command"))
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

/// The validators of a delivery run through `forge`, in the order the binary registers them.
fn delivery(forge: Rc<dyn Forge>) -> Vec<Box<dyn Validator>> {
    vec![
        tests_pass(),
        Box::new(GatePass),
        Box::new(PrOpen::new(forge.clone())),
        Box::new(CiRead::new(forge.clone())),
        Box::new(PrMerged::new(forge)),
        Box::new(ChangeRead),
    ]
}

/// The status the store in `repo` lists `id` at.
fn status(repo: &Path, id: &str) -> String {
    Aep::default()
        .list(repo)
        .expect("the store lists")
        .into_iter()
        .find(|a| a.id == id)
        .map(|a| a.status)
        .unwrap_or_else(|| panic!("{id} is listed"))
}

/// `aep plan artifact history <id> --format json` in `repo`: its evidence entries of `kind`.
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

/// The lines of the case's `events.jsonl`, each one JSON object.
fn events(case_dir: &Path) -> Vec<serde_json::Value> {
    let text = fs::read_to_string(case_dir.join(EVENTS_FILE)).expect("events.jsonl");
    text.lines()
        .map(|line| serde_json::from_str(line).expect("one object per line"))
        .collect()
}

/// The lines of `lines` whose event is `event`.
fn of<'a>(lines: &'a [serde_json::Value], event: &str) -> Vec<&'a serde_json::Value> {
    lines.iter().filter(|line| line["event"] == event).collect()
}

/// The files in `dir`, a directory of the case.
fn files(dir: &Path) -> usize {
    fs::read_dir(dir).map_or(0, |entries| entries.count())
}

/// The subjects of the commits on `branch` that `base` does not hold, newest first.
fn subjects(repo: &Path, base: &str, branch: &str) -> Vec<String> {
    git(repo, &["log", "--format=%s", &format!("{base}..{branch}")])
        .lines()
        .map(str::to_owned)
        .collect()
}

/// `story:ready-one` taken by a run on the tests-only protocol whose tests validator passes:
/// the run given the store, its stop, what it printed and its case directory.
fn tests_only_run(repo: &Path, frontier: &dyn Frontier) -> (Stop, String, PathBuf) {
    let validators: Vec<Box<dyn Validator>> = vec![tests_pass()];
    let protocol = repo.join("tests-only.yaml").display().to_string();
    let mut out = Vec::new();
    let mut run = Run::open_next(
        repo,
        options(&protocol, 4),
        frontier,
        &NoHarness,
        &validators,
        &mut out,
    )
    .expect("the frontier answers")
    .expect("story:ready-one is taken");
    run.record_on(frontier);
    let stop = run.run(&mut out).expect("the run stops without an error");
    (
        stop,
        String::from_utf8_lossy(&out).into_owned(),
        run.case.dir.clone(),
    )
}

/// The acceptance: a run on a story unit in which the tests validator wrote a pass record
/// leaves `events.jsonl` in the case directory with one line per evaluation, step, evidence
/// record and stop, each with its instant and step.
#[test]
fn a_story_run_whose_tests_pass_records_a_line_per_evaluation_step_evidence_and_stop() {
    let repo = repository("acceptance");
    let (stop, text, dir) = tests_only_run(&repo, &Aep::default());
    assert_eq!(stop, Stop::Completed("accepted".to_owned()), "{text}");

    let lines = events(&dir);
    assert_eq!(
        of(&lines, "decision").len(),
        files(&dir.join("decisions")),
        "{lines:#?}"
    );
    let steps = of(&lines, "step");
    assert_eq!(steps.len(), 1, "{lines:#?}");
    assert_eq!(steps[0]["by"], "validator");
    assert_eq!(steps[0]["name"], "takt.tests-run");
    assert_eq!(steps[0]["action"], "tests.run");
    assert_eq!(steps[0]["step"], 1);
    let evidence = of(&lines, "evidence");
    assert_eq!(evidence.len(), files(&dir.join("evidence")), "{lines:#?}");
    assert_eq!(evidence[0]["kind"], "test_result");
    assert_eq!(evidence[0]["result"], "pass");
    let path = evidence[0]["path"].as_str().expect("a path");
    assert!(repo.join(path).is_file(), "{path}");
    assert!(path.starts_with(CASES_DIR), "{path}");
    let decisions = of(&lines, "decision");
    for (n, decision) in decisions.iter().enumerate() {
        assert_eq!(decision["seq"], n + 1);
        let file = decision["file"].as_str().expect("a file");
        assert!(dir.join(file).is_file(), "{file}");
    }
    assert_eq!(of(&lines, "stop").len(), 1, "{lines:#?}");
    let last = lines.last().expect("a line");
    assert_eq!(last["event"], "stop");
    assert_eq!(last["stop"], "Completed (accepted)");
    assert_eq!(last["steps"], 1);
    assert_eq!(lines[0]["event"], "opened");
    assert_eq!(lines[0]["story"], STORY);
    for line in &lines {
        assert!(
            line["at"]
                .as_str()
                .is_some_and(|at| at.len() == 27 && at.ends_with('Z') && at.as_bytes()[19] == b'.'),
            "{line}"
        );
        assert!(line["step"].is_u64(), "{line}");
    }
}

/// story:binary-embeds-revision: the revision of the binary the caller gives the run is the
/// `opened` line's `binary`; a run given none writes no `binary`, as a line written before the
/// field was added has none, and such a line reads as it did.
#[test]
fn the_opened_line_records_the_binary_revision_the_caller_gives() {
    let given = "0123456789abcdef0123456789abcdef01234567-dirty";
    let repo = repository("binary");
    let validators: Vec<Box<dyn Validator>> = vec![tests_pass()];
    let protocol = repo.join("tests-only.yaml").display().to_string();
    let mut out = Vec::new();
    let mut run = Run::open_next(
        &repo,
        options(&protocol, 4),
        &Aep::default(),
        &NoHarness,
        &validators,
        &mut out,
    )
    .expect("the frontier answers")
    .expect("story:ready-one is taken");
    run.binary = Some(given.to_owned());
    run.run(&mut out).expect("the run stops without an error");
    let dir = run.case.dir.clone();
    drop(run);
    let lines = events(&dir);
    assert_eq!(lines[0]["event"], "opened", "{lines:#?}");
    assert_eq!(lines[0]["binary"], given, "{lines:#?}");

    let mut run = Run::resume(
        &repo,
        options(&protocol, 1),
        dir.file_name().and_then(|id| id.to_str()).expect("an id"),
        &NoHarness,
        &validators,
    )
    .expect("the case resumes");
    run.run(&mut out).expect("the run stops without an error");
    drop(run);
    let lines = events(&dir);
    let opened = of(&lines, "opened");
    assert_eq!(opened.len(), 2, "{lines:#?}");
    assert!(
        opened[1].get("binary").is_none(),
        "a run given no revision writes none: {lines:#?}"
    );
}

/// The first check: the store's history holds one `test_result` record on the story whose
/// instant is the case's record's `observed_at`, whose source names takt and whose reference
/// is the record's path in the repository.
#[test]
fn the_story_holds_one_test_result_at_the_records_own_instant_from_takt() {
    let repo = repository("check-one");
    let (stop, text, dir) = tests_only_run(&repo, &Aep::default());
    assert_eq!(stop, Stop::Completed("accepted".to_owned()), "{text}");

    let lines = events(&dir);
    let path = of(&lines, "evidence")[0]["path"]
        .as_str()
        .expect("a path")
        .to_owned();
    let record: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(repo.join(&path)).expect("the record"))
            .expect("json");
    let observed_at = record["observed_at"].as_str().expect("an instant");

    let recorded = store_evidence(&repo, STORY, "test_result");
    assert_eq!(recorded.len(), 1, "{recorded:#?}\n{text}");
    assert_eq!(recorded[0]["at"], observed_at, "{}", recorded[0]);
    assert!(
        recorded[0]["change"]["source"]
            .as_str()
            .is_some_and(|source| source.contains("takt")),
        "{}",
        recorded[0]
    );
    assert_eq!(recorded[0]["change"]["reference"], path.as_str());
    assert_eq!(recorded[0]["actor"], "agent:takt", "{}", recorded[0]);
    let written = of(&lines, "store_evidence");
    assert_eq!(written.len(), 1, "{lines:#?}");
    assert_eq!(written[0]["observed_at"], observed_at);
    assert_eq!(written[0]["reference"], path.as_str());
    assert!(
        text.contains("store: test_result recorded on story:ready-one"),
        "{text}"
    );
}

/// The second check: a story whose case terminated `accepted` is `implemented` afterwards. Both
/// writes are commits on the case's branch, store paths only, the implementation revision
/// unchanged, the base untouched and the tree clean.
#[test]
fn a_story_whose_case_terminated_accepted_is_implemented() {
    let repo = repository("check-two");
    let (stop, text, dir) = tests_only_run(&repo, &Aep::default());
    assert_eq!(stop, Stop::Completed("accepted".to_owned()), "{text}");
    assert_eq!(status(&repo, STORY), "implemented", "{text}");

    let id = dir
        .file_name()
        .expect("a name")
        .to_string_lossy()
        .into_owned();
    let branch = format!("takt/{id}");
    assert_eq!(git(&repo, &["branch", "--show-current"]), branch);
    assert_eq!(
        subjects(&repo, "main", &branch),
        [
            "takt: implemented story:ready-one",
            "takt: test_result story:ready-one",
            "takt: take story:ready-one"
        ]
    );
    for subject in ["HEAD", "HEAD^"] {
        let changed = git(
            &repo,
            &["diff-tree", "--no-commit-id", "--name-only", "-r", subject],
        );
        assert!(
            changed.lines().all(|path| {
                path.starts_with(".engineering/planning/")
                    || path.starts_with(".engineering/evidence/story/ready-one/")
            }),
            "{subject}: {changed}"
        );
    }
    // The implementation revision, read from the case's own snapshot, is still the take.
    let take = git(&repo, &["rev-parse", "HEAD~2"]);
    let snapshot = fs::read_to_string(dir.join("case.yaml")).expect("case.yaml");
    assert!(
        snapshot.contains(&format!("revision: {take}")),
        "{snapshot}"
    );
    assert_eq!(status_on(&repo, "main"), "proposed");
    assert_eq!(
        git(
            &repo,
            &[
                "status",
                "--porcelain",
                "--untracked-files=all",
                "--",
                ".",
                ":!.engineering/cases"
            ]
        ),
        ""
    );
    let lines = events(&dir);
    let moved = of(&lines, "store_move");
    assert_eq!(moved.len(), 1);
    assert_eq!(moved[0]["to"], "implemented");
}

/// The status of `story:ready-one` in the store as `revision` holds it, read without a checkout.
fn status_on(repo: &Path, revision: &str) -> String {
    let text = git(
        repo,
        &[
            "show",
            &format!("{revision}:.engineering/planning/story/ready-one.md"),
        ],
    );
    text.lines()
        .find_map(|line| line.strip_prefix("status: "))
        .expect("a status")
        .to_owned()
}

/// With a forge, the store is written just before the merge the grant authorises, and rides
/// with it: the pull request's head and the head merged are the granted revision with the
/// approval's commit, the test result and the move above it, the move last, and nothing is
/// written after the merge. The run at the gate writes nothing to the store.
#[test]
fn with_a_forge_the_store_is_written_before_the_merge_and_rides_with_it() {
    let repo = repository("forge");
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
        options(DELIVERY, 12),
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
    let worked = git(&repo, &["rev-parse", "HEAD"]);

    let mut run = Run::resume(&repo, options(DELIVERY, 12), &id, &Requester, &validators)
        .expect("the case resumes");
    run.record_on(&store);
    let mut out = Vec::new();
    let stop = run.run(&mut out).expect("the run stops without an error");
    let text = String::from_utf8_lossy(&out).into_owned();
    drop(run);
    assert_eq!(
        stop,
        Stop::ApprovalRequired(vec!["repository.merge".to_owned()]),
        "{text}"
    );
    assert!(
        store_evidence(&repo, STORY, "test_result").is_empty(),
        "{text}"
    );
    assert_eq!(git(&repo, &["rev-parse", "HEAD"]), worked, "{text}");
    let gate = events(&dir);
    assert_eq!(of(&gate, "store_evidence").len(), 0);
    // The one harness step hands the change over; once only the grant stands between the case
    // and its outcome the run stops with no further harness step (story:no-model-step-for-approval).
    let harness: Vec<&serde_json::Value> = of(&gate, "step")
        .into_iter()
        .filter(|line| line["by"] == "harness")
        .collect();
    assert_eq!(harness.len(), 1, "{gate:#?}");
    assert_eq!(harness[0]["action"], "change.handover");
    assert_eq!(harness[0]["tokens"], 7);
    assert_eq!(of(&gate, "stop")[0]["tokens"], 7, "{gate:#?}");

    approve(&repo, &store, &id, "repository.merge", "human:operator")
        .expect("the approval is given");
    let approved = git(&repo, &["rev-parse", "HEAD"]);
    let mut run = Run::resume(&repo, options(DELIVERY, 12), &id, &NoHarness, &validators)
        .expect("the case resumes");
    run.record_on(&store);
    let mut out = Vec::new();
    let stop = run.run(&mut out).expect("the run stops without an error");
    let text = String::from_utf8_lossy(&out).into_owned();
    drop(run);
    assert_eq!(stop, Stop::Completed("accepted".to_owned()), "{text}");

    assert_eq!(memory.merged.borrow().as_slice(), [1], "{text}");
    let branch = format!("takt/{id}");
    assert_eq!(
        memory.pushed.borrow().get(&branch).cloned(),
        Some(git(&repo, &["rev-parse", "HEAD"])),
        "{text}"
    );
    assert_eq!(
        store_evidence(&repo, STORY, "test_result").len(),
        1,
        "{text}"
    );
    assert_eq!(status(&repo, STORY), "implemented", "{text}");
    assert_eq!(git(&repo, &["rev-parse", "HEAD~2"]), approved);
    assert_eq!(
        subjects(&repo, &approved, &branch),
        [
            "takt: implemented story:ready-one",
            "takt: test_result story:ready-one"
        ]
    );
    // Both runs appended to one record: two opened lines, one stop each.
    let lines = events(&dir);
    assert_eq!(of(&lines, "opened").len(), 2, "{lines:#?}");
    assert_eq!(of(&lines, "stop").len(), 2, "{lines:#?}");
    assert_eq!(
        of(&lines, "decision").len(),
        files(&dir.join("decisions")),
        "{lines:#?}"
    );
}

/// Without a forge a delivery case never terminates `accepted`: a run whose tests pass writes
/// nothing to the store and adds no commit to the case's branch.
#[test]
fn a_run_that_does_not_terminate_accepted_writes_nothing_to_the_store() {
    let repo = repository("no-forge");
    let validators: Vec<Box<dyn Validator>> = vec![tests_pass(), Box::new(GatePass)];
    let store = Aep::default();
    let mut out = Vec::new();
    let mut run = Run::open_next(
        &repo,
        options(DELIVERY, 3),
        &store,
        &NoHarness,
        &validators,
        &mut out,
    )
    .expect("the frontier answers")
    .expect("story:ready-one is taken");
    run.record_on(&store);
    let head = git(&repo, &["rev-parse", "HEAD"]);
    let stop = run.run(&mut out);
    let text = String::from_utf8_lossy(&out).into_owned();
    let dir = run.case.dir.clone();
    drop(run);
    assert!(!matches!(stop, Ok(Stop::Completed(_))), "{stop:?}\n{text}");
    let lines = events(&dir);
    assert!(
        of(&lines, "evidence")
            .iter()
            .any(|line| line["kind"] == "test_result" && line["result"] == "pass"),
        "{lines:#?}"
    );
    assert!(store_evidence(&repo, STORY, "test_result").is_empty());
    assert_eq!(status(&repo, STORY), "active");
    assert_eq!(git(&repo, &["rev-parse", "HEAD"]), head);
    assert_eq!(
        lines.last().expect("a line")["event"],
        match stop {
            Ok(_) => "stop",
            Err(_) => "error",
        }
    );
}

/// A pass record no validator of the case wrote (put into the case's evidence by hand) is never
/// recorded on the story: the run says so, the store refuses the move for want of a record, and
/// both refusals are relayed with the stop unchanged.
#[test]
fn a_record_no_validator_wrote_is_not_recorded_and_the_refused_move_is_relayed() {
    let repo = repository("unobserved");
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
    drop(opened);
    let mut run =
        Run::resume(&repo, options(&protocol, 4), &id, &NoHarness, &validators).expect("resumes");
    let revision = run.case.revision("implementation").expect("a revision");
    run.case
        .add_evidence(&Record::new(
            "by-hand",
            "test_result",
            Some("pass".to_owned()),
            "implementation",
            revision,
            "by-hand",
        ))
        .expect("the record is written");
    run.record_on(&store);
    let mut out = Vec::new();
    let stop = run.run(&mut out).expect("the run stops without an error");
    let text = String::from_utf8_lossy(&out).into_owned();
    let dir = run.case.dir.clone();
    drop(run);
    assert_eq!(stop, Stop::Completed("accepted".to_owned()), "{text}");
    assert!(
        store_evidence(&repo, STORY, "test_result").is_empty(),
        "{text}"
    );
    assert_eq!(status(&repo, STORY), "active", "{text}");
    let lines = events(&dir);
    let refusals = of(&lines, "refusal");
    assert_eq!(refusals.len(), 2, "{lines:#?}");
    assert_eq!(refusals[0]["by"], "takt");
    assert_eq!(refusals[1]["by"], "store");
    let reason = refusals[1]["reason"].as_str().expect("a reason");
    assert!(reason.contains("test_result"), "{reason}");
    assert!(
        text.contains(&format!(
            "store: {STORY} not moved to implemented: {reason}"
        )),
        "{text}"
    );
    assert_eq!(
        lines.last().expect("a line")["stop"],
        "Completed (accepted)"
    );
}

/// A store that records the test result and refuses the move, in its own words.
struct RefusingMove(Aep);

impl Frontier for RefusingMove {
    fn name(&self) -> &str {
        "refusing-move"
    }

    fn candidates(&self, repo: &Path) -> anyhow::Result<Vec<String>> {
        self.0.candidates(repo)
    }

    fn active(&self, repo: &Path) -> anyhow::Result<Vec<String>> {
        self.0.active(repo)
    }

    fn show(&self, repo: &Path, id: &str) -> anyhow::Result<Story> {
        self.0.show(repo, id)
    }

    fn take(&self, repo: &Path, id: &str) -> anyhow::Result<Take> {
        self.0.take(repo, id)
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
        self.0.record_test_result(repo, story, reference, at)
    }

    fn implemented(&self, _: &Path, _: &str) -> anyhow::Result<Written> {
        Ok(Written::Refused("lifecycle: not today".to_owned()))
    }
}

/// A move the store refuses is relayed verbatim on the output and in the record, not retried,
/// and the stop is the one the case reached.
#[test]
fn a_refused_move_is_relayed_verbatim_and_the_stop_is_unchanged() {
    let repo = repository("refused-move");
    let frontier = RefusingMove(Aep::default());
    let (stop, text, dir) = tests_only_run(&repo, &frontier);
    assert_eq!(stop, Stop::Completed("accepted".to_owned()), "{text}");
    assert_eq!(
        text.matches("store: story:ready-one not moved to implemented: lifecycle: not today")
            .count(),
        1,
        "{text}"
    );
    let lines = events(&dir);
    let refusals = of(&lines, "refusal");
    assert_eq!(refusals.len(), 1, "{lines:#?}");
    assert_eq!(refusals[0]["by"], "store");
    assert_eq!(refusals[0]["reason"], "lifecycle: not today");
    assert_eq!(of(&lines, "store_evidence").len(), 1);
    assert_eq!(lines.last().expect("a line")["event"], "stop");
    assert_eq!(status(&repo, STORY), "active");
}

/// A case accepted and run again records nothing twice: the store holds one `test_result` and
/// the second run adds no commit.
#[test]
fn an_accepted_case_run_again_records_nothing_twice() {
    let repo = repository("again");
    let store = Aep::default();
    let (stop, text, dir) = tests_only_run(&repo, &store);
    assert_eq!(stop, Stop::Completed("accepted".to_owned()), "{text}");
    let id = dir
        .file_name()
        .expect("a name")
        .to_string_lossy()
        .into_owned();
    let head = git(&repo, &["rev-parse", "HEAD"]);
    let validators: Vec<Box<dyn Validator>> = vec![tests_pass()];
    let protocol = repo.join("tests-only.yaml").display().to_string();
    let mut run =
        Run::resume(&repo, options(&protocol, 4), &id, &NoHarness, &validators).expect("resumes");
    run.record_on(&store);
    let mut out = Vec::new();
    let stop = run.run(&mut out).expect("the run stops without an error");
    let text = String::from_utf8_lossy(&out).into_owned();
    drop(run);
    assert_eq!(stop, Stop::Completed("accepted".to_owned()), "{text}");
    assert_eq!(
        store_evidence(&repo, STORY, "test_result").len(),
        1,
        "{text}"
    );
    assert_eq!(git(&repo, &["rev-parse", "HEAD"]), head, "{text}");
    let lines = events(&dir);
    assert_eq!(of(&lines, "store_evidence").len(), 1);
    assert_eq!(of(&lines, "store_move").len(), 1);
    assert_eq!(of(&lines, "stop").len(), 2);
}

/// A judgement step the kernel refuses puts the case directory back, and the record keeps the
/// step and the refusal: they are written after it is put back. An intent's run has no story
/// and writes nothing to a store.
#[test]
fn a_refused_judgement_step_is_in_the_record_after_the_case_is_put_back() {
    let repo = repository("refused-step");
    let validators: Vec<Box<dyn Validator>> = Vec::new();
    let mut run = Run::open(
        &repo,
        RunOptions {
            protocol: "software.change/1".to_owned(),
            intent: "fix the check".to_owned(),
            inputs: vec![
                ("risk".to_owned(), "standard".to_owned()),
                ("change_kind".to_owned(), "bugfix".to_owned()),
                ("affects_security_boundary".to_owned(), "false".to_owned()),
            ],
            iterations: Some(2),
            hours: None,
            tokens: None,
            case_id: Some("refused-step".to_owned()),
            gate: None,
        },
        &Overreach,
        &validators,
    )
    .expect("the case opens");
    let mut out = Vec::new();
    let stop = run.run(&mut out).expect("the run stops without an error");
    let text = String::from_utf8_lossy(&out).into_owned();
    assert!(matches!(stop, Stop::StepRefused(_)), "{stop:?}\n{text}");
    let lines = events(&run.case.dir);
    let kinds: Vec<&str> = lines
        .iter()
        .map(|line| line["event"].as_str().expect("an event"))
        .collect();
    assert_eq!(
        kinds,
        ["opened", "decision", "step", "refusal", "stop"],
        "{text}"
    );
    assert_eq!(lines[2]["by"], "harness");
    assert_eq!(lines[2]["name"], "overreach");
    assert_eq!(lines[2]["tokens"], 11);
    assert_eq!(lines[3]["by"], "takt");
    assert!(
        lines[3]["reason"]
            .as_str()
            .is_some_and(|reason| reason.contains("tests.write changed a non-test path")),
        "{}",
        lines[3]
    );
    assert!(lines[0].get("story").is_none());
}

/// A protocol whose one judgement action writes, about an artifact that is no `implementation`.
const NO_IMPLEMENTATION: &str = "format: protocol/1

protocol:
  id: no.implementation
  revision: 1
  description: >-
    A report written by a judgement step; the case declares no implementation artifact.

artifacts:
  report:
    description: a report

evidence_kinds:
  report_result:
    description: whether the report reads

claims:
  report.reads:
    description: the report reads
    true_when:
      evidence:
        kind: report_result
        result: reads
        subject: report

actions:
  report.write:
    description: write the report
    effect: write

outcomes:
  accepted:
    description: the report reads
    requires:
      claim: report.reads
";

/// A harness that writes `report.txt` and reports `report.write` with 4321 tokens.
struct Reporter;

impl Harness for Reporter {
    fn name(&self) -> &str {
        "reporter"
    }

    fn step(&self, context: &StepContext<'_>) -> Result<StepReport, anyhow::Error> {
        fs::write(context.tree.join("report.txt"), "the report\n")?;
        Ok(StepReport {
            action: "report.write".to_owned(),
            summary: "wrote the report".to_owned(),
            tokens: Some(4321),
        })
    }
}

/// A step whose change is committed in a case that declares no `implementation` artifact ends
/// the run with an error, and the step's line, with the tokens the harness spent, is in
/// `events.jsonl` all the same: the tokens a step spent are recorded whatever ends it.
#[test]
fn a_step_that_ends_the_run_with_an_error_keeps_its_tokens() {
    let repo = repository("no-implementation");
    let protocol = repo.join("no-implementation.yaml");
    fs::write(&protocol, NO_IMPLEMENTATION).expect("the protocol");
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "protocol"]);
    let validators: Vec<Box<dyn Validator>> = Vec::new();
    let mut run = Run::open(
        &repo,
        RunOptions {
            protocol: protocol.to_str().expect("utf-8").to_owned(),
            intent: "write the report".to_owned(),
            inputs: Vec::new(),
            iterations: Some(2),
            hours: None,
            tokens: None,
            case_id: Some("no-implementation".to_owned()),
            gate: None,
        },
        &Reporter,
        &validators,
    )
    .expect("the case opens");
    let mut out = Vec::new();
    let ended = run.run(&mut out);
    let text = String::from_utf8_lossy(&out).into_owned();
    assert!(ended.is_err(), "{ended:?}\n{text}");
    let lines = events(&run.case.dir);
    let steps = of(&lines, "step");
    assert_eq!(steps.len(), 1, "{lines:?}\n{text}");
    assert_eq!(steps[0]["by"], "harness");
    assert_eq!(steps[0]["action"], "report.write");
    assert_eq!(steps[0]["tokens"], 4321);
}

/// A run whose checks stay pending records each wait and stops `Waiting`.
#[test]
fn each_wait_on_the_forge_is_a_line_and_the_waiting_stop_is_the_last() {
    let repo = repository("waiting");
    let memory = Rc::new(Memory {
        checks: Checks::Pending,
        ..Memory::default()
    });
    let forge: Rc<dyn Forge> = memory.clone();
    let validators = delivery(forge);
    let store = Aep::default();
    let mut out = Vec::new();
    let opened = Run::open_next(
        &repo,
        options(DELIVERY, 6),
        &store,
        &Requester,
        &validators,
        &mut out,
    )
    .expect("the frontier answers")
    .expect("story:ready-one is taken");
    let id = opened.case.id.clone();
    drop(opened);
    fs::write(repo.join("check.txt"), "fixed\n").expect("a file");
    git(&repo, &["commit", "-q", "-am", "the work"]);
    let mut run = Run::resume(&repo, options(DELIVERY, 7), &id, &Requester, &validators)
        .expect("the case resumes");
    run.poll = Duration::from_millis(5);
    run.record_on(&store);
    let mut out = Vec::new();
    let stop = run.run(&mut out).expect("the run stops without an error");
    let text = String::from_utf8_lossy(&out).into_owned();
    assert!(matches!(stop, Stop::Waiting { .. }), "{stop:?}\n{text}");
    let lines = events(&run.case.dir);
    let waits = of(&lines, "waiting");
    assert_eq!(waits.len(), text.matches("  waiting: ").count(), "{text}");
    assert!(!waits.is_empty(), "{text}");
    assert_eq!(waits[0]["on"][0], "ci.read");
    let last = lines.last().expect("a line");
    assert_eq!(last["event"], "stop");
    assert!(
        last["stop"]
            .as_str()
            .is_some_and(|stop| stop.starts_with("Waiting (ci.read")),
        "{last}"
    );
    assert!(store_evidence(&repo, STORY, "test_result").is_empty());
}

/// A judgement step that edits a story's file in the planning store by hand, here moving it
/// to `implemented`, is refused and put back: only takt writes the store, each write its own
/// commit.
struct EditsTheStore;

impl Harness for EditsTheStore {
    fn name(&self) -> &str {
        "edits-the-store"
    }

    fn step(&self, context: &StepContext<'_>) -> Result<StepReport, anyhow::Error> {
        let path = context
            .tree
            .join(".engineering/planning/story/ready-one.md");
        let text = fs::read_to_string(&path)?;
        fs::write(&path, text.replace("status: active", "status: implemented"))?;
        Ok(StepReport {
            action: "implementation.edit".to_owned(),
            summary: "marked the story implemented".to_owned(),
            tokens: None,
        })
    }
}

#[test]
fn a_judgement_step_that_edits_the_planning_store_is_refused_and_put_back() {
    let repo = repository("edits-store");
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(TestsRun::new("false").expect("a command"))];
    let store = Aep::default();
    let mut out = Vec::new();
    let mut run = Run::open_next(
        &repo,
        options(DELIVERY, 3),
        &store,
        &EditsTheStore,
        &validators,
        &mut out,
    )
    .expect("the frontier answers")
    .expect("story:ready-one is taken");
    run.record_on(&store);
    let head = git(&repo, &["rev-parse", "HEAD"]);
    let stop = run.run(&mut out).expect("the run stops without an error");
    let text = String::from_utf8_lossy(&out).into_owned();
    let dir = run.case.dir.clone();
    drop(run);
    let Stop::StepRefused(why) = &stop else {
        panic!("{stop:?}\n{text}");
    };
    assert!(
        why.contains("wrote into the planning store")
            && why.contains(".engineering/planning/story/ready-one.md"),
        "{why}"
    );
    assert_eq!(status(&repo, STORY), "active", "{text}");
    assert_eq!(git(&repo, &["rev-parse", "HEAD"]), head);
    assert_eq!(
        git(
            &repo,
            &["status", "--porcelain", "--", ".", ":!.engineering/cases"]
        ),
        ""
    );
    let lines = events(&dir);
    let refusals = of(&lines, "refusal");
    assert_eq!(refusals.len(), 1, "{lines:#?}");
    assert_eq!(refusals[0]["by"], "takt");
    assert_eq!(lines.last().expect("a line")["event"], "stop");
}

/// A judgement step that writes the commit identity file
/// ([`takt::git::COMMIT_IDENTITY_FILE`]) is refused and put back, and the refusal names the file
/// as the commit identity, never as the planning store, whose sentence names takt's own writes
/// (story:commit-identity-refusal-names-the-identity-file).
struct WritesTheIdentity;

impl Harness for WritesTheIdentity {
    fn name(&self) -> &str {
        "writes-the-identity"
    }

    fn step(&self, context: &StepContext<'_>) -> Result<StepReport, anyhow::Error> {
        fs::write(
            context.tree.join(takt::git::COMMIT_IDENTITY_FILE),
            "[identity]\n\tname = step\n\temail = step@localhost\n",
        )?;
        Ok(StepReport {
            action: "implementation.edit".to_owned(),
            summary: "an identity".to_owned(),
            tokens: None,
        })
    }
}

#[test]
fn a_judgement_step_that_writes_the_commit_identity_is_refused_as_the_identity() {
    let repo = repository("writes-identity");
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(TestsRun::new("false").expect("a command"))];
    let store = Aep::default();
    let mut out = Vec::new();
    let mut run = Run::open_next(
        &repo,
        options(DELIVERY, 3),
        &store,
        &WritesTheIdentity,
        &validators,
        &mut out,
    )
    .expect("the frontier answers")
    .expect("story:ready-one is taken");
    run.record_on(&store);
    let head = git(&repo, &["rev-parse", "HEAD"]);
    let stop = run.run(&mut out).expect("the run stops without an error");
    let text = String::from_utf8_lossy(&out).into_owned();
    drop(run);
    let Stop::StepRefused(why) = &stop else {
        panic!("{stop:?}\n{text}");
    };
    assert!(
        why.contains("commit identity") && why.contains(takt::git::COMMIT_IDENTITY_FILE),
        "{why}"
    );
    assert!(!why.contains("planning store"), "{why}");
    assert_eq!(git(&repo, &["rev-parse", "HEAD"]), head);
    assert!(
        !repo.join(takt::git::COMMIT_IDENTITY_FILE).exists(),
        "{text}"
    );
}

/// A line of the record that does not read and was not cut short (a hand edit in the middle of
/// the file) does not turn an accepted case into a run that errs: the store's writes are not
/// made, an `error` line says why, and the stop is the one the case reached, recorded last.
#[test]
fn an_unreadable_line_is_an_error_line_and_the_accepted_stop_is_kept() {
    let repo = repository("unreadable");
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
    fs::write(dir.join(EVENTS_FILE), "not an event\n").expect("a line");

    let mut run =
        Run::resume(&repo, options(&protocol, 4), &id, &NoHarness, &validators).expect("resumes");
    run.record_on(&store);
    let mut out = Vec::new();
    let stop = run.run(&mut out).expect("the run keeps its stop");
    let text = String::from_utf8_lossy(&out).into_owned();
    assert_eq!(stop, Stop::Completed("accepted".to_owned()), "{text}");
    let raw = fs::read_to_string(dir.join(EVENTS_FILE)).expect("events.jsonl");
    let lines: Vec<serde_json::Value> = raw
        .lines()
        .skip(1)
        .map(|line| serde_json::from_str(line).expect("one object per line"))
        .collect();
    let errors = of(&lines, "error");
    assert_eq!(errors.len(), 1, "{lines:#?}");
    assert!(
        errors[0]["error"]
            .as_str()
            .is_some_and(|error| error.contains("line 1") && error.contains("does not read")),
        "{}",
        errors[0]
    );
    assert_eq!(
        lines.last().expect("a line")["stop"],
        "Completed (accepted)"
    );
    assert!(store_evidence(&repo, STORY, "test_result").is_empty());
}

/// A line cut short is ended by the next line written, which names it as skipped; the record
/// reads on, every line after it whole.
#[test]
fn a_torn_line_is_ended_named_and_skipped() {
    let scratch = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("run-record-{}-torn-unit", std::process::id()));
    let _ = fs::remove_dir_all(&scratch);
    fs::create_dir_all(&scratch).expect("a directory");
    let events = takt::events::Events::new(&scratch);
    events
        .emit(
            0,
            &takt::events::Event::Error {
                error: "first".to_owned(),
            },
        )
        .expect("a line");
    let mut text = fs::read_to_string(events.path()).expect("the file");
    text.push_str(r#"{"at":"2026-10-18T00:00:00Z","ev"#);
    fs::write(events.path(), &text).expect("a torn line");
    assert_eq!(events.read().expect("reads").len(), 1);
    events
        .emit(
            1,
            &takt::events::Event::Error {
                error: "third".to_owned(),
            },
        )
        .expect("a line");
    let read = events.read().expect("reads");
    let kinds: Vec<&str> = read
        .iter()
        .map(|line| line["event"].as_str().expect("an event"))
        .collect();
    assert_eq!(kinds, ["error", "skipped", "error"], "{read:#?}");
    assert_eq!(read[1]["line"], 2);
    assert_eq!(read[2]["error"], "third");
    let _ = fs::remove_dir_all(&scratch);
}
