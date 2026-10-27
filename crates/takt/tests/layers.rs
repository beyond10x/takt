//! story:a-trial-records-the-revision-of-each-layer (ADR 0101): a run's `opened` line carries
//! a `layers` map, layer name to revision, each read from the binary or library that ran: takt's
//! own build revision. The harness layer is the harness program's own version as its stream
//! stated it on the run's harness steps, and the model layer the model id the stream reported
//! there: takt runs the harness program in a judgement step and nowhere else, never for its
//! version, so neither layer is on the `opened` line and a reader joins them from the step
//! lines. The scorecard prints the layers of the runs it scores and refuses to score runs whose
//! layers differ, naming the layer, unless asked to compare across them; a record without
//! `layers` reads as not recorded.

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::anyhow;
use takt::case::CASES_DIR;
use takt::events::{EVENTS_FILE, Event, Events, TokenUsage};
use takt::frontier::Aep;
use takt::harness::{ClaudeCode, Harness, StepContext, StepReport};
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

/// A harness the runs never reach.
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

fn opened_lines(case_dir: &Path) -> Vec<serde_json::Value> {
    let text = fs::read_to_string(case_dir.join(EVENTS_FILE)).expect("events.jsonl");
    text.lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("one object per line"))
        .filter(|line| line["event"] == "opened")
        .collect()
}

/// A run driven through the library records on its `opened` line the layers it can read as it
/// opens: takt as the binary revision its caller gave, and no other. The harness is not run at
/// open, so its layer is not there, never guessed; a run given no binary writes the map empty.
#[test]
fn the_opened_line_records_the_layers_the_run_can_read() {
    let repo = repository("run");
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(TestsRun::new("true").expect("a command"))];
    let protocol = repo.join("tests-only.yaml").display().to_string();
    let mut out = Vec::new();
    let harness = NoHarness;
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
        serde_json::json!({"takt": given}),
        "takt's own revision, and no harness layer before a harness step: {opened:#?}"
    );

    let mut run = Run::resume(
        &repo,
        options(&protocol),
        dir.file_name().and_then(|id| id.to_str()).expect("an id"),
        &harness,
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
        "no binary: the map is written and holds no layer: {opened:#?}"
    );
}

/// A fake `claude` in `dir`: each call counts itself in `dir/calls` and writes its arguments
/// one per line to `dir/args-<n>`, prints an init event stating `version` as Claude Code's
/// stream does (`claude_code_version`), writes a test file and reports `tests.write`.
fn fake_claude(dir: &Path, version: &str) -> ClaudeCode {
    let script = r#"#!/bin/sh
d='@DIR@'
n=$(cat "$d/calls" 2>/dev/null || echo 0)
n=$((n + 1))
echo "$n" > "$d/calls"
printf '%s\n' "$@" > "$d/args-$n"
printf '%s\n' '{"type":"system","subtype":"init","claude_code_version":"@VERSION@"}'
mkdir -p tests
echo "$n" >> tests/work.txt
printf '%s\n' '{"type":"result","subtype":"success","result":"{\"action\": \"tests.write\", \"summary\": \"wrote a test\"}","usage":{"input_tokens":3,"output_tokens":4},"num_turns":2}'
"#
    .replace("@DIR@", &dir.display().to_string())
    .replace("@VERSION@", version);
    let program = dir.join("claude");
    fs::write(&program, script).expect("a script");
    fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).expect("exec bit");
    ClaudeCode {
        program: program.display().to_string(),
        ..ClaudeCode::default()
    }
}

/// The harness layer is read from the harness's own stream at the step that ran it, never
/// from a call of the program outside a step: the `opened` line holds takt's revision alone,
/// the harness step's line states the version its init event stated, the program ran once
/// (the step), and the scorecard joins the layer from the step line.
#[test]
fn the_harness_layer_is_the_version_the_steps_stream_stated() {
    let tree = scratch("stream-tree");
    fs::write(tree.join("README.md"), "a story\n").expect("a file");
    git(&tree, &["init", "-q", "-b", "main"]);
    git(&tree, &["add", "-A"]);
    git(&tree, &["commit", "-q", "-m", "start"]);
    let scripts = scratch("stream-scripts");
    let harness = fake_claude(&scripts, "2.1.303");
    let validators: Vec<Box<dyn Validator>> = Vec::new();
    let options = RunOptions {
        protocol: "software.change/1".to_owned(),
        intent: "record the layers".to_owned(),
        inputs: vec![
            ("risk".to_owned(), "standard".to_owned()),
            ("change_kind".to_owned(), "bugfix".to_owned()),
            ("affects_security_boundary".to_owned(), "false".to_owned()),
        ],
        iterations: Some(1),
        hours: None,
        tokens: None,
        case_id: Some("versioned".to_owned()),
        gate: Some("repository.merge".to_owned()),
    };
    let mut run = Run::open(&tree, options, &harness, &validators).expect("the case opens");
    run.binary = Some(TAKT_A.to_owned());
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let text = String::from_utf8_lossy(&out).into_owned();
    stop.unwrap_or_else(|error| panic!("the run runs: {error:#}\n{text}"));
    let dir = run.case.dir.clone();
    drop(run);
    let lines = Events::new(&dir).read().expect("the run record");
    let opened: Vec<&serde_json::Value> = lines
        .iter()
        .filter(|line| line["event"] == "opened")
        .collect();
    assert_eq!(opened.len(), 1, "{lines:#?}");
    assert_eq!(
        opened[0]["layers"],
        serde_json::json!({"takt": TAKT_A}),
        "the harness is not run at open, so its layer is not there: {lines:#?}"
    );
    let steps: Vec<&serde_json::Value> = lines
        .iter()
        .filter(|line| line["event"] == "step" && line["by"] == "harness")
        .collect();
    assert_eq!(steps.len(), 1, "{lines:#?}");
    assert_eq!(
        steps[0]["version"],
        serde_json::json!("2.1.303"),
        "the step line states the version the stream stated: {lines:#?}"
    );
    let calls = fs::read_to_string(scripts.join("calls")).expect("the calls count");
    assert_eq!(
        calls.trim(),
        "1",
        "the program ran for the step and for nothing else"
    );
    let args = fs::read_to_string(scripts.join("args-1")).expect("the arguments");
    assert!(
        !args.lines().any(|arg| arg == "--version"),
        "never asked for its version: {args}"
    );
    let card = Scorecard::read(&tree).expect("the scorecard reads");
    let row = card
        .rows
        .iter()
        .find(|row| row.case == "versioned")
        .unwrap_or_else(|| panic!("the run's row: {card:#?}"));
    assert_eq!(
        row.layers,
        Some(layers(&[("takt", TAKT_A), ("harness", "2.1.303")])),
        "the harness layer joined from the step line: {card:#?}"
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
                confinement: None,
                endpoint: None,
                records: None,
                scan: None,
                preflight: None,
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
                version: None,
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
