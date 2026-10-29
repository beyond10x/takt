//! Adversarial cases of wave 44, U3: story:duplicate-key-reader-limits, the duplicate-key reader
//! of a YAML run file (`repeated_key`, `plain_scalar` in `crates/takt/src/run_config.rs`) on
//! spellings beyond the three the story names: anchors redefined on a collection, a tag whose
//! text is read back as YAML, custom tags, flow mappings, quoted, merge, complex and multi-line
//! keys, comments, document markers, tabs, CRLF and Unicode line separators.

use std::path::{Path, PathBuf};

use takt::run_config::RunConfig;

/// A scratch directory under this crate's build directory, one per case.
fn dir(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("w44-u3-adv-{name}-{}", std::process::id()));
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

/// An anchor first set on a scalar and then set again on a mapping: YAML's alias names the latest
/// node, and serde_yaml_ng resolves `*k` to the mapping `{x: 1}`, so the duplicate is
/// `iterations` on line 5. The reader records anchors of scalars only, keeps the stale
/// `iterations` for `k`, and must not report line 4.
#[test]
fn adv_w44_u3_anchor_redefined_on_a_mapping_is_not_the_old_scalar() {
    refused_with(
        "stale-anchor-mapping",
        "a: &k iterations\nb: &k {x: 1}\niterations: 1\n*k : 2\niterations: 3\n",
        "line 5: it holds the key `iterations` twice",
    );
}

/// As above with the anchor set again on a sequence.
#[test]
fn adv_w44_u3_anchor_redefined_on_a_sequence_is_not_the_old_scalar() {
    refused_with(
        "stale-anchor-sequence",
        "a: &k iterations\nb: &k [x]\niterations: 1\n*k : 2\niterations: 3\n",
        "line 5: it holds the key `iterations` twice",
    );
}

/// A verbatim tag whose percent-escapes decode to `> iterations #`: serde_yaml_ng reads the key as
/// the string `model`, so the duplicate is `iterations` on line 3. The reader formats the decoded
/// tag into `!<...> model` and parses that text again, where it reads as the string `iterations`.
#[test]
fn adv_w44_u3_a_tag_is_not_read_back_as_yaml_text() {
    refused_with(
        "tag-injection",
        "!<tag:yaml.org,2002:str%3E%20iterations%20%23> model: x\niterations: 1\niterations: 2\n",
        "line 3: it holds the key `iterations` twice",
    );
}

/// Two custom-tagged keys serde_yaml_ng holds distinct (`!foo 1` is the number 1 under the tag,
/// `!foo '1'` the string): the duplicate is `iterations` on line 4. The reader re-serialises a
/// tagged key's text quoted, reads both as the string under the tag and stops at line 2.
#[test]
fn adv_w44_u3_a_custom_tag_on_a_plain_number_is_not_read_as_a_string() {
    refused_with(
        "custom-tag-number",
        "!foo 1: a\n!foo '1': b\niterations: 1\niterations: 2\n",
        "line 4: it holds the key `iterations` twice",
    );
}

/// A tagged key before the duplicate whose tag decodes to a space (`!a%20b`): the reader's
/// `!<!a b>` does not parse, so the reader stops at the tagged key, the case the story fixes.
#[test]
fn adv_w44_u3_a_tag_with_an_escaped_space_before_the_duplicate() {
    refused_with(
        "tag-escaped-space",
        "!a%20b x: 1\niterations: 1\niterations: 2\n",
        "line 3: it holds the key `iterations` twice",
    );
}

/// A U+2028 LINE SEPARATOR inside a quoted value: libyaml counts it as a line break, so the
/// reported line is one past the line `grep -n` and the TOML reader (which count `\n`) give.
#[test]
fn adv_w44_u3_a_unicode_line_separator_does_not_shift_the_line() {
    refused_with(
        "line-separator",
        "test_cmd: \"a\u{2028}b\"\niterations: 1\niterations: 2\n",
        "line 3: it holds the key `iterations` twice",
    );
}

/// The premise of the two cases above that get the generic refusal: serde_yaml_ng's own error
/// for each input is the duplicate `iterations`, so the reader lost a duplicate it was handed.
#[test]
fn adv_w44_u3_serde_finds_the_duplicate_in_the_tag_cases() {
    for text in [
        "!foo 1: a\n!foo '1': b\niterations: 1\niterations: 2\n",
        "!a%20b x: 1\niterations: 1\niterations: 2\n",
    ] {
        let error = serde_yaml_ng::from_str::<serde_yaml_ng::Value>(text).expect_err("refused");
        assert_eq!(
            error.to_string(),
            "duplicate entry with key \"iterations\"",
            "{text:?}"
        );
        assert_eq!(error.location().map(|at| at.line()), Some(1), "{text:?}");
    }
}

// ---- green probes ---------------------------------------------------------------------------

#[test]
fn adv_w44_u3_flow_mapping_alias_key_multi_line() {
    refused_with(
        "flow-alias",
        "{&k iterations: 1,\n hours: 1.0,\n *k : 2}\n",
        "line 3: it holds the key `iterations` twice",
    );
}

#[test]
fn adv_w44_u3_quoted_keys_equal_plain() {
    refused_with(
        "quoted",
        "\"iterations\": 1\nhours: 1.0\n'iterations': 2\n",
        "line 3: it holds the key `iterations` twice",
    );
}

#[test]
fn adv_w44_u3_escaped_double_quoted_key_equals_plain() {
    refused_with(
        "escaped-quoted",
        "\"iter\\x61tions\": 1\nhours: 1.0\niterations: 2\n",
        "line 3: it holds the key `iterations` twice",
    );
}

#[test]
fn adv_w44_u3_merge_key_twice() {
    refused_with(
        "merge",
        "<<: {a: 1}\nhours: 1.0\n<<: {b: 2}\n",
        "line 3: it holds a key twice",
    );
}

#[test]
fn adv_w44_u3_complex_key_repeats_plain() {
    refused_with(
        "complex",
        "iterations: 1\nhours: 1.0\n? iterations\n: 2\n",
        "line 3: it holds the key `iterations` twice",
    );
}

#[test]
fn adv_w44_u3_multi_line_complex_key() {
    refused_with(
        "multi-line",
        "? a\n  b\n: 1\n? a b\n: 2\n",
        "line 4: it holds a key twice",
    );
}

#[test]
fn adv_w44_u3_comments_holding_the_key() {
    refused_with(
        "comments",
        "iterations: 1 # iterations: 9\n# iterations: 3\niterations: 2\n",
        "line 3: it holds the key `iterations` twice",
    );
}

#[test]
fn adv_w44_u3_document_markers_and_directive() {
    refused_with(
        "documents",
        "%YAML 1.1\n---\niterations: 1\niterations: 2\n...\n",
        "line 4: it holds the key `iterations` twice",
    );
}

#[test]
fn adv_w44_u3_tabs_after_the_colon() {
    refused_with(
        "tabs",
        "iterations:\t1\nhours:\t1.0\niterations:\t2\n",
        "line 3: it holds the key `iterations` twice",
    );
}

#[test]
fn adv_w44_u3_crlf_tag_and_alias() {
    refused_with(
        "crlf",
        "!!str model: x\r\n&k iterations: 1\r\n*k : 2\r\n",
        "line 3: it holds the key `iterations` twice",
    );
}

#[test]
fn adv_w44_u3_tagged_duplicate_itself() {
    refused_with(
        "tagged-dup",
        "!!str iterations: 1\nhours: 1.0\niterations: 2\n",
        "line 3: it holds the key `iterations` twice",
    );
}

#[test]
fn adv_w44_u3_int_tag_equals_plain_number() {
    refused_with(
        "int-tag",
        "!!int 1: a\nb: 2\n1: c\n",
        "line 3: it holds a key twice",
    );
}

#[test]
fn adv_w44_u3_two_colon_key() {
    refused_with(
        "two-colon",
        "x::: 1\nhours: 1.0\nx::: 2\n",
        "line 3: it holds a key twice",
    );
}

#[test]
fn adv_w44_u3_alias_of_a_colon_key() {
    refused_with(
        "colon-alias",
        "&k x:: 1\nhours: 1.0\n*k : 2\n",
        "line 3: it holds a key twice",
    );
}

#[test]
fn adv_w44_u3_alias_of_an_anchor_on_a_value() {
    refused_with(
        "value-anchor",
        "test_cmd: &m iterations\niterations: 1\n*m : 2\n",
        "line 3: it holds the key `iterations` twice",
    );
}

#[test]
fn adv_w44_u3_anchor_redefined_on_a_scalar() {
    refused_with(
        "scalar-redefined",
        "a: &k model\nb: &k iterations\niterations: 1\n*k : 2\n",
        "line 4: it holds the key `iterations` twice",
    );
}

#[test]
fn adv_w44_u3_null_keys() {
    refused_with(
        "null",
        "~: 1\nb: 2\nnull: 3\n",
        "line 3: it holds a key twice",
    );
}

#[test]
fn adv_w44_u3_hex_equals_decimal() {
    refused_with(
        "hex",
        "0x10: a\nb: 2\n16: c\n",
        "line 3: it holds a key twice",
    );
}

#[test]
fn adv_w44_u3_tag_with_an_escaped_bang() {
    refused_with(
        "tag-escaped-bang",
        "!a%21b x: 1\niterations: 1\niterations: 2\n",
        "line 3: it holds the key `iterations` twice",
    );
}

#[test]
fn adv_w44_u3_alias_key_in_a_sequence_item() {
    refused_with(
        "sequence-item",
        "commit:\n  - &k iterations: 1\n    *k : 2\n",
        "line 3: it holds the key `iterations` twice",
    );
}

/// No false positive: a valid file with quoted keys and an anchored key is read, and its values
/// are the ones written.
#[test]
fn adv_w44_u3_valid_file_with_quoted_and_anchored_keys_is_read() {
    let path = file(
        "valid",
        "\"iterations\": 3\n&k 'hours': 1.5\n# iterations: 9\n",
    );
    let config = RunConfig::read(&path).expect("a valid file is read");
    assert_eq!(config.iterations, Some(3));
    assert_eq!(config.hours, Some(1.5));
}
