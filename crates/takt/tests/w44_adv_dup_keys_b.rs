//! Adversarial cases of wave 44, U3, second pass: story:duplicate-key-reader-limits after the
//! fix of the first pass (`repeated_key` in `crates/takt/src/run_config.rs` now counts a key's
//! line from libyaml's `start_mark.index`, re-reads a tagged key from its source text, and skips
//! a key it cannot resolve). libyaml's mark index counts bytes; the reader walks `text.chars()`.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use takt::run_config::RunConfig;

/// A scratch directory under this crate's build directory, one per case.
fn dir(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("w44-u3b-adv-{name}-{}", std::process::id()));
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

// ---- red candidates -------------------------------------------------------------------------

/// Non-ASCII text before the duplicate (a German comment, 16 bytes more than characters): the
/// reader takes the byte index as a count of characters, walks past the duplicate's line end and
/// counts one line too many. No tag, no alias: a plain file.
#[test]
fn adv_w44_u3b_non_ascii_before_the_duplicate_keeps_its_line() {
    refused_with(
        "non-ascii-comment",
        "# Prüfläufe für Größenänderungen: Übergänge, Maßstäbe, Rückläufe — grüße\n\
         iterations: 1\nhours: 1.0\nhours: 2.0\n",
        "line 4: it holds the key `hours` twice",
    );
}

/// Line endings of a lone carriage return: libyaml, and every editor, count each as a line
/// break; the reader counts only `\n`, so every duplicate is on line 1.
#[test]
fn adv_w44_u3b_carriage_return_line_endings_keep_the_line() {
    refused_with(
        "cr-only",
        "iterations: 1\rhours: 1.0\riterations: 2\r",
        "line 3: it holds the key `iterations` twice",
    );
}

/// Twelve two-byte characters before a tagged key: the source slice of `!!str abcd` is taken
/// twelve characters late, lands on the value `iterations`, and the reader reads the key `abcd`
/// as `iterations`, reporting line 3 where serde_yaml_ng's duplicate is on line 4.
#[test]
fn adv_w44_u3b_a_tagged_key_after_non_ascii_text_is_read_from_its_own_text() {
    refused_with(
        "tag-shifted",
        "test_cmd: \"éééééééééééé\"\n!!str abcd: iterations\niterations: 1\niterations: 2\n",
        "line 4: it holds the key `iterations` twice",
    );
}

/// A tagged key holding non-ASCII text: the slice takes as many characters as the key has
/// bytes, reads `!!str größe: ` as a mapping, and the duplicate of lines 1 and 2 is reported at
/// line 3.
#[test]
fn adv_w44_u3b_a_tagged_non_ascii_key_is_read_whole() {
    refused_with(
        "tag-non-ascii",
        "!!str größe: 1\ngröße: 2\ngröße: 3\n",
        "line 2: it holds a key twice",
    );
}

/// A tag handle declared by a `%TAG` directive: the tagged key's text alone does not parse, the
/// reader now skips it instead of stopping, and reports the next pair (line 5) where
/// serde_yaml_ng's duplicate is on line 4.
#[test]
fn adv_w44_u3b_a_directive_tag_handle_is_resolved() {
    refused_with(
        "tag-directive-handle",
        "%TAG !e! tag:example.com,2000:\n---\n!e!x iterations: 1\niterations: 2\niterations: 3\n",
        "line 4: it holds the key `iterations` twice",
    );
}

/// `%TAG !` set to the core schema: `!int 1` is the number 1 to serde_yaml_ng, and the
/// reader's text alone reads it as a local tag, so it reports line 5 for serde's line 4.
#[test]
fn adv_w44_u3b_a_redefined_primary_handle_is_resolved() {
    refused_with(
        "tag-directive-primary",
        "%TAG ! tag:yaml.org,2002:\n---\n!int 1: a\n1: b\n1: c\n",
        "line 4: it holds a key twice",
    );
}

/// The line count walks the text from its start for every event, quadratic in the file: 40 000
/// keys and a duplicate at the end. The refusal comes within 20 s.
#[test]
fn adv_w44_u3b_a_large_file_is_refused_in_bounded_time() {
    let mut text = String::new();
    for index in 0..40_000 {
        text.push_str(&format!("k{index}: 1\n"));
    }
    text.push_str("k0: 2\n");
    let path = file("large", &text);
    let started = Instant::now();
    let got = refusal(&path);
    let took = started.elapsed();
    assert!(got.contains("line 40001: it holds a key twice"), "{got}");
    assert!(took < Duration::from_secs(20), "took {took:?}");
}

// ---- green probes ---------------------------------------------------------------------------

#[test]
fn adv_w44_u3b_anchor_on_a_mapping_then_on_a_scalar() {
    refused_with(
        "anchor-mapping-then-scalar",
        "a: &k {x: 1}\nb: &k iterations\niterations: 1\n*k : 2\n",
        "line 4: it holds the key `iterations` twice",
    );
}

#[test]
fn adv_w44_u3b_alias_of_a_mapping_as_a_key_before_the_duplicate() {
    refused_with(
        "alias-mapping-key",
        "a: &k {x: 1}\n*k : 1\niterations: 1\niterations: 2\n",
        "line 4: it holds the key `iterations` twice",
    );
}

#[test]
fn adv_w44_u3b_anchored_tagged_key_and_alias() {
    refused_with(
        "anchored-tagged",
        "&k !!str iterations: 1\nhours: 1.0\n*k : 2\n",
        "line 3: it holds the key `iterations` twice",
    );
}

#[test]
fn adv_w44_u3b_tagged_quoted_multi_line_key() {
    refused_with(
        "tagged-quoted-multi-line",
        "? !!str \"itera\n  tions\"\n: 1\n? \"itera tions\"\n: 2\n",
        "line 4: it holds a key twice",
    );
}

#[test]
fn adv_w44_u3b_tagged_key_in_a_flow_mapping() {
    refused_with(
        "tagged-flow",
        "{!!str iterations: 1,\n hours: 1.0,\n iterations: 2}\n",
        "line 3: it holds the key `iterations` twice",
    );
}
