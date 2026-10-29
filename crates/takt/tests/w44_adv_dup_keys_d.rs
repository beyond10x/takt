//! Adversarial cases of wave 44, U3, fourth pass: story:duplicate-key-reader-limits after the fix
//! of the third pass, driven from the story's "Decision (wave 44, third review)": a local-tagged
//! key is a tagged value and never equals the untagged key; a global non-core tag is the string
//! of its text; an alias whose anchor is, at that point of the file, a mapping or a sequence is
//! passed over, and a key skipped this way and a later key serde finds equal to it give the
//! generic refusal.

use std::path::{Path, PathBuf};

use takt::run_config::RunConfig;

/// A scratch directory under this crate's build directory, one per case.
fn dir(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("w44-u3d-adv-{name}-{}", std::process::id()));
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

/// The refusal of `text` is the generic one: it names no line of a key held twice.
fn refused_generic(name: &str, text: &str) {
    let got = refusal(&file(name, text));
    assert!(
        !got.contains("twice"),
        "{name}: {text:?}\n  want the generic refusal\n  got:  {got}"
    );
}

const TWO_ALIASES: &str = "a: &k [x]\n*k : 1\n*k : 2\niterations: 1\niterations: 2\n";
const COMPLEX_THEN_ALIAS: &str = "? &k [x]\n: 1\niterations: 1\n*k : 2\n";
const NESTED: &str = "a: &m {x: 1}\nb: &n [*m]\n*n : 1\n*m : 2\niterations: 1\niterations: 2\n";
const ONE_PAIR: &str = "&k iterations: &k [x]\n*k : 1\niterations: 2\n";
const STALE_ID: &str =
    "a: &k iterations\nb: &k {x: 1}\nc: &j iterations\niterations: 1\n*k : 2\niterations: 3\n";

// ---- alias of a collection ------------------------------------------------------------------

/// Third decision: "A key the reader skips this way and a later key that serde finds equal to it
/// give the generic refusal." Two aliases of one sequence: serde's first duplicate is the
/// sequence key on line 3, so the `iterations` pair on line 5 is not the one it refused.
#[test]
fn adv_w44_u3d_two_aliases_of_one_collection_then_a_duplicate() {
    refused_generic("two-aliases", TWO_ALIASES);
}

/// As above, the skipped key being an anchored complex key and the later one its alias.
#[test]
fn adv_w44_u3d_alias_of_an_anchored_complex_key() {
    refused_generic("complex-then-alias", COMPLEX_THEN_ALIAS);
}

/// Nested aliases: `*n` is a sequence holding the alias `*m` of a mapping; both are passed over
/// and the real duplicate is on line 6.
#[test]
fn adv_w44_u3d_nested_aliases_of_collections_then_a_duplicate() {
    refused_with(
        "nested",
        NESTED,
        "line 6: it holds the key `iterations` twice",
    );
}

/// An anchor set on a key and set again on its own value, a sequence: `*k` on line 2 is the
/// sequence, so the duplicate is `iterations` on line 3.
#[test]
fn adv_w44_u3d_anchor_on_a_key_redefined_on_its_value() {
    refused_with(
        "one-pair",
        ONE_PAIR,
        "line 3: it holds the key `iterations` twice",
    );
}

/// An anchor set on a scalar, then a sequence, then a scalar again: `*k` is the latest, `hours`.
#[test]
fn adv_w44_u3d_anchor_scalar_collection_scalar() {
    refused_with(
        "scalar-collection-scalar",
        "a: &k iterations\nb: &k [x]\nc: &k hours\niterations: 1\n*k : 2\nhours: 3\n",
        "line 6: it holds the key `hours` twice",
    );
}

/// The decision's "at that point of the file": `*k` on line 5 is the mapping `{x: 1}`, so the
/// reader passes it and reports line 6. serde_yaml_ng 0.10 itself resolves `*k` here to the
/// node anchored `j` (see the premise below), so its own duplicate is on line 5.
#[test]
fn adv_w44_u3d_redefined_anchor_then_a_new_anchor() {
    refused_with(
        "stale-id",
        STALE_ID,
        "line 6: it holds the key `iterations` twice",
    );
}

// ---- tagged keys ----------------------------------------------------------------------------

/// Third decision, first rule, for the non-specific tag `!`: serde_yaml_ng holds `! iterations`
/// as a value tagged `!`, so the duplicate is on line 3.
#[test]
fn adv_w44_u3d_a_non_specific_tag_is_not_the_plain_key() {
    refused_with(
        "non-specific",
        "! iterations: 1\niterations: 2\niterations: 3\n",
        "line 3: it holds the key `iterations` twice",
    );
}

/// Third decision, first rule, where `!!` is redefined to the local prefix `!`: `!!str` is then
/// the local tag `!str`, not the core `str`, so the duplicate is on line 5; and a quoted
/// local-tagged key is a tagged string, so in the second file the duplicate is on line 3.
#[test]
fn adv_w44_u3d_local_tags_by_handle_and_on_a_quoted_key() {
    refused_with(
        "local-secondary",
        "%TAG !! !\n---\n!!str iterations: 1\niterations: 2\niterations: 3\n",
        "line 5: it holds the key `iterations` twice",
    );
    refused_with(
        "local-quoted",
        "!x 'iterations': 1\niterations: 2\niterations: 3\n",
        "line 3: it holds the key `iterations` twice",
    );
}

// ---- premises -------------------------------------------------------------------------------

/// The premises above, from serde_yaml_ng itself.
#[test]
fn adv_w44_u3d_serde_premises() {
    let message = |text: &str| {
        serde_yaml_ng::from_str::<serde_yaml_ng::Value>(text)
            .expect_err("a duplicate")
            .to_string()
    };
    assert!(
        message(TWO_ALIASES).starts_with("duplicate entry in YAML map"),
        "{}",
        message(TWO_ALIASES)
    );
    assert!(
        message(COMPLEX_THEN_ALIAS).starts_with("duplicate entry in YAML map"),
        "{}",
        message(COMPLEX_THEN_ALIAS)
    );
    for text in [
        "! iterations: 1\niterations: 2\n",
        "%TAG !! !\n---\n!!str iterations: 1\niterations: 2\n",
        "!x 'iterations': 1\niterations: 2\n",
        "&k iterations: &k [x]\n*k : 1\n",
        "a: &m {x: 1}\nb: &n [*m]\n*n : 1\n*m : 2\niterations: 1\n",
    ] {
        let value: serde_yaml_ng::Value = serde_yaml_ng::from_str(text).expect("no duplicate");
        assert!(value.as_mapping().is_some(), "{text:?}");
    }
    // serde_yaml_ng 0.10's loader numbers an anchor `anchors.len()`; a redefined anchor does not
    // grow the map, so the next new anchor takes the same number and `*k` names `j`'s node.
    let value: serde_yaml_ng::Value =
        serde_yaml_ng::from_str("a: &k iterations\nb: &k {x: 1}\nc: &j other\nd: *k\n")
            .expect("a mapping");
    assert_eq!(
        value.get("d"),
        Some(&serde_yaml_ng::Value::String("other".to_owned())),
        "serde_yaml_ng resolves `*k` to the node anchored `j`"
    );
}
