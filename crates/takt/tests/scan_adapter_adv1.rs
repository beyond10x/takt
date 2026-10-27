//! Adversary pass 1 on story:scan-and-forge-are-adapters, the library's edge: the run
//! configuration file's `scan` field read against Acceptance 4, "`scan` with another value in
//! the file is refused naming the file and the line, as any other bad field is", and against the
//! domain's `takt.work.ScanName`, an enum of the two words `gates` and `none`.

use std::fs;
use std::path::PathBuf;

use takt::run_config::RunConfig;

/// A scratch directory under this crate's build directory, removed on drop.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("scan-adapter-adv1-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("a scratch directory");
        Self(dir)
    }

    fn file(&self, name: &str, text: &str) -> PathBuf {
        let path = self.0.join(name);
        fs::write(&path, text).expect("a run configuration file");
        path
    }
}

/// A mapping whose one key is `none` (or `gates`) is not the word `none`. serde reads a unit
/// variant from an externally tagged table, so in TOML `scan = { none = {} }` is taken as the
/// declaration `scan: none` rather than refused as a value of another shape. The YAML forms are
/// the same shape in the other format, where the reader refuses them; every form here must be
/// refused, as `scan: [none]` or `scan: 1` is.
#[test]
fn a_scan_written_as_a_mapping_is_refused_not_read_as_an_adapter() {
    let scratch = Scratch::new("mapping");
    let mut accepted = Vec::new();
    for (label, name, text) in [
        (
            "yaml flow map",
            "flow.yaml",
            "iterations: 1\nscan: {none: ~}\n",
        ),
        (
            "yaml block map",
            "block.yaml",
            "iterations: 1\nscan:\n  none:\n",
        ),
        (
            "loom.agent/1 takt member",
            "agent.yaml",
            "format: loom.agent/1\ntakt:\n  scan: {none: null}\n",
        ),
        (
            "yaml gates map",
            "gates.yaml",
            "iterations: 1\nscan: {gates: ~}\n",
        ),
        (
            "toml inline table",
            "run.toml",
            "iterations = 1\nscan = { none = {} }\n",
        ),
    ] {
        let path = scratch.file(name, text);
        if let Ok(config) = RunConfig::read(&path) {
            accepted.push(format!("{label}: read as {:?}", config.scan));
        }
    }
    assert!(
        accepted.is_empty(),
        "a mapping is neither `gates` nor `none`, and was accepted:\n{}",
        accepted.join("\n")
    );
}
