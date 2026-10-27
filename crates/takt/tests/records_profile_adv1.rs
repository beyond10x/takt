//! Adversary pass 1 on story:a-run-keeps-every-decision-only-on-a-trial, the library side: the
//! run configuration's `records` in TOML and YAML, every value but `default` and `trial`
//! refused before a case opens, repeating no value written in the file.

use std::fs;
use std::path::{Path, PathBuf};

use takt::run_config::RunConfig;

/// A scratch directory under this crate's build directory, one per test.
fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("records-adv1-{name}-{}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch directory is removed");
    }
    fs::create_dir_all(&dir).expect("a directory");
    dir
}

/// The refusal of `text` written to `dir/<file>`, as one string; panics when it is read.
fn refusal(dir: &Path, file: &str, text: &str) -> String {
    let path = dir.join(file);
    fs::write(&path, text).expect("a run configuration file");
    match RunConfig::read(&path) {
        Ok(config) => panic!("{file} with {text:?} is read: {config:?}"),
        Err(error) => format!("{error:#}"),
    }
}

/// Each value of `records` that is not `default` or `trial`, in either format and of any type,
/// is refused naming the key, repeating no value: a case of another spelling (`TRIAL`,
/// `Default`), a value with a space, an empty string, a number, a boolean, a list, a table.
#[test]
fn every_records_value_but_default_and_trial_is_refused_repeating_none() {
    let dir = scratch("values");
    let toml = [
        ("records = \"TRIAL\"\n", "TRIAL"),
        ("records = \"Default\"\n", "Default"),
        ("records = \"trial \"\n", "trial "),
        ("records = \"\"\n", ""),
        ("records = 987654321987\n", "987654321987"),
        ("records = true\n", "true"),
        ("records = [\"trial\"]\n", "[\""),
        ("records = { trial = 1 }\n", "{ trial"),
    ];
    for (text, value) in toml {
        let said = refusal(&dir, "run.toml", text);
        assert!(said.contains("`records`"), "{text:?}: {said}");
        if !value.is_empty() && value != "true" {
            assert!(!said.contains(value), "{text:?} repeats its value: {said}");
        }
    }
    let yaml = [
        ("records: TRIAL\n", "TRIAL"),
        ("records: Default\n", "Default"),
        ("records: ''\n", ""),
        ("records: 987654321987\n", "987654321987"),
        ("records: yes-MARKER\n", "yes-MARKER"),
        ("records: [trial]\n", "[trial]"),
        ("records:\n  trial: 1\n", "trial: 1"),
    ];
    for (text, value) in yaml {
        let said = refusal(&dir, "run.yaml", text);
        assert!(said.contains("`records`"), "{text:?}: {said}");
        if !value.is_empty() {
            assert!(!said.contains(value), "{text:?} repeats its value: {said}");
        }
    }
    let loom = refusal(
        &dir,
        "agent.yaml",
        "format: loom.agent/1\ntakt:\n  records: TRIAL\n",
    );
    assert!(loom.contains("`records`"), "{loom}");
    assert!(!loom.contains("TRIAL"), "{loom}");
}

/// A key written twice is refused, in either format, whichever value comes last: a file that
/// says both `trial` and `default` names no one profile.
#[test]
fn a_records_key_written_twice_is_refused() {
    let dir = scratch("twice");
    for text in [
        "records = \"trial\"\nrecords = \"default\"\n",
        "records = \"default\"\nrecords = \"trial\"\n",
    ] {
        refusal(&dir, "run.toml", text);
    }
    for text in [
        "records: trial\nrecords: default\n",
        "records: default\nrecords: trial\n",
    ] {
        refusal(&dir, "run.yaml", text);
    }
    refusal(
        &dir,
        "agent.yaml",
        "format: loom.agent/1\ntakt:\n  records: trial\n  records: default\n",
    );
}

/// The two values are read in both formats as they are written, so the flag's parser and the
/// file's agree on the spelling.
#[test]
fn default_and_trial_are_read_in_either_format() {
    let dir = scratch("read");
    for (file, text, want) in [
        ("run.toml", "records = \"trial\"\n", "trial"),
        ("run.toml", "records = \"default\"\n", "default"),
        ("run.yaml", "records: trial\n", "trial"),
        ("run.yml", "records: \"default\"\n", "default"),
    ] {
        let path = dir.join(file);
        fs::write(&path, text).expect("a file");
        let config = RunConfig::read(&path).expect("read");
        assert_eq!(config.records.as_deref(), Some(want), "{file} {text:?}");
    }
}
