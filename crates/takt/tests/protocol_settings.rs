//! The run file names the protocol and sets its declared inputs
//! (story:the-run-file-sets-protocol-settings), library side: `RunConfig::read` keeps the
//! `protocol` key and the `protocols` section with their lines, `resolve_settings` takes each
//! input from the flag, a story tag, the run file or takt's default, and a case opened on the
//! result records each value with its source in `case.yaml`. `software.change/1` declares
//! `risk: [trivial, standard, elevated, critical]`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use takt::case::{CaseDir, CommitAs, settings_of};
use takt::protocol::{Settings, SettingsRequest, Source, resolve_settings};
use takt::run_config::{ProtocolSettings, RunConfig};

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
