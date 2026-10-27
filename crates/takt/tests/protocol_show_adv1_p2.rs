//! The acceptance example of story:takt-ships-default-protocol-settings: `story.delivery/2` with a
//! run file setting `gate_on: each_revision` lists `tests_first` and `gate_on`. It was the case of
//! story:takt-lists-a-protocols-settings that waited for that revision, and moves with its story.

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
