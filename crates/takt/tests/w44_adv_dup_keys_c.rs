//! Adversarial cases of wave 44, U3, third pass: story:duplicate-key-reader-limits after the fix
//! of the second pass, driven from the story's "Decision (wave 44, review)": a tagged key's value
//! is built from its resolved tag (core tags by serde's rules, a local tag as a tagged value
//! around the plain-resolved text, any other tag as a string), and a key the reader cannot
//! resolve stops it, so the generic refusal stands rather than a later pair.

use std::path::{Path, PathBuf};

use takt::run_config::RunConfig;

/// A scratch directory under this crate's build directory, one per case.
fn dir(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("w44-u3c-adv-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    dir
}

/// Writes `text` as `<dir(name)>/run.yaml` and returns its path.
fn file(name: &str, text: &str) -> PathBuf {
    let path = dir(name).join("run.yaml");
    std::fs::write(&path, text).expect("a configuration file");
    path
}

/// `RunConfig::read`'s refusal rendered as the CLI prints it.
fn refusal(path: &Path) -> String {
    format!("{:#}", RunConfig::read(path).expect_err("refused"))
}

/// The refusal of `text` holds `want`.
fn refused_with(name: &str, text: &str, want: &str) {
    let got = refusal(&file(name, text));
    assert!(
        got.contains(want),
        "{name}: {text:?}\n  want: {want:?}\n  got:  {got}"
    );
}

const LOCAL_TAG: &str = "!x iterations: 1\niterations: 2\niterations: 3\n";
const GLOBAL_TAG: &str = "!<tag:example.com,2000:x> 1: a\n1: b\n1: c\n";
const UNRESOLVED: &str = "? a\n\n  b\n: 1\n\"a\\nb\": 2\n\"a\\nb\": 3\n";

// ---- red candidates -------------------------------------------------------------------------

/// Decision: "a local tag (`!x`) as a tagged value around the plain-resolved text". serde_yaml_ng
/// holds `!x iterations` as a tagged value, distinct from the string `iterations`, so the
/// duplicate is on line 3. A reader that resolves the local-tagged key as its plain text finds
/// `iterations` twice at line 2.
#[test]
fn adv_w44_u3c_a_local_tagged_key_is_not_the_plain_key() {
    refused_with(
        "local-tag",
        LOCAL_TAG,
        "line 3: it holds the key `iterations` twice",
    );
}

/// Decision: "any other tag as a string". serde_yaml_ng reads `!<tag:example.com,2000:x> 1` as
/// the string "1" (a global tag that is not a core tag), distinct from the number 1, so the
/// duplicate is on line 3. A reader that resolves the key as plain text reads the number 1 and
/// reports line 2.
#[test]
fn adv_w44_u3c_a_global_non_core_tag_key_is_a_string() {
    refused_with("global-tag", GLOBAL_TAG, "line 3: it holds a key twice");
}

/// Decision: "A key the reader cannot resolve stops it, as it did before the first fix: it gives
/// up and the generic refusal stands, rather than reporting a later pair." The explicit plain
/// key on lines 1-3 folds to "a\nb", which the reader's plain resolution does not read back;
/// serde_yaml_ng holds it equal to the quoted key on line 5, its duplicate. A reader that skips
/// the unresolved key reports the later pair at line 6.
#[test]
fn adv_w44_u3c_an_unresolved_key_stops_the_reader() {
    let got = refusal(&file("unresolved", UNRESOLVED));
    assert!(
        !got.contains("line 6"),
        "an unresolved key must stop the reader, not report the later pair: {got}"
    );
}

// ---- premises -------------------------------------------------------------------------------

/// The premises of the three cases above, from serde_yaml_ng itself: the first two keys of each
/// tag case are distinct (two keys parse), and the unresolved key's duplicate is named.
#[test]
fn adv_w44_u3c_serde_premises() {
    for text in [
        "!x iterations: 1\niterations: 2\n",
        "!<tag:example.com,2000:x> 1: a\n1: b\n",
    ] {
        let value: serde_yaml_ng::Value = serde_yaml_ng::from_str(text).expect("two keys");
        assert_eq!(value.as_mapping().map(|m| m.len()), Some(2), "{text:?}");
    }
    let error =
        serde_yaml_ng::from_str::<serde_yaml_ng::Value>(UNRESOLVED).expect_err("a duplicate");
    assert_eq!(error.to_string(), "duplicate entry with key \"a\\nb\"");
    let read: Result<serde_yaml_ng::Mapping, _> = serde_yaml_ng::from_str("k: a\nb");
    assert!(
        read.is_err(),
        "the folded key's text does not read back as a value"
    );
}

// ---- green probes ---------------------------------------------------------------------------

/// A UTF-8 byte order mark: libyaml skips it and counts its three bytes in the mark index.
#[test]
fn adv_w44_u3c_byte_order_mark() {
    refused_with(
        "bom",
        "\u{FEFF}iterations: 1\nhours: 1.0\niterations: 2\n",
        "line 3: it holds the key `iterations` twice",
    );
}

/// `\r\r\n` is two breaks.
#[test]
fn adv_w44_u3c_cr_then_crlf() {
    refused_with(
        "cr-crlf",
        "iterations: 1\r\r\nhours: 1.0\r\niterations: 2\r\n",
        "line 4: it holds the key `iterations` twice",
    );
}

/// Floats hash alike in serde_yaml_ng; the set finds `.nan` as serde does.
#[test]
fn adv_w44_u3c_nan() {
    refused_with(
        "nan",
        ".nan: 1\nb: 2\n.NaN: 3\n",
        "line 3: it holds a key twice",
    );
}

/// Core tags by serde's rules: `!!float 1` is the float 1.0, `!!bool` a boolean, `!!null null`
/// null.
#[test]
fn adv_w44_u3c_core_tags_by_serde_rules() {
    refused_with(
        "float",
        "!!float 1: a\nb: 2\n1.0: c\n",
        "line 3: it holds a key twice",
    );
    refused_with(
        "bool",
        "!!bool 'true': a\nb: 2\ntrue: c\n",
        "line 3: it holds a key twice",
    );
    refused_with(
        "null",
        "!!null 'null': a\nb: 2\n~: c\n",
        "line 3: it holds a key twice",
    );
}
