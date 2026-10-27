//! Adversary cases on story:the-state-directory-holds-every-host-record, the library half: how
//! takt reads a `loom.agent/1` document's `runtime` member. Loom declares `runtime.state_dir` as
//! text (`loom.agent.RuntimeSpec`) and refuses a value that is not text, or is empty, and a
//! member of `runtime` it does not declare; takt's reader says a value of another type is
//! refused, naming the file and never the value.

use std::fs;
use std::path::{Path, PathBuf};

use takt::run_config::RunConfig;

/// Writes `text` as `<scratch>/<name>/<file>` and returns its path.
fn document(name: &str, file: &str, text: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "run-config-state-dir-adv1-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("a directory");
    let path = dir.join(file);
    fs::write(&path, text).expect("a run configuration file");
    path
}

/// `runtime.state_dir` written as a number or a boolean is not text: refused, as `takt.state_dir`
/// is and as Loom refuses it, naming the file and never the value. Not read as a directory
/// called `4242` beside the file.
#[test]
fn a_loom_runtime_state_dir_that_is_not_text_is_refused_without_its_value() {
    for (name, value) in [
        ("number", "4242"),
        ("boolean", "true"),
        ("fraction", "17.5"),
    ] {
        let path = document(
            &format!("not-text-{name}"),
            "agent.yaml",
            &format!("format: loom.agent/1\nruntime:\n  state_dir: {value}\n"),
        );
        let read = RunConfig::read(&path);
        let error = match read {
            Ok(config) => panic!(
                "a {name} as runtime.state_dir is refused, not read as {:?}",
                config.state_dir
            ),
            Err(error) => format!("{error:#}"),
        };
        assert!(error.contains(&path.display().to_string()), "{error}");
        assert!(!error.contains(value), "{error}");
    }
}

/// The same value as `takt.state_dir` is refused: the two members of one document are read with
/// the same strictness.
#[test]
fn a_takt_state_dir_that_is_a_number_is_refused() {
    let path = document(
        "takt-number",
        "agent.yaml",
        "format: loom.agent/1\ntakt:\n  state_dir: 4242\n",
    );
    assert!(
        RunConfig::read(&path).is_err(),
        "takt.state_dir: 4242 is refused"
    );
}

/// An empty or blank `runtime.state_dir` names no directory: refused, as Loom refuses it
/// (`is empty`). Not read as the configuration file's own directory, which `/config` is in a
/// pod, so the run's records would go beside the mounted document.
#[test]
fn an_empty_loom_runtime_state_dir_is_refused() {
    for (name, value) in [("empty", "\"\""), ("blank", "\"   \"")] {
        let path = document(
            &format!("empty-{name}"),
            "agent.yaml",
            &format!("format: loom.agent/1\nruntime:\n  state_dir: {value}\n"),
        );
        if let Ok(config) = RunConfig::read(&path) {
            panic!(
                "an {name} runtime.state_dir is refused, not read as {:?} (the file's directory is {})",
                config.state_dir,
                path.parent().expect("a directory").display()
            );
        }
    }
}

/// A member of `runtime` that Loom does not declare (`state_dir`, `connectors`, `confinement`),
/// such as `state_dir` misspelt, is refused, as a top-level member that is neither Loom's nor
/// `takt` is: dropped, the run would keep its records under the state home, which a pod whose
/// home is not kept loses.
#[test]
fn a_misspelt_loom_runtime_member_is_refused() {
    let path = document(
        "misspelt",
        "agent.yaml",
        "format: loom.agent/1\nruntime:\n  state_dri: /data/takt\n",
    );
    if let Ok(config) = RunConfig::read(&path) {
        panic!(
            "runtime.state_dri is refused, not dropped (state_dir read as {:?})",
            config.state_dir
        );
    }
}

/// Loom's own members of `runtime` are not refused, and `state_dir` is read beside them.
#[test]
fn loom_runtime_members_beside_state_dir_are_not_refused() {
    let path = document(
        "members",
        "agent.yaml",
        "format: loom.agent/1\nruntime:\n  state_dir: /data/takt\n  connectors:\n    program: connectors\n    state_dir: /data/connectors\n    timeout_seconds: 30\n  confinement: none\n",
    );
    assert_eq!(
        RunConfig::read(&path).expect("read").state_dir,
        Some(PathBuf::from("/data/takt"))
    );
}

/// A `runtime` that is not a mapping is refused, naming the file and never what is written.
#[test]
fn a_loom_runtime_that_is_not_a_mapping_is_refused_without_its_value() {
    for (name, value) in [
        ("scalar", "MARKER-runtime"),
        ("sequence", "[MARKER-runtime]"),
    ] {
        let path = document(
            &format!("runtime-{name}"),
            "agent.yaml",
            &format!("format: loom.agent/1\nruntime: {value}\n"),
        );
        let error = format!("{:#}", RunConfig::read(&path).expect_err("refused"));
        assert!(error.contains(&path.display().to_string()), "{error}");
        assert!(!error.contains("MARKER"), "{error}");
    }
}

/// `runtime.state_dir` holding a mapping is refused without repeating a key or value of it.
#[test]
fn a_loom_runtime_state_dir_that_is_a_mapping_is_refused_without_its_value() {
    let path = document(
        "state-dir-mapping",
        "agent.yaml",
        "format: loom.agent/1\nruntime:\n  state_dir: {MARKER-key: MARKER-value}\n",
    );
    let error = format!("{:#}", RunConfig::read(&path).expect_err("refused"));
    assert!(!error.contains("MARKER"), "{error}");
}

/// `runtime` is a member of a `loom.agent/1` document only: a TOML run file, or a YAML run file
/// with no `format`, that holds it is refused, never the value repeated.
#[test]
fn runtime_outside_a_loom_agent_document_is_refused_without_its_value() {
    let toml = document(
        "toml",
        "run.toml",
        "[runtime]\nstate_dir = \"/MARKER/state\"\n",
    );
    let error = format!("{:#}", RunConfig::read(&toml).expect_err("refused"));
    assert!(!error.contains("MARKER"), "{error}");
    let yaml = document("yaml", "run.yaml", "runtime:\n  state_dir: /MARKER/state\n");
    let error = format!("{:#}", RunConfig::read(&yaml).expect_err("refused"));
    assert!(!error.contains("MARKER"), "{error}");
}
