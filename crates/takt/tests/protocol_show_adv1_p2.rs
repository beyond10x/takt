//! The acceptance example of story:takt-ships-default-protocol-settings: `story.delivery/2` with a
//! run file setting `gate_on: each_revision` lists `tests_first` and `gate_on`. It was the case of
//! story:takt-lists-a-protocols-settings that waited for that revision, and moved with its story.

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
/// `gate_on: each_revision` lists `tests_first` (default `required`, run file unset) and
/// `gate_on` (default `after_handover`, run file `each_revision` at its line).
#[test]
fn the_acceptance_example_lists_story_delivery_2() {
    let (_dir, path, settings) = file_settings(
        "acceptance",
        "run.yaml",
        "protocols:\n  story.delivery:\n    gate_on: each_revision\n",
    );
    let text = list_settings("story.delivery/2", &settings)
        .expect("story:takt-ships-default-protocol-settings: story.delivery/2 is listed")
        .text();
    let tests_first = text
        .split("input gate_on")
        .next()
        .expect("the listing has a part before gate_on");
    assert!(
        text.starts_with("protocol story.delivery/2\n")
            && tests_first.contains("input tests_first"),
        "story:takt-ships-default-protocol-settings: the listing names the protocol, then tests_first: {text}"
    );
    assert!(
        tests_first.contains("  default: required\n")
            && tests_first.contains("  run file: unset\n"),
        "story:takt-ships-default-protocol-settings: tests_first defaults to required, unset in the file: {text}"
    );
    let gate_on = text
        .split("input gate_on")
        .nth(1)
        .expect("story:takt-ships-default-protocol-settings: the listing has gate_on");
    assert!(
        gate_on.contains("  default: after_handover\n")
            && gate_on.contains(&format!("  run file: each_revision ({}:3)", path.display())),
        "story:takt-ships-default-protocol-settings: gate_on defaults to after_handover, each_revision from the file's line 3: {text}"
    );
}
