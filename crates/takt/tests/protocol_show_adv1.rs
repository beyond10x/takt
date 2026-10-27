//! Adversarial cases for story:takt-lists-a-protocols-settings: the listing against the story's
//! own acceptance example, a run-file value `takt run` refuses, and a TOML run file's lines.

use std::fs;
use std::path::{Path, PathBuf};

use takt::protocol::list_settings;
use takt::run_config::{ProtocolSettings, RunConfig};

struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn file_settings(name: &str, file: &str, text: &str) -> (Scratch, PathBuf, ProtocolSettings) {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("protocol-show-adv1-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(&dir).expect("scratch dir");
    let path = dir.join(file);
    fs::write(&path, text).expect("a run file");
    let settings = RunConfig::read(&path)
        .expect("a readable run file")
        .protocol_settings;
    (Scratch(dir), path, settings)
}

/// Acceptance 1 as the story writes it: `story.delivery/2` with a run file setting
/// `gate_on: each_revision` lists `tests_first` (default `required`) and `gate_on` (default
/// `after_handover`, run file `each_revision`). `story.delivery/2` ships with the engineering
/// protocols' next release (story:takt-ships-default-protocol-settings), so today the listing
/// refuses it naming the protocols takt knows; the case turns red when that revision arrives,
/// and is then to assert the example itself.
#[test]
fn the_acceptance_example_lists_story_delivery_2() {
    let (_dir, _path, settings) = file_settings(
        "acceptance",
        "run.yaml",
        "protocols:\n  story.delivery:\n    gate_on: each_revision\n",
    );
    let refusal = match list_settings("story.delivery/2", &settings) {
        Err(error) => format!("{error:#}"),
        Ok(listing) => panic!(
            "story:takt-ships-default-protocol-settings: story.delivery/2 is listed now, so this case asserts the example: {}",
            listing.text()
        ),
    };
    assert!(
        refusal.contains("no protocol story.delivery/2")
            && refusal.contains("story.delivery/1")
            && refusal.contains("software.change/1"),
        "story:takt-ships-default-protocol-settings: the refusal names no known protocol: {refusal}"
    );
}

/// A run file value outside the declared set is what `takt run` refuses at its line; the listing
/// shows it as the run file's value with no word that the run refuses it.
#[test]
fn a_value_the_run_refuses_is_not_listed_as_set() {
    let (_dir, _path, settings) = file_settings(
        "refused",
        "run.yaml",
        "protocols:\n  software.change:\n    risk: extreme\n",
    );
    match list_settings("software.change/1", &settings) {
        Err(error) => {
            let said = format!("{error:#}");
            assert!(said.contains("line 3"), "{said}");
        }
        Ok(listing) => {
            let text = listing.text();
            assert!(
                !text.contains("  run file: extreme ("),
                "a refused value is listed as set: {text}"
            );
        }
    }
}

/// A TOML run file: each form names the line the value is written on.
#[test]
fn a_toml_run_file_names_each_line() {
    for (name, text, line) in [
        (
            "table",
            "iterations = 1\n\n[protocols.\"software.change\"]\nrisk = \"elevated\"\n",
            4,
        ),
        (
            "parent",
            "[protocols]\n\"software.change\" = { risk = \"elevated\" }\n",
            2,
        ),
        (
            "inline",
            "protocols = { \"software.change\" = { risk = \"elevated\" } }\n",
            1,
        ),
        (
            "two-tables",
            "[protocols.\"software.change/1\"]\nchange_kind = \"bugfix\"\n\n[protocols.\"software.change\"]\nrisk = \"elevated\"\n",
            5,
        ),
    ] {
        let (_dir, _path, settings) = file_settings(name, "run.toml", text);
        let listing = list_settings("software.change/1", &settings).expect("a listing");
        let set = listing.inputs[0]
            .run_file
            .as_ref()
            .unwrap_or_else(|| panic!("{name}: risk is set"));
        assert_eq!(
            (set.value.as_str(), set.line),
            ("elevated", line),
            "{name}: {text}"
        );
    }
}
