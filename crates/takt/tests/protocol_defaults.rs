//! story:takt-ships-default-protocol-settings. takt ships one defaults document, as data: the
//! protocol a store story opens on (`story.delivery/3`) and the value of each declared input
//! nobody sets (`tests_first: required`, `gate_on: after_handover`). Canon declares those inputs
//! and gives them no default, so a case whose input nothing sets is refused at open, naming the
//! input and its values, and the reference page of the defaults is generated from the document
//! and checked against the site's copy.
//!
//! To rewrite the page after the document changes, run this test with
//! `TAKT_WRITE_DEFAULTS_PAGE=1`.

#[path = "support/own_home.rs"]
mod own_home;

use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use takt::case::{CaseDir, CommitAs, settings_of};
use takt::evidence::Record;
use takt::harness::{Harness, StepContext, StepReport};
use takt::protocol::{
    LoadedProtocol, Settings, SettingsRequest, Source, default_protocol, defaults_page,
    list_settings, resolve_settings,
};
use takt::run_config::ProtocolSettings;
use takt::validator::{GateRun, Observation, TestsRun, Validator};
use takt::{Run, RunOptions, Stop};

/// A scratch directory under this crate's build directory, removed on drop.
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

fn git(tree: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(args)
        .current_dir(tree)
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?}");
}

/// A repository with one commit on `main`, in a scratch directory named for `name`.
fn repository(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("protocol-defaults-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(&dir).expect("scratch dir");
    fs::write(dir.join("README.md"), "a change\n").expect("a file");
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "start"]);
    Scratch(dir)
}

/// The settings a run resolves for a store story with nothing from the command line, a tag or a
/// run file.
fn story_settings() -> Settings {
    resolve_settings(&SettingsRequest {
        file: &ProtocolSettings::default(),
        flag_protocol: None,
        flag_inputs: &[],
        tags: &[],
        default_protocol: default_protocol(true),
        file_protocol_applies: true,
    })
    .expect("the defaults resolve")
}

/// Acceptance 1. With no run-file `protocol`, a store story opens on `story.delivery/3` with
/// `tests_first: required` and `gate_on: after_handover`, each recorded with source `default`,
/// and `takt protocol show` lists the same two defaults.
#[test]
fn a_store_story_opens_on_story_delivery_3() {
    let settings = story_settings();
    assert_eq!(settings.protocol.id, "story.delivery/3");
    assert_eq!(settings.protocol_source, Source::Default);
    let values: Vec<(&str, &str, Source)> = settings
        .inputs
        .iter()
        .map(|setting| {
            (
                setting.name.as_str(),
                setting.value.as_str(),
                setting.source,
            )
        })
        .collect();
    assert_eq!(
        values,
        [
            ("tests_first", "required", Source::Default),
            ("gate_on", "after_handover", Source::Default),
        ]
    );

    let repo = repository("story");
    let mut case = CaseDir::open_as(
        &repo,
        settings.protocol.clone(),
        "story-one",
        "a story",
        &settings.pairs(),
        &CommitAs::Takt,
    )
    .expect("the case opens on the defaults");
    case.record_settings(settings.protocol_source.as_str(), &settings.sources())
        .expect("the sources are recorded");
    let held = settings_of(&case.dir).expect("case.yaml reads");
    assert!(
        held.contains(&("tests_first".into(), "required".into(), "default".into())),
        "{held:?}"
    );
    assert!(
        held.contains(&("gate_on".into(), "after_handover".into(), "default".into())),
        "{held:?}"
    );

    let shown = list_settings("story.delivery/3", &ProtocolSettings::default())
        .expect("a built-in")
        .text();
    assert!(shown.contains("  default: required\n"), "{shown}");
    assert!(shown.contains("  default: after_handover\n"), "{shown}");
}

/// The run file names the protocol the defaults then apply to: `story.delivery/3` is only the
/// default of a story, an intent keeps `software.change/1`.
#[test]
fn an_intent_opens_on_software_change() {
    assert_eq!(default_protocol(false), "software.change/1");
    assert_eq!(default_protocol(true), "story.delivery/3");
}

/// A `protocol/1` file declaring the input `mode`, which no flag, tag, run file or default sets.
const MODAL: &str = "format: protocol/1

protocol:
  id: modal.change
  revision: 1
  description: >-
    A protocol with one case input and no default for it.

case:
  inputs:
    mode:
      description: how the change is made
      values: [quick, careful]

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
  implementation.edit:
    description: change the implementation
    effect: write
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

/// Acceptance 2. A protocol whose input has no value anywhere is refused at open, naming the
/// input and its values, and nothing is written; the shipped protocol's two inputs are refused
/// the same way when a caller gives none.
#[test]
fn an_unset_input_is_refused_by_name() {
    let repo = repository("unset");
    let file = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "protocol-defaults-{}-modal.yaml",
        std::process::id()
    ));
    fs::write(&file, MODAL).expect("a protocol file");
    let settings = resolve_settings(&SettingsRequest {
        file: &ProtocolSettings::default(),
        flag_protocol: file.to_str(),
        flag_inputs: &[],
        tags: &[],
        default_protocol: default_protocol(true),
        file_protocol_applies: true,
    })
    .expect("the protocol resolves; nothing sets its input");
    assert!(settings.inputs.is_empty(), "{:?}", settings.inputs);
    let error = CaseDir::open_as(
        &repo,
        settings.protocol.clone(),
        "unset-one",
        "a change",
        &settings.pairs(),
        &CommitAs::Takt,
    )
    .expect_err("an input nobody sets refuses the case");
    let text = format!("{error:#}");
    assert!(text.contains("mode"), "{text}");
    assert!(text.contains("quick") && text.contains("careful"), "{text}");
    assert!(!repo.join(".takt/cases/unset-one").exists(), "{text}");
    fs::remove_file(&file).expect("the protocol file is removed");

    let delivery = LoadedProtocol::builtin(default_protocol(true)).expect("a built-in");
    let error = CaseDir::open(&repo, delivery, "unset-two", "a story", &[])
        .expect_err("Canon gives the inputs no default");
    let text = format!("{error:#}");
    assert!(text.contains("tests_first"), "{text}");
    assert!(
        text.contains("required") && text.contains("optional"),
        "{text}"
    );
    assert!(
        text.contains("give it with `--input tests_first=<value>` or the story tag `input:tests_first=<value>`"),
        "{text}"
    );
}

/// takt's own `story.delivery/3`, embedded in the binary and listed among the built-ins: named
/// by `--protocol`, a case opens on it with takt's defaults, `tests_first=required` and
/// `gate_on=after_handover`, each from `default`; and after a red gate with green tests at the
/// current revision, `implementation.edit` is admissible, so the gate's failure is fixed in the
/// implementation.
#[test]
fn story_delivery_3_opens_with_the_defaults_and_fixes_a_red_gate() {
    assert!(
        LoadedProtocol::builtins()
            .iter()
            .any(|(name, major)| name == "story-delivery" && *major == 3),
        "{:?}",
        LoadedProtocol::builtins()
    );
    assert!(takt::protocol::builtin_ids().contains(&"story.delivery/3".to_owned()));
    assert_eq!(default_protocol(true), "story.delivery/3");
    let settings = resolve_settings(&SettingsRequest {
        file: &ProtocolSettings::default(),
        flag_protocol: Some("story.delivery/3"),
        flag_inputs: &[],
        tags: &[],
        default_protocol: default_protocol(true),
        file_protocol_applies: true,
    })
    .expect("the built-in resolves with its defaults");
    assert_eq!(settings.protocol.id, "story.delivery/3");
    assert_eq!(settings.protocol_source, Source::Flag);
    assert_eq!(
        settings.line(),
        "settings: protocol story.delivery/3 (flag); tests_first=required (default); \
         gate_on=after_handover (default)"
    );
    let repo = repository("delivery-3");
    let mut case = CaseDir::open_as(
        &repo,
        settings.protocol.clone(),
        "delivery-3",
        "a story",
        &settings.pairs(),
        &CommitAs::Takt,
    )
    .expect("the case opens on the defaults");
    let at = "aaaaaaaa11111111111111111111111111111111";
    case.set_revision("implementation", at)
        .expect("the revision is set");
    for (n, (kind, result)) in [
        ("tests_change", None),
        ("test_result", Some("pass")),
        ("handover", None),
        ("gate_result", Some("fail")),
    ]
    .into_iter()
    .enumerate()
    {
        case.add_evidence(&takt::evidence::Record::new(
            format!("e{n}"),
            kind,
            result.map(str::to_owned),
            "implementation",
            at,
            "operator-run",
        ))
        .expect("the record is written");
    }
    let decision = case.evaluate(0).expect("evaluates");
    assert_eq!(
        decision
            .actions
            .get("implementation.edit")
            .map(String::as_str),
        Some("admissible"),
        "{}",
        decision.json
    );
}

/// Under `story.delivery/3`, after a red `tests.run` at the current revision, `change.handover`
/// is not admissible, so a step is never offered it; before any test result, and once the tests
/// pass, it is.
#[test]
fn story_delivery_3_offers_no_handover_at_a_red_revision() {
    let protocol = LoadedProtocol::builtin("story.delivery/3").expect("takt's own");
    let repo = repository("delivery-3-red");
    let inputs = [
        ("tests_first".to_owned(), "required".to_owned()),
        ("gate_on".to_owned(), "after_handover".to_owned()),
    ];
    let mut case =
        CaseDir::open(&repo, protocol, "delivery-3-red", "a story", &inputs).expect("opens");
    let handover = |case: &CaseDir, seq: usize| {
        let decision = case.evaluate(seq).expect("evaluates");
        decision
            .actions
            .get("change.handover")
            .cloned()
            .unwrap_or_default()
    };
    assert_eq!(handover(&case, 0), "admissible");
    let add = |case: &mut CaseDir, n: usize, kind: &str, result: Option<&str>, at: &str| {
        case.set_revision("implementation", at)
            .expect("the revision is set");
        case.add_evidence(&takt::evidence::Record::new(
            format!("e{n}"),
            kind,
            result.map(str::to_owned),
            "implementation",
            at,
            "operator-run",
        ))
        .expect("the record is written");
    };
    let red = "aaaaaaaa11111111111111111111111111111111";
    add(&mut case, 1, "tests_change", None, red);
    add(&mut case, 2, "test_result", Some("fail"), red);
    assert_eq!(handover(&case, 1), "blocked");
    let green = "bbbbbbbb22222222222222222222222222222222";
    add(&mut case, 3, "test_result", Some("pass"), green);
    assert_eq!(handover(&case, 2), "admissible");
}

/// Acceptance 3. The generated page of the defaults equals the site's copy; `task check` fails
/// when the document and the page drift.
#[test]
fn the_reference_page_matches_the_document() {
    let page = defaults_page().expect("the page renders");
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../website/docs/reference/protocol-defaults.md");
    if std::env::var_os("TAKT_WRITE_DEFAULTS_PAGE").is_some() {
        fs::write(&path, &page).expect("the page is written");
    }
    let site = fs::read_to_string(&path).unwrap_or_default();
    assert_eq!(
        site, page,
        "website/docs/reference/protocol-defaults.md differs from the page generated from \
         crates/takt/src/protocol_defaults.yaml; rewrite it with TAKT_WRITE_DEFAULTS_PAGE=1 \
         cargo test -p b10x-takt --test protocol_defaults"
    );
    assert!(
        page.contains("| a story of the planning store | `story.delivery/3` |"),
        "{page}"
    );
    assert!(
        page.contains("| `story.delivery/3` | `tests_first` |"),
        "{page}"
    );
    assert!(
        page.contains("| `story.delivery/2` | `tests_first` |"),
        "{page}"
    );
    assert!(page.contains("`after_handover`"), "{page}");
}

/// Every default the document ships is a value its protocol declares, for an input it declares,
/// and the two protocols it names are built-ins.
#[test]
fn every_shipped_default_is_a_declared_value() {
    for id in [default_protocol(true), default_protocol(false)] {
        assert!(LoadedProtocol::builtin(id).is_ok(), "{id} is a built-in");
    }
    for id in ["story.delivery/3", "story.delivery/2", "software.change/1"] {
        let protocol = LoadedProtocol::builtin(id).expect("a built-in");
        for input in protocol.declared_inputs() {
            let shipped = takt::protocol::builtin_default(id, &input.name)
                .unwrap_or_else(|| panic!("{id} ships a default for {}", input.name));
            assert!(
                input.values.iter().any(|value| value == shipped),
                "{id} {}: {shipped} is not one of {:?}",
                input.name,
                input.values
            );
        }
    }
}

/// The story's tests, its implementation and the edit that fixes the gate, in the scripted run
/// of [`a_red_gate_with_passing_tests_is_fixed_in_the_implementation`].
const STORY_TESTS: &str = "tests/story.txt";
const STORY_CODE: &str = "src/story.txt";
const GATE_FIX: &str = "src/formatted.txt";

/// `tests.run`: red while the tree holds the story's tests and not its implementation, green
/// otherwise.
struct StoryTests;

impl Validator for StoryTests {
    fn name(&self) -> &str {
        "story-tests"
    }

    fn action(&self) -> &str {
        TestsRun::ACTION
    }

    fn kind(&self) -> &str {
        TestsRun::KIND
    }

    fn subject(&self) -> &str {
        "implementation"
    }

    fn observe(&self, tree: &Path, revision: &str, seq: usize) -> anyhow::Result<Observation> {
        let red = tree.join(STORY_TESTS).is_file() && !tree.join(STORY_CODE).is_file();
        let result = if red { "fail" } else { "pass" };
        Ok(Observation {
            summary: result.to_owned(),
            artifact: None,
            records: vec![Record::new(
                format!("tests-{seq}"),
                TestsRun::KIND,
                Some(result.to_owned()),
                "implementation",
                revision,
                self.name(),
            )],
        })
    }
}

/// `gate.run`: red until the tree holds [`GATE_FIX`].
struct FormatGate;

impl Validator for FormatGate {
    fn name(&self) -> &str {
        "format-gate"
    }

    fn action(&self) -> &str {
        GateRun::ACTION
    }

    fn kind(&self) -> &str {
        GateRun::KIND
    }

    fn subject(&self) -> &str {
        "implementation"
    }

    fn observe(&self, tree: &Path, revision: &str, seq: usize) -> anyhow::Result<Observation> {
        let result = if tree.join(GATE_FIX).is_file() {
            "pass"
        } else {
            "fail"
        };
        Ok(Observation {
            summary: result.to_owned(),
            artifact: None,
            records: vec![Record::new(
                format!("gate-{seq}"),
                GateRun::KIND,
                Some(result.to_owned()),
                "implementation",
                revision,
                self.name(),
            )],
        })
    }
}

/// One step the scripted harness was given: the claims `tests.pass` and `gate.green`, what it
/// was offered, and what it chose.
#[derive(Debug, Clone)]
struct Given {
    tests_pass: String,
    gate_green: String,
    offered: Vec<String>,
    chose: String,
}

/// A harness that writes the story's tests when they are absent, then its implementation, hands
/// the change over whenever it may, and fixes the gate in the implementation when it is offered
/// the edit after that; else declines. It keeps each step it was given.
#[derive(Default)]
struct Scripted {
    given: RefCell<Vec<Given>>,
}

impl Harness for Scripted {
    fn name(&self) -> &str {
        "scripted"
    }

    fn step(&self, context: &StepContext<'_>) -> anyhow::Result<StepReport> {
        let offered = |action: &str| context.offered.iter().any(|offer| offer == action);
        let write = |path: &str| {
            let at = context.tree.join(path);
            fs::create_dir_all(at.parent().expect("a parent")).expect("a directory");
            fs::write(&at, "written\n").expect("a file");
        };
        let chose = if !context.tree.join(STORY_TESTS).is_file() && offered("tests.write") {
            write(STORY_TESTS);
            "tests.write"
        } else if !context.tree.join(STORY_CODE).is_file() && offered("implementation.edit") {
            write(STORY_CODE);
            "implementation.edit"
        } else if offered("change.handover") {
            "change.handover"
        } else if !context.tree.join(GATE_FIX).is_file() && offered("implementation.edit") {
            write(GATE_FIX);
            "implementation.edit"
        } else {
            takt::harness::DECLINE
        };
        let claim = |id: &str| context.decision.claims.get(id).cloned().unwrap_or_default();
        self.given.borrow_mut().push(Given {
            tests_pass: claim("tests.pass"),
            gate_green: claim("gate.green"),
            offered: context.offered.to_vec(),
            chose: chose.to_owned(),
        });
        Ok(StepReport {
            action: chose.to_owned(),
            summary: format!("step {}: {chose}", context.step),
            tokens: Some(10),
        })
    }
}

/// story:story-delivery-3-is-the-default, acceptance 3. A store story on the protocol and the
/// inputs it opens on by default, its tests passing at a revision where the repository's gate
/// fails: the next step is offered `implementation.edit`, takes it, and the gate is green at the
/// revision it makes, with no failing test written first.
#[test]
fn a_red_gate_with_passing_tests_is_fixed_in_the_implementation() {
    let repo = repository("red-gate");
    let settings = story_settings();
    assert_eq!(settings.protocol.id, "story.delivery/3");
    let harness = Scripted::default();
    let validators: Vec<Box<dyn Validator>> = vec![Box::new(StoryTests), Box::new(FormatGate)];
    let options = RunOptions {
        protocol: settings.spec.clone(),
        intent: "a story".to_owned(),
        inputs: settings.pairs(),
        iterations: Some(12),
        hours: None,
        tokens: None,
        case_id: Some("red-gate".to_owned()),
        gate: None,
    };
    let mut run = Run::open(&repo, options, &harness, &validators).expect("the case opens");
    assert_eq!(run.case.protocol.id, "story.delivery/3");
    let mut out = Vec::new();
    let stop = run.run(&mut out).map_err(|error| format!("{error:#}"));
    let text = String::from_utf8(out).expect("utf-8");
    let case_dir = run.case.dir.clone();
    drop(run);
    assert!(
        !matches!(stop, Ok(Stop::StepRefused(_))),
        "{stop:?}\n{text}"
    );
    let given = harness.given.borrow().clone();
    let after_red_gate: Vec<&Given> = given
        .iter()
        .filter(|step| step.tests_pass == "true" && step.gate_green == "false")
        .collect();
    assert!(
        !after_red_gate.is_empty(),
        "no step was given passing tests and a red gate: {given:?}\n{text}"
    );
    for step in &after_red_gate {
        assert!(
            step.offered
                .iter()
                .any(|offer| offer == "implementation.edit"),
            "a step with passing tests and a red gate is not offered the edit: {step:?}\n{text}"
        );
    }
    assert_eq!(
        after_red_gate[0].chose, "implementation.edit",
        "{given:?}\n{text}"
    );
    let records: Vec<serde_json::Value> = fs::read_dir(case_dir.join("evidence"))
        .expect("an evidence directory")
        .map(|entry| {
            serde_json::from_str(
                &fs::read_to_string(entry.expect("an entry").path()).expect("a file"),
            )
            .expect("json")
        })
        .collect();
    let gates: Vec<&str> = records
        .iter()
        .filter(|record| record["kind"] == "gate_result")
        .filter_map(|record| record["result"].as_str())
        .collect();
    assert!(
        gates.contains(&"fail") && gates.contains(&"pass"),
        "the gate was not red and then green: {gates:?}\n{text}"
    );
    assert_eq!(
        records
            .iter()
            .filter(|record| record["kind"] == "tests_change")
            .count(),
        1,
        "a test was written to fix the gate:\n{text}"
    );
}

/// story:story-delivery-3-is-the-default, acceptance 4. A case opened on `story.delivery/2`,
/// as every store story was before `story.delivery/3` became the default, resumes on
/// `story.delivery/2`, the same document, with the inputs it was opened with, whatever protocol
/// a new case would open on now.
#[test]
fn a_case_opened_on_story_delivery_2_resumes_on_it() {
    let repo = repository("resume-two");
    let harness = Scripted::default();
    let validators: Vec<Box<dyn Validator>> = Vec::new();
    let inputs = vec![
        ("tests_first".to_owned(), "required".to_owned()),
        ("gate_on".to_owned(), "after_handover".to_owned()),
    ];
    let options = |protocol: &str| RunOptions {
        protocol: protocol.to_owned(),
        intent: "a story".to_owned(),
        inputs: inputs.clone(),
        iterations: Some(0),
        hours: None,
        tokens: None,
        case_id: Some("on-two".to_owned()),
        gate: None,
    };
    let opened = Run::open(&repo, options("story.delivery/2"), &harness, &validators)
        .expect("the case opens on story.delivery/2");
    assert_eq!(opened.case.protocol.id, "story.delivery/2");
    let two = opened.case.protocol.ir_json.clone();
    drop(opened);
    git(&repo, &["checkout", "-q", "main"]);
    assert_eq!(default_protocol(true), "story.delivery/3");
    let resumed = Run::resume(
        &repo,
        options(default_protocol(true)),
        "on-two",
        &harness,
        &validators,
    )
    .expect("the case resumes");
    assert_eq!(resumed.case.protocol.id, "story.delivery/2");
    assert_eq!(resumed.case.protocol.ir_json, two);
    assert_eq!(resumed.case.inputs(), inputs);
}
