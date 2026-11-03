//! The run file names the protocol and sets its declared inputs
//! (story:the-run-file-sets-protocol-settings), library side: `RunConfig::read` keeps the
//! `protocol` key and the `protocols` section with their lines, `resolve_settings` takes each
//! input from the flag, a story tag, the run file or takt's default, and a case opened on the
//! result records each value with its source in `case.yaml`. `software.change/1` declares
//! `risk: [trivial, standard, elevated, critical]`.
//!
//! The run side (story:story-delivery-tests-first): a story case on `story.delivery/3` with the
//! run file's `tests_first: optional` admits a harness that edits the implementation first; with
//! takt's default, `required`, the protocol never offers that edit, and a harness that takes it
//! anyway is refused with the change discarded.

#[path = "support/own_home.rs"]
mod own_home;

use std::cell::{Cell, RefCell};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

use takt::case::{CaseDir, CommitAs, settings_of};
use takt::evidence::Record;
use takt::forge::{Checks, Forge, Memory};
use takt::harness::{Harness, StepContext, StepReport};
use takt::protocol::{Settings, SettingsRequest, Source, default_protocol, resolve_settings};
use takt::run_config::{ProtocolSettings, RunConfig};
use takt::validator::{ChangeRead, CiRead, Observation, PrOpen, Validator};
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

fn directory(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("protocol-settings-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(&dir).expect("scratch dir");
    Scratch(dir)
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

/// A repository with one commit on `main`.
fn repository(name: &str) -> Scratch {
    let dir = directory(name);
    fs::write(dir.join("README.md"), "a change\n").expect("a file");
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "start"]);
    dir
}

/// The run file `text` as `name` in a directory of its own, read.
fn read(name: &str, file: &str, text: &str) -> (Scratch, anyhow::Result<RunConfig>) {
    let dir = directory(name);
    let path = dir.join(file);
    fs::write(&path, text).expect("a run file");
    let read = RunConfig::read(&path);
    (dir, read)
}

fn file_settings(name: &str, file: &str, text: &str) -> (Scratch, ProtocolSettings) {
    let (dir, read) = read(name, file, text);
    (dir, read.expect("the run file reads").protocol_settings)
}

const ELEVATED: &str = "protocols:\n  software.change:\n    risk: elevated\n";

/// Resolves for a story (`story`) or an intent with the given layers.
fn resolve(
    file: &ProtocolSettings,
    flag_protocol: Option<&str>,
    flag_inputs: &[(String, String)],
    tags: &[&str],
    story: bool,
) -> anyhow::Result<Settings> {
    let tags: Vec<String> = tags.iter().map(|tag| (*tag).to_owned()).collect();
    resolve_settings(&SettingsRequest {
        file,
        flag_protocol,
        flag_inputs,
        tags: &tags,
        default_protocol: if story {
            "story.delivery/1"
        } else {
            "software.change/1"
        },
        file_protocol_applies: story,
    })
}

fn value(settings: &Settings, name: &str) -> (String, Source) {
    let setting = settings
        .inputs
        .iter()
        .find(|setting| setting.name == name)
        .unwrap_or_else(|| panic!("{name} is set"));
    (setting.value.clone(), setting.source)
}

/// The case `settings` opens in `repo`, its sources recorded, and what `case.yaml` then holds of
/// its inputs: `(name, value, source)`.
fn opened(repo: &Path, settings: &Settings) -> (CaseDir, Vec<(String, String, String)>) {
    let mut case = CaseDir::open_as(
        repo,
        settings.protocol.clone(),
        "probe",
        "a change",
        &settings.pairs(),
        &CommitAs::Takt,
    )
    .expect("the case opens");
    case.record_settings(settings.protocol_source.as_str(), &settings.sources())
        .expect("the sources are recorded");
    let held = settings_of(&case.dir).expect("case.yaml reads");
    (case, held)
}

/// Acceptance 1: a run file setting `software.change` `risk: elevated` opens a case whose
/// `case.yaml` records `risk: elevated` with source `run-file`.
#[test]
fn the_run_file_sets_an_input() {
    for (file, text) in [
        ("run.yaml", ELEVATED),
        (
            "run.toml",
            "[protocols.\"software.change\"]\nrisk = \"elevated\"\n",
        ),
    ] {
        let (_dir, settings) = file_settings("sets", file, text);
        let resolved = resolve(&settings, None, &[], &[], false).expect("resolves");
        assert_eq!(
            value(&resolved, "risk"),
            ("elevated".to_owned(), Source::RunFile),
            "{file}"
        );
        let repo = repository("sets-case");
        let (case, held) = opened(&repo, &resolved);
        assert!(
            held.contains(&("risk".into(), "elevated".into(), "run-file".into())),
            "{file}: {held:?}"
        );
        let yaml = fs::read_to_string(case.dir.join("case.yaml")).expect("case.yaml");
        assert!(yaml.contains("risk: elevated"), "{yaml}");
        assert!(yaml.contains("risk: run-file"), "{yaml}");
        // The inputs nobody set take takt's built-in default, recorded as such.
        assert!(
            held.contains(&("change_kind".into(), "feature".into(), "default".into())),
            "{held:?}"
        );
        drop(case);
    }
}

/// Acceptance 2: `risk: extreme` is refused with the file, the line and the four declared values.
#[test]
fn an_unknown_value_is_refused_at_its_line() {
    for (file, text, line) in [
        (
            "run.yaml",
            "iterations: 3\nprotocols:\n  software.change:\n    risk: extreme\n",
            4,
        ),
        (
            "run.toml",
            "iterations = 3\n[protocols.\"software.change\"]\nrisk = \"extreme\"\n",
            3,
        ),
        (
            "run.yaml",
            "format: loom.agent/1\ntakt:\n  protocols:\n    software.change:\n      risk: extreme\n",
            5,
        ),
    ] {
        let (dir, settings) = file_settings("unknown-value", file, text);
        let refusal = format!(
            "{:#}",
            resolve(&settings, None, &[], &[], false).expect_err("refused")
        );
        let named = dir.join(file).display().to_string();
        assert!(refusal.contains(&named), "{refusal}");
        assert!(
            refusal.contains(&format!("line {line}:")),
            "{file}: {refusal}"
        );
        assert!(
            refusal.contains("trivial, standard, elevated, critical"),
            "{refusal}"
        );
        assert!(
            !refusal.contains("extreme"),
            "no value written in the file is repeated: {refusal}"
        );
    }
}

/// An unknown protocol and an unknown input are refused at their lines too, naming what the
/// protocol declares.
#[test]
fn an_unknown_protocol_or_input_is_refused_at_its_line() {
    let (_dir, settings) = file_settings(
        "unknown-protocol",
        "run.yaml",
        "protocols:\n  no.such:\n    risk: trivial\n",
    );
    let refusal = format!(
        "{:#}",
        resolve(&settings, None, &[], &[], false).expect_err("refused")
    );
    assert!(refusal.contains("line 2:"), "{refusal}");
    assert!(refusal.contains("software.change/1"), "{refusal}");

    let (_dir, settings) = file_settings(
        "unknown-input",
        "run.yaml",
        "protocols:\n  software.change:\n    risk: trivial\n    speed: fast\n",
    );
    let refusal = format!(
        "{:#}",
        resolve(&settings, None, &[], &[], false).expect_err("refused")
    );
    assert!(refusal.contains("line 4:"), "{refusal}");
    assert!(
        refusal.contains("risk, change_kind, affects_security_boundary"),
        "{refusal}"
    );

    let (_dir, settings) = file_settings(
        "flow",
        "run.yaml",
        "protocols: {software.change: {risk: extreme}}\n",
    );
    let refusal = format!(
        "{:#}",
        resolve(&settings, None, &[], &[], false).expect_err("refused")
    );
    assert!(refusal.contains("line 1:"), "{refusal}");
}

/// A `protocols` member of the wrong shape is refused when the file is read, at its line.
#[test]
fn a_protocols_member_of_the_wrong_shape_is_refused_when_read() {
    for (text, line) in [
        ("iterations: 1\nprotocols: [software.change]\n", 2),
        ("protocols:\n  software.change: elevated\n", 2),
        ("protocols:\n  software.change:\n    risk: [elevated]\n", 3),
        ("protocol: software.change\n", 1),
    ] {
        let (_dir, read) = read("shape", "run.yaml", text);
        let refusal = format!("{:#}", read.expect_err("refused"));
        assert!(
            refusal.contains(&format!("line {line}:")),
            "{text:?}: {refusal}"
        );
    }
}

/// Acceptance 3: with the run file at `elevated`, a story tagged `input:risk=trivial` opens with
/// `trivial`, source `tag`.
#[test]
fn a_tag_overrides_the_run_file() {
    let (_dir, file) = file_settings("tag", "run.yaml", ELEVATED);
    let resolved = resolve(&file, None, &[], &["input:risk=trivial"], false).expect("resolves");
    assert_eq!(
        value(&resolved, "risk"),
        ("trivial".to_owned(), Source::Tag)
    );
    let repo = repository("tag-case");
    let (case, held) = opened(&repo, &resolved);
    assert!(
        held.contains(&("risk".into(), "trivial".into(), "tag".into())),
        "{held:?}"
    );
    drop(case);
    let refusal = format!(
        "{:#}",
        resolve(&file, None, &[], &["input:risk=extreme"], false).expect_err("refused")
    );
    assert!(
        refusal.contains("trivial, standard, elevated, critical"),
        "{refusal}"
    );
}

/// Acceptance 4: `--input risk=critical` over that tag opens with `critical`, source `flag`.
#[test]
fn the_flag_overrides_the_tag() {
    let (_dir, file) = file_settings("flag", "run.yaml", ELEVATED);
    let flags = [("risk".to_owned(), "critical".to_owned())];
    let resolved = resolve(&file, None, &flags, &["input:risk=trivial"], false).expect("resolves");
    assert_eq!(
        value(&resolved, "risk"),
        ("critical".to_owned(), Source::Flag)
    );
    let repo = repository("flag-case");
    let (case, held) = opened(&repo, &resolved);
    assert!(
        held.contains(&("risk".into(), "critical".into(), "flag".into())),
        "{held:?}"
    );
    drop(case);
    let undeclared = [("speed".to_owned(), "fast".to_owned())];
    let refusal = format!(
        "{:#}",
        resolve(&file, None, &undeclared, &[], false).expect_err("refused")
    );
    assert!(refusal.contains("declares no input speed"), "{refusal}");
}

/// Acceptance 5: `protocol: software.change/1` opens a store story on it.
#[test]
fn the_protocol_key_chooses_the_protocol() {
    let (_dir, file) = file_settings("key", "run.yaml", "protocol: software.change/1\n");
    let resolved = resolve(&file, None, &[], &[], true).expect("resolves");
    assert_eq!(resolved.protocol.id, "software.change/1");
    assert_eq!(resolved.protocol_source, Source::RunFile);
    // An intent opens on the software change whatever the key says, and the flag wins over it.
    let other = resolve(&file, Some("story.delivery/1"), &[], &[], true).expect("resolves");
    assert_eq!(other.protocol.id, "story.delivery/1");
    assert_eq!(other.protocol_source, Source::Flag);
    let (_dir, bad) = file_settings(
        "key-unknown",
        "run.yaml",
        "iterations: 1\nprotocol: no.such/1\n",
    );
    let refusal = format!(
        "{:#}",
        resolve(&bad, None, &[], &[], true).expect_err("refused")
    );
    assert!(refusal.contains("line 2:"), "{refusal}");
    let default = resolve(&ProtocolSettings::default(), None, &[], &[], true).expect("resolves");
    assert_eq!(default.protocol.id, "story.delivery/1");
    assert_eq!(default.protocol_source, Source::Default);
}

/// Acceptance 6: with `protocol: software.change/1` in the run file, a story tagged
/// `protocol:story.delivery/1` opens on `story.delivery/1`, recorded with source `tag`.
#[test]
fn a_protocol_tag_overrides_the_key() {
    let (_dir, file) = file_settings("tag-protocol", "run.yaml", "protocol: software.change/1\n");
    let resolved =
        resolve(&file, None, &[], &["protocol:story.delivery/1"], true).expect("resolves");
    assert_eq!(resolved.protocol.id, "story.delivery/1");
    assert_eq!(resolved.protocol_source, Source::Tag);
    assert!(
        resolved
            .line()
            .starts_with("settings: protocol story.delivery/1 (tag)"),
        "{}",
        resolved.line()
    );
    let repo = repository("tag-protocol-case");
    let mut case = CaseDir::open_as(
        &repo,
        resolved.protocol.clone(),
        "probe",
        "a change",
        &resolved.pairs(),
        &CommitAs::Takt,
    )
    .expect("the case opens");
    case.record_settings("tag", &[]).expect("recorded");
    let yaml = fs::read_to_string(case.dir.join("case.yaml")).expect("case.yaml");
    assert!(yaml.contains("protocol_source: tag"), "{yaml}");
    let refusal = format!(
        "{:#}",
        resolve(&file, None, &[], &["protocol:no.such/1"], true).expect_err("refused")
    );
    assert!(refusal.contains("story.delivery/1"), "{refusal}");
    let judge = format!(
        "{:#}",
        resolve(&file, None, &[], &["protocol:self.improvement/1"], true).expect_err("refused")
    );
    assert!(judge.contains("never on a story"), "{judge}");
}

/// The story's test file and its implementation file, for the run cases below.
const TESTS: &str = "tests/remove_test.txt";
const IMPLEMENTATION: &str = "src/remove.txt";

/// Whether `path` is in the tree of `revision`.
fn holds(tree: &Path, revision: &str, path: &str) -> bool {
    Command::new("git")
        .args(["cat-file", "-e", &format!("{revision}:{path}")])
        .current_dir(tree)
        .status()
        .expect("git runs")
        .success()
}

/// A validator of `action` that observes `pass` at every revision, but `fail` for `tests.run`
/// when the revision holds the story's tests and not its implementation.
struct Scripted {
    action: &'static str,
    kind: &'static str,
    red: bool,
}

impl Validator for Scripted {
    fn name(&self) -> &str {
        self.action
    }

    fn action(&self) -> &str {
        self.action
    }

    fn kind(&self) -> &str {
        self.kind
    }

    fn subject(&self) -> &str {
        "implementation"
    }

    fn observe(&self, tree: &Path, revision: &str, seq: usize) -> anyhow::Result<Observation> {
        let fails =
            self.red && holds(tree, revision, TESTS) && !holds(tree, revision, IMPLEMENTATION);
        let result = if fails { "fail" } else { "pass" };
        Ok(Observation {
            summary: result.to_owned(),
            artifact: None,
            records: vec![Record::new(
                format!("{}-{seq}", self.kind),
                self.kind,
                Some(result.to_owned()),
                self.subject(),
                revision,
                self.name(),
            )],
        })
    }
}

/// A developer that edits the implementation first, at its first step, whether or not the
/// step offers `implementation.edit`; then writes the story's tests when offered, and hands
/// the change over. It keeps what each step was offered.
struct ImplementationFirst {
    steps: Cell<usize>,
    offered: RefCell<Vec<Vec<String>>>,
}

impl Harness for ImplementationFirst {
    fn name(&self) -> &str {
        "implementation-first"
    }

    fn step(&self, context: &StepContext<'_>) -> anyhow::Result<StepReport> {
        self.steps.set(self.steps.get() + 1);
        self.offered.borrow_mut().push(context.offered.to_vec());
        let offered = |action: &str| context.offered.iter().any(|offer| offer == action);
        let write = |path: &str, action: &str| -> anyhow::Result<StepReport> {
            let file = context.tree.join(path);
            fs::create_dir_all(file.parent().expect("a parent"))?;
            fs::write(&file, format!("{action}\n"))?;
            Ok(StepReport {
                action: action.to_owned(),
                summary: format!("wrote {path}"),
                tokens: Some(100),
            })
        };
        if self.steps.get() == 1 {
            return write(IMPLEMENTATION, "implementation.edit");
        }
        if !context.tree.join(TESTS).exists() && offered("tests.write") {
            return write(TESTS, "tests.write");
        }
        let action = if context.approvals.iter().any(|a| a == "repository.merge") {
            "repository.merge"
        } else if offered("change.handover") {
            "change.handover"
        } else {
            context
                .offered
                .first()
                .map_or("repository.inspect", String::as_str)
        };
        Ok(StepReport {
            action: action.to_owned(),
            summary: "the change is complete".to_owned(),
            tokens: Some(100),
        })
    }
}

/// A finished story run: its stop, what it printed, the forge's calls, what each step was
/// offered, the case directory and the tree, kept until dropped.
struct Delivered {
    stop: Result<Stop, String>,
    text: String,
    calls: Vec<String>,
    offered: Vec<Vec<String>>,
    case_dir: PathBuf,
    tree: Scratch,
}

/// A store story's run on the settings the run file `text` resolves to with no flag and no
/// tag, by [`ImplementationFirst`]: the tests observed red until the implementation is there,
/// the gate green, `pr.open` and `ci.read` on a fake forge whose checks pass, and
/// `change.read`.
fn delivered_on(name: &str, text: &str) -> Delivered {
    let (_dir, file) = file_settings(name, "run.yaml", text);
    let settings = resolve_settings(&SettingsRequest {
        file: &file,
        flag_protocol: None,
        flag_inputs: &[],
        tags: &[],
        default_protocol: default_protocol(true),
        file_protocol_applies: true,
    })
    .expect("the settings resolve");
    assert_eq!(settings.protocol.id, "story.delivery/3");
    let tree = repository(&format!("{name}-run"));
    let memory = Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    });
    let forge: Rc<dyn Forge> = memory.clone();
    let validators: Vec<Box<dyn Validator>> = vec![
        Box::new(Scripted {
            action: "tests.run",
            kind: "test_result",
            red: true,
        }),
        Box::new(Scripted {
            action: "gate.run",
            kind: "gate_result",
            red: false,
        }),
        Box::new(PrOpen::new(forge.clone())),
        Box::new(CiRead::new(forge)),
        Box::new(ChangeRead),
    ];
    let options = RunOptions {
        protocol: settings.spec.clone(),
        intent: "remove a todo by its number".to_owned(),
        inputs: settings.pairs(),
        iterations: Some(40),
        hours: None,
        tokens: None,
        case_id: Some(name.to_owned()),
        gate: Some("repository.merge".to_owned()),
    };
    let harness = ImplementationFirst {
        steps: Cell::new(0),
        offered: RefCell::new(Vec::new()),
    };
    let mut run = Run::open(&tree, options, &harness, &validators).expect("the case opens");
    let mut out = Vec::new();
    let stop = run.run(&mut out).map_err(|error| format!("{error:#}"));
    let case_dir = run.case.dir.clone();
    drop(run);
    let calls = memory.calls.borrow().clone();
    Delivered {
        stop,
        text: String::from_utf8(out).expect("utf-8"),
        calls,
        offered: harness.offered.into_inner(),
        case_dir,
        tree,
    }
}

/// The lines of the case's `events.jsonl` whose `event` is `event`.
fn events_of(case_dir: &Path, event: &str) -> Vec<serde_json::Value> {
    fs::read_to_string(case_dir.join("events.jsonl"))
        .expect("events.jsonl")
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("json"))
        .filter(|line| line["event"] == event)
        .collect()
}

/// With `protocols: {story.delivery: {tests_first: optional}}`, a harness that edits the
/// implementation first is admitted: its first step is offered `implementation.edit`, the edit
/// is kept on the case's branch, and the case records no refusal.
#[test]
fn tests_optional_admits_implementation_first() {
    let run = delivered_on(
        "optional",
        "protocols: {story.delivery: {tests_first: optional}}\n",
    );
    let text = &run.text;
    let yaml = fs::read_to_string(run.case_dir.join("case.yaml")).expect("case.yaml");
    assert!(yaml.contains("tests_first: optional"), "{yaml}");
    assert!(
        run.offered
            .first()
            .is_some_and(|first| first.iter().any(|a| a == "implementation.edit")),
        "the first step was not offered implementation.edit: {:?}\n{text}",
        run.offered
    );
    let refusals = events_of(&run.case_dir, "refusal");
    assert!(refusals.is_empty(), "{refusals:?}\n{text}");
    assert!(!text.contains("refused:"), "{text}");
    assert!(
        holds(&run.tree, "HEAD", IMPLEMENTATION),
        "the implementation edit was not kept: {text}"
    );
    assert!(
        !matches!(run.stop, Ok(Stop::StepRefused(_))),
        "{:?}\n{text}",
        run.stop
    );
}

/// With takt's default `tests_first: required`, the protocol does not offer
/// `implementation.edit` before the story's tests are written and red; a harness that edits the
/// implementation first anyway is refused as taking an action not offered, its edit discarded,
/// the refusal recorded, and nothing reaches the forge.
#[test]
fn tests_required_refuses_implementation_first() {
    let run = delivered_on("required", "iterations: 40\n");
    let text = &run.text;
    let yaml = fs::read_to_string(run.case_dir.join("case.yaml")).expect("case.yaml");
    assert!(yaml.contains("tests_first: required"), "{yaml}");
    let first = run.offered.first().expect("a step was taken");
    assert!(
        !first.iter().any(|a| a == "implementation.edit"),
        "implementation.edit was offered before the tests: {first:?}\n{text}"
    );
    match &run.stop {
        Ok(Stop::StepRefused(why)) => assert!(
            why.contains("implementation.edit was not offered"),
            "{why}\n{text}"
        ),
        other => panic!("not refused: {other:?}\n{text}"),
    }
    let refusals = events_of(&run.case_dir, "refusal");
    assert_eq!(refusals.len(), 1, "{refusals:?}\n{text}");
    assert!(
        !holds(&run.tree, "HEAD", IMPLEMENTATION),
        "the refused edit was kept: {text}"
    );
    assert!(run.calls.is_empty(), "{:?}\n{text}", run.calls);
}
