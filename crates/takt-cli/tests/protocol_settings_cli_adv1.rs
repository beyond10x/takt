//! Adversarial cases for story:the-run-file-sets-protocol-settings through the binary: a value
//! outside the declared set from any layer is refused, a resumed case's run file is still checked,
//! and the `opened` line carries each value's source.

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
        "protocol-settings-cli-adv1-{}-{name}",
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

const ELEVATED: &str =
    "protocol: software.change/1\nprotocols:\n  software.change:\n    risk: elevated\n";

/// The case directory of the one case under `repo`.
fn case_dir(repo: &Path) -> PathBuf {
    let cases: Vec<PathBuf> = fs::read_dir(repo.join(".takt/cases"))
        .expect("a cases directory")
        .map(|entry| entry.expect("an entry").path())
        .collect();
    assert_eq!(cases.len(), 1, "{cases:?}");
    cases[0].clone()
}

/// Acceptance 6 and the outcome: "Every value is recorded with its source on `opened` and in
/// `case.yaml`". The `opened` line of the run record carries the protocol's source and each
/// input's.
#[test]
fn the_opened_line_records_each_source() {
    let (dir, config) = repository(
        "opened",
        &["protocol:software.change/1", "input:risk=trivial"],
        "protocols:\n  software.change:\n    change_kind: bugfix\n",
    );
    let repo = dir.join("repo");
    let (code, out, err) = run(&repo, &config, &[]);
    assert!(code != 1, "opened: stdout {out} stderr {err}");
    let events = fs::read_to_string(case_dir(&repo).join("events.jsonl")).expect("events");
    let opened: serde_json::Value = events
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("a line"))
        .find(|line| line["event"] == "opened")
        .expect("an opened line");
    let text = opened.to_string();
    assert!(
        text.contains("\"tag\"") && text.contains("\"run-file\"") && text.contains("\"default\""),
        "the opened line names no source: {opened}"
    );
}

/// `--input` with a value outside the declared set is refused before a case opens, naming the
/// declared values, as the run file's and a tag's are.
#[test]
fn a_flag_value_outside_the_declared_set_is_refused() {
    let (dir, config) = repository("flag-value", &[], ELEVATED);
    let repo = dir.join("repo");
    let (code, out, err) = run(&repo, &config, &["--input", "risk=extreme"]);
    assert_eq!(code, 1, "{out} {err}");
    assert!(
        err.contains("extreme")
            && ["trivial", "standard", "elevated", "critical"]
                .iter()
                .all(|value| err.contains(value)),
        "{err}"
    );
    assert!(!repo.join(".takt/cases").exists(), "no case opened");
}

/// A resumed case keeps its inputs, but a run file whose `protocols` section names a value the
/// protocol does not declare is still a refused run file: the story refuses an unknown value at
/// its line "as RunConfig refuses an unknown key", which it does on every run.
#[test]
fn a_resumed_case_refuses_a_bad_run_file() {
    let (dir, config) = repository("resumed", &[], ELEVATED);
    let repo = dir.join("repo");
    let (code, out, err) = run(&repo, &config, &[]);
    assert!(code != 1, "opened: stdout {out} stderr {err}");
    let id = case_dir(&repo)
        .file_name()
        .expect("a name")
        .to_string_lossy()
        .into_owned();
    let bad = dir.join("bad.yaml");
    fs::write(
        &bad,
        "protocols:\n  software.change:\n    risk: extreme\n    no_such_input: x\n",
    )
    .expect("a run file");
    let (code, out, err) = run(&repo, &bad, &["--case", &id]);
    assert_eq!(
        code, 1,
        "the bad run file is accepted on resume: {out} {err}"
    );
    assert!(err.contains("bad.yaml is refused at line"), "{err}");
}

/// An intent's run ignores the `protocol` key by design, but a key that names no protocol takt
/// knows is a refused run file whatever the unit.
#[test]
fn an_intent_run_refuses_an_unknown_protocol_key() {
    let (dir, config) = repository("intent-key", &[], "protocol: no.such/1\n");
    let repo = dir.join("repo");
    let (code, out, err) = run(&repo, &config, &["--intent", "Fix the check"]);
    assert_eq!(code, 1, "an unknown protocol key is accepted: {out} {err}");
    assert!(err.contains("run.yaml is refused at line 1"), "{err}");
}
