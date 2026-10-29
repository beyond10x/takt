//! Adversarial cases for story:a-duplicate-yaml-key-is-refused-at-its-line, second round: a
//! refusal says "holds a key twice" only when the parser found a key twice, and then at the
//! line of the key it found, naming it only when takt or Loom declares it, in YAML as in TOML.

use std::path::{Path, PathBuf};

use takt::run_config::RunConfig;

/// A scratch directory under this crate's build directory, one per case.
fn dir(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("run-config-dup-adv2-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    dir
}

/// Writes `text` as `<dir(name)>/<file>` and returns its path.
fn file(name: &str, file: &str, text: &str) -> PathBuf {
    let path = dir(name).join(file);
    std::fs::write(&path, text).expect("a configuration file");
    path
}

/// `RunConfig::read`'s refusal rendered as the CLI prints it.
fn refusal(path: &Path) -> String {
    format!("{:#}", RunConfig::read(path).expect_err("refused"))
}

/// A file that holds no key twice, refused for a syntax error, is refused at that error's line
/// and not as holding a key twice, wherever text inside a scalar reads like `key: value`.
#[test]
fn a_syntax_error_is_not_reported_as_a_duplicate_key() {
    for (index, (text, want)) in [
        (
            "setup:\n  - name: deps\n    argv:\n      - |\n        echo step: one\n        echo step: two\nmodel: x\n  bad: y\n",
            "line 8",
        ),
        // A plain scalar's continuation line cannot hold `: `.
        (
            "test_cmd: cargo test\n  story: a\n  story: b\niterations: 1\niterations: 2\n",
            "line 2",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let y = refusal(&file(&format!("syntax-{index}"), "run.yaml", text));
        assert!(
            !y.contains("twice") && y.contains(want),
            "{text:?}: a syntax error at {want} is refused there, not as a duplicate key: {y}"
        );
    }
}

/// Two documents (`...` ends the first) are refused as more than one document would be, never
/// as one mapping holding a key twice.
#[test]
fn two_documents_are_not_reported_as_a_duplicate_key() {
    for (index, text) in [
        "iterations: 1\n...\niterations: 2\n",
        "iterations: 1\n---\niterations: 2\n",
    ]
    .into_iter()
    .enumerate()
    {
        let y = refusal(&file(&format!("docs-{index}"), "run.yaml", text));
        assert!(!y.contains("twice"), "{text:?}: {y}");
    }
}

/// The real duplicate, `iterations`, is named at its line, not text inside a quoted scalar that
/// reads like `story: …`.
#[test]
fn the_real_duplicate_is_named_not_text_inside_a_quoted_scalar() {
    for (index, (text, want)) in [
        (
            "test_cmd: \"cargo test\n  story: a\n  story: b\"\niterations: 1\niterations: 2\n",
            "line 5",
        ),
        (
            "test_cmd: 'cargo test\n  story: a\n  story: b'\niterations: 1\niterations: 2\n",
            "line 5",
        ),
        (
            "test_cmd: \"{story: a, story: b}\"\niterations: 1\niterations: 2\n",
            "line 3",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let y = refusal(&file(&format!("quoted-{index}"), "run.yaml", text));
        assert!(
            y.contains(want) && y.contains("`iterations` twice") && !y.contains("`story`"),
            "{text:?}: want {want}: names the wrong line or key: {y}"
        );
    }
}

/// The real duplicate is named, not lines of a block scalar that is a sequence item (`- |`) or
/// that follows an anchor or a tag.
#[test]
fn the_real_duplicate_is_named_not_lines_of_a_block_scalar() {
    for (index, (text, want)) in [
        (
            "setup:\n  - name: deps\n    argv:\n      - sh\n      - -c\n      - |\n        echo step: one\n        echo step: two\niterations: 1\niterations: 2\n",
            "line 10",
        ),
        (
            "test_cmd: &cmd |\n  story: a\n  story: b\niterations: 1\niterations: 2\n",
            "line 5",
        ),
        (
            "test_cmd: !!str |\n  story: a\n  story: b\niterations: 1\niterations: 2\n",
            "line 5",
        ),
        (
            "test_cmd: >-\n  story: a\n\n  story: b\niterations: 1\niterations: 2\n",
            "line 6",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let y = refusal(&file(&format!("block-{index}"), "run.yaml", text));
        assert!(
            y.contains(want) && y.contains("`iterations` twice") && !y.contains("`story`"),
            "{text:?}: want {want} and `iterations`: {y}"
        );
    }
}

/// A refusal that is not a duplicate says nothing of a key twice, so "names the key" is
/// asserted by "`iterations` twice", which the shape text alone never holds.
#[test]
fn a_non_duplicate_refusal_names_no_key_twice() {
    let y = refusal(&file(
        "shape",
        "run.yaml",
        "hours: 1.0\nmodel: x\n  bad: y\n",
    ));
    assert!(!y.contains("twice"), "{y}");
    let t = refusal(&file("shape", "run.toml", "iterations = 1\nhours = [\n"));
    assert!(!y.contains("twice") && !t.contains("twice"), "{t}");
}

/// TOML names the duplicate key as YAML does, from the key the toml parser points at, bare or
/// quoted, in the root table, a table or a repeated header.
#[test]
fn toml_names_the_duplicate_key_at_its_line() {
    for (index, (text, want)) in [
        (
            "iterations = 1\nhours = 1.0\niterations = 2\n",
            "line 3: it holds the key `iterations` twice",
        ),
        (
            "\"iterations\" = 1\niterations = 2\n",
            "line 2: it holds the key `iterations` twice",
        ),
        (
            "model = \"a\"\n'model' = \"b\"\n",
            "line 2: it holds the key `model` twice",
        ),
        (
            "[protocols]\n\n[protocols]\n",
            "line 3: it holds the key `protocols` twice",
        ),
        (
            "[forge_commands]\nmerge = [\"a\"]\nmerge = [\"b\"]\n",
            "line 3: it holds a key twice",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let t = refusal(&file(&format!("toml-{index}"), "run.toml", text));
        assert!(t.contains(want), "{text:?}: want {want:?}: {t}");
    }
}

/// What the YAML side finds, each at the duplicate's own line: block and flow mappings, a
/// sequence item, an explicit key, quoted and plain spellings of one string, keys that are not
/// strings, a comment and a document marker before.
#[test]
fn real_duplicates_are_found_at_their_line() {
    let cases: &[(&str, &str, &str)] = &[
        (
            "nested",
            "protocols:\n  a: {}\n  b: {}\n  a: {}\n",
            "line 4",
        ),
        (
            "siblings_then_dup",
            "commit:\n  x: 1\nforge_commands:\n  x: [a]\nmodel: m\nmodel: n\n",
            "line 6",
        ),
        ("item", "setup:\n  - name: a\n    name: b\n", "line 3"),
        (
            "crlf",
            "iterations: 1\r\nhours: 1.0\r\niterations: 2\r\n",
            "line 3",
        ),
        (
            "trail",
            "iterations : 1\nhours: 1.0\n'iterations': 2  # c\n",
            "line 3",
        ),
        (
            "block",
            "test_cmd: |-\n  iterations: 1\n  iterations: 2\ntest_cmd: x\n",
            "line 4",
        ),
        (
            "folded",
            "story: >+2\n   a: 1\n   a: 2\nstory: y\n",
            "line 4",
        ),
        ("tabsep", "model:\tm\nmodel:\tn\n", "line 2"),
        ("merge", "a: &x {p: 1}\nb:\n  <<: *x\n  <<: *x\n", "line 4"),
        ("flow", "protocols: {a: {}, b: {},\n  a: {}}\n", "line 2"),
        (
            "flowtop",
            "{iterations: 1,\n hours: 1.0,\n iterations: 2}\n",
            "line 3",
        ),
        (
            "explicit",
            "? iterations\n: 1\nhours: 1.0\n? iterations\n: 2\n",
            "line 4",
        ),
        (
            "comment",
            "# iterations: 0\niterations: 1\n# iterations: 0\niterations: 2\n",
            "line 4",
        ),
        ("marker", "---\niterations: 1\niterations: 2\n", "line 3"),
        (
            "number",
            "protocols:\n  1: {}\n  \"1\": {}\n  1: {}\n",
            "line 4",
        ),
        (
            "string",
            "protocols:\n  1: {}\n  \"1\": {}\n  '1': {}\n",
            "line 4",
        ),
        ("null", "protocols:\n  ~: {}\n  null: {}\n", "line 3"),
    ];
    let mut wrong = Vec::new();
    for (name, text, want) in cases {
        let y = refusal(&file(name, "run.yaml", text));
        if !(y.contains(&format!("{want}: it holds")) && y.contains("twice")) {
            wrong.push(format!("{name}: want {want}: {y}"));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// Sibling mappings with the same key, and sequence items with the same key, are valid and
/// read (no refusal).
#[test]
fn the_same_key_in_sibling_mappings_or_items_is_not_refused() {
    let text = "setup:\n  - name: a\n    argv: [x]\n  - name: b\n    argv: [y]\n";
    RunConfig::read(&file("valid", "run.yaml", text)).expect("a valid file is read");
}

/// A value never reaches the refusal: an undeclared duplicate key is not named, in YAML or
/// TOML, and a key that quotes part of the shape text is not taken for a declared one.
#[test]
fn an_undeclared_duplicate_key_is_not_echoed() {
    let undeclared = "unnamed_w40_probe";
    let y = refusal(&file(
        "echo",
        "run.yaml",
        &format!("protocols:\n  {undeclared}: {{}}\n  {undeclared}: {{}}\n"),
    ));
    assert!(!y.contains(undeclared) && y.contains("twice"), "{y}");
    let t = refusal(&file(
        "echo",
        "run.toml",
        &format!("[protocols]\n{undeclared} = {{}}\n{undeclared} = {{}}\n"),
    ));
    assert!(!t.contains(undeclared) && t.contains("twice"), "{t}");
    let steer = "iterations` and `hours";
    let y = refusal(&file(
        "steer",
        "run.yaml",
        &format!("protocols:\n  \"{steer}\": {{}}\n  \"{steer}\": {{}}\n"),
    ));
    assert!(!y.contains("the key `"), "{y}");
}

/// serde_yaml_ng's own error names the duplicate key and locates it at its mapping's start;
/// takt reads the key from that message, so a release that changes it turns this red.
#[test]
fn serde_yaml_ng_names_the_duplicate_key_at_its_mapping() {
    let error = serde_yaml_ng::from_str::<serde_yaml_ng::Value>(
        "hours: 1.0\nprotocols:\n  a: {}\n  b: {}\n  a: {}\n",
    )
    .expect_err("a duplicate");
    assert_eq!(
        error.to_string(),
        "protocols: duplicate entry with key \"a\" at line 3 column 3"
    );
    let at = error.location().expect("a location");
    assert_eq!((at.line(), at.column(), at.index()), (3, 3, 24));
}
