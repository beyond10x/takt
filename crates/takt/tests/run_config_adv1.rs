//! Adversarial cases for story:one-config-contract-with-loom, library side: `RunConfig::read`
//! driven with YAML-specific inputs and compared with the TOML route, which the story says takes
//! the same fields (`takt.work.RunConfiguration`, `ess/domains/takt.yaml`).

use std::path::{Path, PathBuf};

use takt::run_config::RunConfig;

const MARKER: &str = "MARKER-adv1-u3-7c41";

/// A scratch directory under this crate's build directory, one per case.
fn dir(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("run-config-adv1-{name}-{}", std::process::id()));
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

/// `RunConfig::read`'s doc comment: "a value of another type, is refused". TOML refuses a
/// number or a boolean where a string field is; YAML must too, or the two forms do not take
/// "the same fields".
#[test]
fn a_string_field_refuses_a_number_or_boolean_in_yaml_as_in_toml() {
    for (index, (toml, yaml)) in [
        ("model = 5\n", "model: 5\n"),
        ("harness = true\n", "harness: true\n"),
        ("test_cmd = 1.5\n", "test_cmd: 1.5\n"),
        ("forge = 0\n", "forge: 0\n"),
    ]
    .into_iter()
    .enumerate()
    {
        let t = file(&format!("type-{index}"), "run.toml", toml);
        assert!(RunConfig::read(&t).is_err(), "TOML {toml:?} is refused");
        let y = file(&format!("type-{index}"), "run.yaml", yaml);
        let read = RunConfig::read(&y);
        assert!(
            read.is_err(),
            "YAML {yaml:?} is refused as TOML {toml:?} is, but read as {read:?}"
        );
    }
}

/// A YAML tag is refused, as Loom's decoder refuses one on any value ("nothing is ignored",
/// loom 0.42.0 `crates/loom-sdk/src/agent.rs` `untagged`); takt drops the tag and reads the
/// tagged value as if it were plain.
#[test]
fn a_yaml_tag_is_refused_not_dropped() {
    for (index, text) in [
        format!("model: !secret {MARKER}\n"),
        format!("format: loom.agent/1\ntakt: !custom {{model: {MARKER}}}\n"),
    ]
    .iter()
    .enumerate()
    {
        let path = file(&format!("tag-{index}"), "run.yaml", text);
        let read = RunConfig::read(&path);
        assert!(read.is_err(), "{text:?} is refused, but read as {read:?}");
    }
}

/// A duplicate key is refused in either form at the duplicate's line, 3, naming the key; YAML
/// must not name line 1, where its mapping starts (serde_yaml_ng reports a duplicate there).
#[test]
fn a_duplicate_key_is_refused_at_its_line_in_yaml_as_in_toml() {
    let t = file(
        "dup",
        "run.toml",
        "iterations = 1\nhours = 1.0\niterations = 2\n",
    );
    let toml = refusal(&t);
    assert!(
        toml.contains("line 3") && toml.contains("iterations"),
        "TOML: {toml}"
    );
    let y = file(
        "dup",
        "run.yaml",
        "iterations: 1\nhours: 1.0\niterations: 2\n",
    );
    let yaml = refusal(&y);
    assert!(
        yaml.contains("line 3") && !yaml.contains("line 1:") && yaml.contains("iterations"),
        "YAML names line 3 and the key as TOML does: {yaml}"
    );
}

/// A `loom.agent/1` document's members are Loom's thirteen (loom 0.42.0
/// `crates/loom-sdk/src/agent.rs` `spec`) and `takt`. A run field written beside `format`
/// instead of under `takt`, or `takt` misspelt, is neither Loom's nor takt's; it must be
/// refused, not silently dropped, or `iterations: 0` and `harness: none` are lost and the run
/// takes the defaults (`--harness claude`, no step budget).
#[test]
fn a_member_neither_looms_nor_takts_is_refused_not_dropped() {
    for (index, text) in [
        "format: loom.agent/1\nagent: {id: a, name: A, brief: b}\nmodel: {routes: routes.yaml}\niterations: 0\nharness: none\n",
        "format: loom.agent/1\nagent: {id: a, name: A, brief: b}\nTakt: {iterations: 0, harness: none}\n",
    ]
    .into_iter()
    .enumerate()
    {
        let path = file(&format!("misplaced-{index}"), "agent.yaml", text);
        let read = RunConfig::read(&path);
        assert!(
            read.is_err(),
            "{text:?}: a member neither Loom's nor takt's is refused, but read as {read:?}"
        );
    }
}

/// A byte order mark is read alike in every form: a plain TOML and a plain YAML file with one
/// read (checked by the probe this case was written from), and a `loom.agent/1` document with
/// one must give its `takt` member.
#[test]
fn a_byte_order_mark_loom_agent_document_gives_its_takt_member() {
    let plain = file("bom", "run.yaml", "\u{feff}iterations: 2\n");
    assert_eq!(RunConfig::read(&plain).expect("read").iterations, Some(2));
    let agent = file(
        "bom",
        "agent.yaml",
        "\u{feff}format: loom.agent/1\ntakt:\n  iterations: 2\n",
    );
    let read = RunConfig::read(&agent).map_err(|error| format!("{error:#}"));
    assert_eq!(read.map(|config| config.iterations), Ok(Some(2)));
}

/// The wave-37 review: `strict` reads YAML through JSON, which has no infinity or NaN, so
/// `hours: .inf` came back as no `hours` at all, an unbounded run where TOML's `hours = inf`
/// reaches `--hours`' own check. A number that is not finite is refused at the read.
#[test]
fn a_yaml_number_that_is_not_finite_is_refused_not_dropped() {
    for (index, yaml) in [
        "hours: .inf\n",
        "hours: -.inf\n",
        "hours: .nan\n",
        "format: loom.agent/1\ntakt:\n  hours: .inf\n",
    ]
    .into_iter()
    .enumerate()
    {
        let path = file(&format!("finite-{index}"), "run.yaml", yaml);
        assert!(RunConfig::read(&path).is_err(), "{yaml:?} was read");
    }
    let path = file("finite-ok", "run.yaml", "hours: 1.5\n");
    assert_eq!(RunConfig::read(&path).expect("read").hours, Some(1.5));
}
