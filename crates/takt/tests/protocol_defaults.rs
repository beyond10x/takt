//! story:takt-ships-default-protocol-settings. takt ships one defaults document, as data: the
//! protocol a store story opens on (`story.delivery/2`) and the value of each declared input
//! nobody sets (`tests_first: required`, `gate_on: after_handover`). Canon declares those inputs
//! and gives them no default, so a case whose input nothing sets is refused at open, naming the
//! input and its values, and the reference page of the defaults is generated from the document
//! and checked against the site's copy.
//!
//! To rewrite the page after the document changes, run this test with
//! `TAKT_WRITE_DEFAULTS_PAGE=1`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use takt::case::{CaseDir, CommitAs, settings_of};
use takt::protocol::{
    LoadedProtocol, Settings, SettingsRequest, Source, default_protocol, defaults_page,
    list_settings, resolve_settings,
};
use takt::run_config::ProtocolSettings;

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

/// Acceptance 1. With no run-file `protocol`, a store story opens on `story.delivery/2` with
/// `tests_first: required` and `gate_on: after_handover`, each recorded with source `default`,
/// and `takt protocol show` lists the same two defaults.
#[test]
fn a_store_story_opens_on_story_delivery_2() {
    let settings = story_settings();
    assert_eq!(settings.protocol.id, "story.delivery/2");
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

    let shown = list_settings("story.delivery/2", &ProtocolSettings::default())
        .expect("a built-in")
        .text();
    assert!(shown.contains("  default: required\n"), "{shown}");
    assert!(shown.contains("  default: after_handover\n"), "{shown}");
}

/// The run file names the protocol the defaults then apply to: `story.delivery/2` is only the
/// default of a story, an intent keeps `software.change/1`.
#[test]
fn an_intent_opens_on_software_change() {
    assert_eq!(default_protocol(false), "software.change/1");
    assert_eq!(default_protocol(true), "story.delivery/2");
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

    let delivery = LoadedProtocol::builtin("story.delivery/2").expect("a built-in");
    let error = CaseDir::open(&repo, delivery, "unset-two", "a story", &[])
        .expect_err("Canon gives the inputs no default");
    let text = format!("{error:#}");
    assert!(text.contains("tests_first"), "{text}");
    assert!(
        text.contains("required") && text.contains("optional"),
        "{text}"
    );
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
    for id in ["story.delivery/2", "software.change/1"] {
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
