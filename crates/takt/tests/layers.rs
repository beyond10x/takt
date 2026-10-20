//! story:a-trial-records-the-revision-of-each-layer (ADR 0101): a run's `opened` line carries
//! a `layers` map, layer name to revision, each read from the binary or library that ran: takt's
//! own build revision and the harness's version as the harness reports it; the model layer is
//! the model id the harness's stream reported on the run's steps. The scorecard prints the
//! layers of the runs it scores and refuses to score runs whose layers differ, naming the layer,
//! unless asked to compare across them; a record without `layers` reads as not recorded.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::anyhow;
use takt::case::CASES_DIR;
use takt::events::{EVENTS_FILE, Event, Events, TokenUsage};
use takt::frontier::Aep;
use takt::harness::{Harness, StepContext, StepReport};
use takt::scorecard::Scorecard;
use takt::validator::{TestsRun, Validator};
use takt::{Run, RunOptions};

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");

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

fn scratch(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("layers-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(&dir).expect("a scratch tree");
    Scratch(dir)
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

/// A scratch repository on `main` holding a committed copy of the `one-ready` store and the
/// tests-only protocol.
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

/// A harness the runs never reach, which reports the version it runs as.
struct Versioned(Option<&'static str>);

impl Harness for Versioned {
    fn name(&self) -> &str {
        "versioned"
    }

    fn revision(&self) -> Option<String> {
        self.0.map(str::to_owned)
    }

    fn step(&self, context: &StepContext<'_>) -> Result<StepReport, anyhow::Error> {
        Err(anyhow!(
            "no harness: a judgement step among {} is left",
            context.offered.join(", ")
        ))
    }
}

fn opened_lines(case_dir: &Path) -> Vec<serde_json::Value> {
    let text = fs::read_to_string(case_dir.join(EVENTS_FILE)).expect("events.jsonl");
    text.lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("one object per line"))
        .filter(|line| line["event"] == "opened")
        .collect()
}

/// A run driven through the library records on its `opened` line the layers it can read: takt
/// as the binary revision its caller gave, the harness as the harness reports its version;
/// a harness that reports none leaves the layer absent, never guessed.
#[test]
fn the_opened_line_records_the_layers_the_run_can_read() {
    let repo = repository("run");
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(TestsRun::new("true").expect("a command"))];
    let protocol = repo.join("tests-only.yaml").display().to_string();
    let mut out = Vec::new();
    let harness = Versioned(Some("2.1.304 (Claude Code)"));
    let mut run = Run::open_next(
        &repo,
        options(&protocol),
        &Aep::default(),
        &harness,
        &validators,
        &mut out,
    )
    .expect("the frontier answers")
    .expect("story:ready-one is taken");
    let given = "0123456789abcdef0123456789abcdef01234567";
    run.binary = Some(given.to_owned());
    run.run(&mut out).expect("the run stops without an error");
    let dir = run.case.dir.clone();
    drop(run);
    let opened = opened_lines(&dir);
    assert_eq!(opened.len(), 1, "{opened:#?}");
    assert_eq!(
        opened[0]["layers"],
        serde_json::json!({"takt": given, "harness": "2.1.304 (Claude Code)"}),
        "{opened:#?}"
    );

    let unversioned = Versioned(None);
    let mut run = Run::resume(
        &repo,
        options(&protocol),
        dir.file_name().and_then(|id| id.to_str()).expect("an id"),
        &unversioned,
        &validators,
    )
    .expect("the case resumes");
    run.run(&mut out).expect("the run stops without an error");
    drop(run);
    let opened = opened_lines(&dir);
    assert_eq!(opened.len(), 2, "{opened:#?}");
    assert_eq!(
        opened[1]["layers"],
        serde_json::json!({}),
        "no binary and no harness version: the map is written and holds no layer: {opened:#?}"
    );
}

fn layers(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(name, revision)| ((*name).to_owned(), (*revision).to_owned()))
        .collect()
}

/// One run of `case` under `repo`: `opened` with `layers`, one harness step reporting `model`,
/// then an accepted stop; the held-out check beside the record passed.
fn record(repo: &Path, case: &str, layers: Option<BTreeMap<String, String>>, model: Option<&str>) {
    let dir = repo.join(CASES_DIR).join(case);
    fs::create_dir_all(&dir).expect("a case directory");
    fs::write(dir.join(takt::scorecard::HELD_OUT_FILE), "PASS\n").expect("the held-out result");
    let events = Events::new(&dir);
    let usage = TokenUsage {
        input: 100,
        output: 10,
        cache_read: 0,
        cache_creation: 0,
    };
    let lines = [
        (
            0,
            Event::Opened {
                case: case.to_owned(),
                protocol: "story.delivery/1".to_owned(),
                story: None,
                inputs: Vec::new(),
                base: None,
                binary: layers
                    .as_ref()
                    .and_then(|layers| layers.get("takt").cloned()),
                layers,
            },
        ),
        (
            1,
            Event::Step {
                by: "harness".to_owned(),
                name: "claude-code".to_owned(),
                action: "implementation.edit".to_owned(),
                summary: "edited".to_owned(),
                tokens: Some(usage.total()),
                usage: Some(usage),
                revision: None,
                model: model.map(str::to_owned),
                cost_usd: None,
                duration_ms: None,
                api_ms: None,
                turns: None,
                started_at: None,
                transcript: None,
                transcript_refused: None,
            },
        ),
        (
            1,
            Event::Stop {
                stop: "Completed (accepted)".to_owned(),
                steps: 1,
                tokens: usage.total(),
                completed: Some(true),
            },
        ),
    ];
    for (step, event) in lines {
        events.emit(step, &event).expect("the event is written");
    }
}

fn scored(repo: &Path, across: bool) -> Result<String, String> {
    let mut out = Vec::new();
    takt::scorecard::report(repo, None, None, None, &[], across, &mut out)
        .map(|_| String::from_utf8(out).expect("utf-8"))
        .map_err(|error| format!("{error:#}"))
}

const TAKT_A: &str = "0123456789abcdef0123456789abcdef01234567";

/// A row's layers are the `opened` line's, the model layer joined from the harness steps'
/// reported model; a record without `layers` reads as not recorded, whatever its steps say.
#[test]
fn a_rows_layers_are_the_opened_lines_and_the_steps_model() {
    let repo = scratch("rows");
    record(
        &repo,
        "scenario-1-trial-1",
        Some(layers(&[
            ("takt", TAKT_A),
            ("harness", "2.1.304 (Claude Code)"),
        ])),
        Some("claude-opus-5-5"),
    );
    record(&repo, "scenario-1-trial-2", None, Some("claude-opus-5-5"));
    let card = Scorecard::read(&repo).expect("the scorecard reads");
    assert_eq!(card.rows.len(), 2, "{card:#?}");
    assert_eq!(
        card.rows[0].layers,
        Some(layers(&[
            ("takt", TAKT_A),
            ("harness", "2.1.304 (Claude Code)"),
            ("model", "claude-opus-5-5"),
        ])),
        "{card:#?}"
    );
    assert_eq!(
        card.rows[1].layers, None,
        "a record without `layers` is not recorded, never a revision: {card:#?}"
    );
    let text = scored(&repo, true).expect("scored across layers");
    assert!(
        text.contains("layers  takt ") && text.contains("harness 2.1.304 (Claude Code)"),
        "{text}"
    );
    assert!(text.contains("model claude-opus-5-5"), "{text}");
    assert!(text.contains("layers  not recorded"), "{text}");
}

/// Runs whose layers differ are refused, naming the layer and both runs; asked to compare
/// across them, they are scored.
#[test]
fn runs_whose_layers_differ_are_refused_naming_the_layer_unless_asked() {
    let repo = scratch("differ");
    record(
        &repo,
        "scenario-1-trial-1",
        Some(layers(&[
            ("takt", TAKT_A),
            ("harness", "2.1.304 (Claude Code)"),
        ])),
        Some("claude-opus-5-5"),
    );
    record(
        &repo,
        "scenario-1-trial-2",
        Some(layers(&[
            ("takt", TAKT_A),
            ("harness", "2.1.305 (Claude Code)"),
        ])),
        Some("claude-opus-5-5"),
    );
    let error = scored(&repo, false).expect_err("refused");
    assert!(error.contains("layer `harness`"), "{error}");
    assert!(
        error.contains("scenario-1-trial-1#1") && error.contains("scenario-1-trial-2#1"),
        "{error}"
    );
    assert!(
        error.contains("2.1.304 (Claude Code)") && error.contains("2.1.305 (Claude Code)"),
        "{error}"
    );
    assert!(error.contains("--across-layers"), "{error}");
    let text = scored(&repo, true).expect("scored across layers");
    assert!(text.contains("2.1.305 (Claude Code)"), "{text}");
}

/// A run that recorded no layers beside one that did is a difference too: the refusal says the
/// run recorded none, and never reads its layers as any revision. Runs that all recorded none
/// are scored as before.
#[test]
fn a_run_without_layers_beside_one_with_them_is_refused() {
    let repo = scratch("mixed");
    record(
        &repo,
        "scenario-1-trial-1",
        Some(layers(&[("takt", TAKT_A)])),
        None,
    );
    record(&repo, "scenario-1-trial-2", None, None);
    let error = scored(&repo, false).expect_err("refused");
    assert!(error.contains("not recorded"), "{error}");
    assert!(error.contains("scenario-1-trial-2#1"), "{error}");

    let before = scratch("before");
    record(&before, "scenario-1-trial-1", None, Some("m"));
    record(&before, "scenario-1-trial-2", None, Some("n"));
    let text = scored(&before, false).expect("runs from before the field are scored");
    assert!(text.contains("layers  not recorded"), "{text}");
}

/// The same layers in a different map order are the same layers; a layer one run states and
/// another does not is named.
#[test]
fn a_layer_one_run_states_and_another_does_not_is_named() {
    let repo = scratch("absent");
    record(
        &repo,
        "scenario-1-trial-1",
        Some(layers(&[
            ("takt", TAKT_A),
            ("harness", "2.1.304 (Claude Code)"),
        ])),
        Some("claude-opus-5-5"),
    );
    record(
        &repo,
        "scenario-1-trial-2",
        Some(layers(&[
            ("takt", TAKT_A),
            ("harness", "2.1.304 (Claude Code)"),
        ])),
        None,
    );
    let error = scored(&repo, false).expect_err("refused");
    assert!(error.contains("layer `model`"), "{error}");
    assert!(error.contains("scenario-1-trial-2#1"), "{error}");
}
