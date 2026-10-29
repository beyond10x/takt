//! The run file's `protocol` and `protocols` through the binary, over a copy of a fixture planning
//! store whose story carries tags (story:the-run-file-sets-protocol-settings): the run file sets an
//! input, a story's `input:` tag overrides it, `--input` overrides the tag, the `protocol` key
//! chooses the protocol a store story opens on and the story's `protocol:` tag overrides the key;
//! each value is recorded with its source in the case's `case.yaml` and said on the run's output.

#[path = "../../takt/tests/support/own_home.rs"]
mod own_home;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The fixture stores of the library crate, one directory each.
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../takt/tests/fixtures");

const STORY: &str = ".engineering/planning/story/ready-one.md";

fn git(tree: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(args)
        .current_dir(tree)
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?}");
}

/// A scratch git repository under this crate's build directory, removed on drop.
struct Scratch(PathBuf);

impl std::ops::Deref for Scratch {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("a directory");
    for entry in fs::read_dir(from).expect("a readable fixture") {
        let entry = entry.expect("an entry");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("a file type").is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).expect("a copied file");
        }
    }
}

/// A committed copy of the `one-ready` fixture store whose story carries `tags`, and the run file
/// `run.yaml` holding `run_file`, beside the repository, not in it.
fn repository(name: &str, tags: &[&str], run_file: &str) -> (Scratch, PathBuf) {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "protocol-settings-cli-{}-{name}",
        std::process::id()
    ));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    let repo = dir.join("repo");
    copy_dir(&Path::new(FIXTURES).join("one-ready"), &repo);
    let story = fs::read_to_string(repo.join(STORY)).expect("the story");
    let tags = tags
        .iter()
        .map(|tag| format!("- {tag}\n"))
        .collect::<String>();
    let story = story.replace(
        "summary: The one ready story\n",
        &format!("summary: The one ready story\ntags:\n{tags}"),
    );
    fs::write(repo.join(STORY), story).expect("a tagged story");
    fs::write(repo.join("check.txt"), "broken\n").expect("a file");
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "start"]);
    let file = dir.join("run.yaml");
    fs::write(&file, run_file).expect("a run file");
    (Scratch(dir), file)
}

/// `takt run` on `repo` with no harness, one iteration and a passing test command, plus `extra`:
/// the exit code, stdout and stderr.
fn run(repo: &Path, config: &Path, extra: &[&str]) -> (i32, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args([
            "run",
            "--repo",
            repo.to_str().expect("utf-8"),
            "--harness",
            "none",
            "--iterations",
            "1",
            "--test-cmd",
            "true",
            "--config",
            config.to_str().expect("utf-8"),
        ])
        .args(extra)
        .output()
        .expect("takt runs");
    (
        output.status.code().expect("an exit code"),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// The `case.yaml` of the one case under `repo`.
fn case_yaml(repo: &Path) -> String {
    let cases: Vec<PathBuf> = fs::read_dir(repo.join(".takt/cases"))
        .expect("a cases directory")
        .map(|entry| entry.expect("an entry").path())
        .collect();
    assert_eq!(cases.len(), 1, "{cases:?}");
    fs::read_to_string(cases[0].join("case.yaml")).expect("case.yaml")
}

fn protocol_id(repo: &Path) -> String {
    let cases: Vec<PathBuf> = fs::read_dir(repo.join(".takt/cases"))
        .expect("a cases directory")
        .map(|entry| entry.expect("an entry").path())
        .collect();
    let yaml = fs::read_to_string(cases[0].join("protocol.yaml")).expect("protocol.yaml");
    let doc: serde_yaml_ng::Value = serde_yaml_ng::from_str(&yaml).expect("protocol yaml");
    format!(
        "{}/{}",
        doc["protocol"]["id"].as_str().expect("an id"),
        doc["protocol"]["revision"].as_u64().expect("a revision")
    )
}

const ELEVATED: &str =
    "protocol: software.change/1\nprotocols:\n  software.change:\n    risk: elevated\n";

#[test]
fn the_run_file_sets_an_input() {
    let (dir, config) = repository("sets", &[], ELEVATED);
    let repo = dir.join("repo");
    let (code, out, err) = run(&repo, &config, &[]);
    assert!(code != 1, "opened: stdout {out} stderr {err}");
    let yaml = case_yaml(&repo);
    assert!(yaml.contains("risk: elevated"), "{yaml}");
    assert!(yaml.contains("risk: run-file"), "{yaml}");
    assert!(
        out.contains("settings: protocol software.change/1 (run-file); risk=elevated (run-file)"),
        "{out}"
    );
}

#[test]
fn an_unknown_value_is_refused_at_its_line() {
    let (dir, config) = repository(
        "refused",
        &[],
        "protocol: software.change/1\nprotocols:\n  software.change:\n    risk: extreme\n",
    );
    let repo = dir.join("repo");
    let (code, out, err) = run(&repo, &config, &[]);
    assert_eq!(code, 1, "{out} {err}");
    assert!(err.contains("run.yaml is refused at line 4:"), "{err}");
    assert!(
        err.contains("trivial, standard, elevated, critical"),
        "{err}"
    );
    assert!(!repo.join(".takt/cases").exists(), "no case opened");
}

#[test]
fn a_tag_overrides_the_run_file() {
    let (dir, config) = repository("tag", &["input:risk=trivial"], ELEVATED);
    let repo = dir.join("repo");
    let (code, out, err) = run(&repo, &config, &[]);
    assert!(code != 1, "opened: stdout {out} stderr {err}");
    let yaml = case_yaml(&repo);
    assert!(yaml.contains("risk: trivial"), "{yaml}");
    assert!(yaml.contains("risk: tag"), "{yaml}");
    assert!(out.contains("risk=trivial (tag)"), "{out}");
}

#[test]
fn the_flag_overrides_the_tag() {
    let (dir, config) = repository("flag", &["input:risk=trivial"], ELEVATED);
    let repo = dir.join("repo");
    let (code, out, err) = run(&repo, &config, &["--input", "risk=critical"]);
    assert!(code != 1, "opened: stdout {out} stderr {err}");
    let yaml = case_yaml(&repo);
    assert!(yaml.contains("risk: critical"), "{yaml}");
    assert!(yaml.contains("risk: flag"), "{yaml}");
}

#[test]
fn the_protocol_key_chooses_the_protocol() {
    let (dir, config) = repository("key", &[], "protocol: software.change/1\n");
    let repo = dir.join("repo");
    let (code, out, err) = run(&repo, &config, &[]);
    assert!(code != 1, "opened: stdout {out} stderr {err}");
    assert_eq!(protocol_id(&repo), "software.change/1");
    assert!(
        out.contains("settings: protocol software.change/1 (run-file)"),
        "{out}"
    );
}

#[test]
fn a_protocol_tag_overrides_the_key() {
    let (dir, config) = repository(
        "protocol-tag",
        &["protocol:story.delivery/1"],
        "protocol: software.change/1\n",
    );
    let repo = dir.join("repo");
    let (code, out, err) = run(&repo, &config, &[]);
    assert!(code != 1, "opened: stdout {out} stderr {err}");
    assert_eq!(protocol_id(&repo), "story.delivery/1");
    assert!(
        out.contains("settings: protocol story.delivery/1 (tag)"),
        "{out}"
    );
    assert!(case_yaml(&repo).contains("protocol_source: tag"));
}

/// A tag the protocol does not accept refuses the story before its case opens, naming the tag.
#[test]
fn a_tag_that_the_protocol_refuses_opens_no_case() {
    let (dir, config) = repository("bad-tag", &["input:risk=extreme"], ELEVATED);
    let repo = dir.join("repo");
    let (code, out, err) = run(&repo, &config, &[]);
    assert_eq!(code, 1, "{out} {err}");
    assert!(err.contains("input:risk=extreme"), "{err}");
    assert!(
        err.contains("trivial, standard, elevated, critical"),
        "{err}"
    );
    assert!(!repo.join(".takt/cases").exists(), "no case opened");
}
