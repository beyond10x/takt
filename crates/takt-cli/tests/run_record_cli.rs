//! story:run-record-and-story-evidence through the binary: `takt run` on a story of a copy of a
//! fixture planning store, whose case terminates `accepted` once its tests pass, leaves
//! `events.jsonl` in the case directory, records the tests validator's pass record on the story
//! as `test_result` evidence from takt at the record's own instant, and moves the story to
//! `implemented`, both on the case's branch.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The fixture stores of the library crate, one directory each.
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../takt/tests/fixtures");

/// The story the fixture store holds ready.
const STORY: &str = "story:ready-one";

/// One validator-owned action, `tests.run`; accepted once the tests pass.
const TESTS_ONLY: &str = "format: protocol/1

protocol:
  id: tests.only
  revision: 1
  description: >-
    Run the tests against the implementation at its current revision; accepted once they pass.

artifacts:
  implementation:
    description: a revision of the code

evidence_kinds:
  test_result:
    description: the result of running tests against an implementation revision

claims:
  tests.pass:
    description: the tests pass for the current implementation revision
    true_when:
      evidence:
        kind: test_result
        result: pass
        subject: implementation

obligations:
  tests.required:
    description: the tests pass for the current implementation revision
    discharged_when:
      claim: tests.pass

actions:
  tests.run:
    description: run the tests against an implementation revision
    effect: read
    may_produce:
      - evidence: test_result

outcomes:
  accepted:
    description: the tests pass for the current implementation revision
    requires:
      claim: tests.pass
";

/// Runs `git <args>` in `tree`; its trimmed standard output.
fn git(tree: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(args)
        .current_dir(tree)
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("utf-8")
        .trim()
        .to_owned()
}

/// A scratch tree under this crate's build directory, removed on drop; a removal that fails
/// fails the case, unless the case is already failing.
struct Scratch(PathBuf);

impl std::ops::Deref for Scratch {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            let message = format!("remove the scratch tree {}: {error}", self.0.display());
            if std::thread::panicking() {
                eprintln!("{message}");
            } else {
                panic!("{message}");
            }
        }
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

/// A scratch repository on `main` holding a committed copy of the `one-ready` store and the
/// protocol above as `tests-only.yaml`.
fn repository(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("run-record-cli-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    copy_dir(&Path::new(FIXTURES).join("one-ready"), &dir);
    fs::write(dir.join("tests-only.yaml"), TESTS_ONLY).expect("the protocol");
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "start"]);
    Scratch(dir)
}

/// Runs the built `takt` binary with `args`: the exit code, stdout and stderr.
fn takt(args: &[&str]) -> (i32, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args(args)
        .output()
        .expect("takt runs");
    (
        output.status.code().expect("an exit code"),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// The status the store in `repo` lists `id` at.
fn status(repo: &Path, id: &str) -> String {
    takt::frontier::Aep::default()
        .list(repo)
        .expect("the store lists")
        .into_iter()
        .find(|a| a.id == id)
        .map(|a| a.status)
        .unwrap_or_else(|| panic!("{id} is listed"))
}

/// `aep plan artifact history <id> --format json` in `repo`, as `aep` prints it: one key per
/// line, which the checks below read as text (this crate has no JSON reader of its own).
fn history(repo: &Path, id: &str) -> String {
    let output = Command::new("aep")
        .args(["plan", "artifact", "history", id, "--format", "json"])
        .current_dir(repo)
        .output()
        .expect("aep runs");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("utf-8")
}

/// The string value of `key` in a JSON document printed one key per line.
fn value_of(json: &str, key: &str) -> String {
    let prefix = format!("\"{key}\": \"");
    json.lines()
        .find_map(|line| line.trim().strip_prefix(&prefix))
        .and_then(|rest| rest.split('"').next())
        .unwrap_or_else(|| panic!("no {key} in {json}"))
        .to_owned()
}

/// The one case directory under the repository's cases directory.
fn the_case(repo: &Path) -> PathBuf {
    let cases: Vec<PathBuf> = fs::read_dir(repo.join(".engineering/cases"))
        .expect("the cases directory")
        .map(|entry| entry.expect("an entry").path())
        .collect();
    assert_eq!(cases.len(), 1, "{cases:?}");
    cases[0].clone()
}

#[test]
fn takt_run_on_a_story_whose_case_is_accepted_records_the_test_result_and_moves_the_story() {
    let repo = repository("accepted");
    let protocol = repo.join("tests-only.yaml");
    let (code, out, err) = takt(&[
        "run",
        "--repo",
        repo.to_str().expect("utf-8"),
        "--harness",
        "none",
        "--forge",
        "none",
        "--iterations",
        "4",
        "--test-cmd",
        "true",
        "--protocol",
        protocol.to_str().expect("utf-8"),
    ]);
    assert_eq!(code, 0, "stdout {out} stderr {err}");
    assert!(out.contains("stopped: Completed (accepted)"), "{out}");

    // The store: one test_result from takt at the record's own instant, and the move.
    let dir = the_case(&repo);
    assert!(dir.join("events.jsonl").is_file(), "{out}");
    let lines = takt::events::Events::new(&dir)
        .read()
        .expect("one object per line");
    let evidence = lines
        .iter()
        .find(|line| line["event"] == "evidence" && line["kind"] == "test_result")
        .expect("the tests validator's record");
    let path = evidence["path"].as_str().expect("a path");
    let observed_at = value_of(
        &fs::read_to_string(repo.join(path)).expect("the record"),
        "observed_at",
    );
    let recorded = history(&repo, STORY);
    assert_eq!(
        recorded.matches("\"kind\": \"test_result\"").count(),
        1,
        "{recorded}\n{out}"
    );
    assert!(
        recorded.contains(&format!("\"at\": \"{observed_at}\"")),
        "{recorded}"
    );
    assert!(recorded.contains("\"source\": \"takt\""), "{recorded}");
    assert!(
        recorded.contains(&format!("\"reference\": \"{path}\"")),
        "{recorded}"
    );
    assert_eq!(status(&repo, STORY), "implemented", "{out}");
    assert!(
        out.contains(&format!("store: {STORY} moved to implemented")),
        "{out}"
    );
    assert_eq!(
        lines.last().expect("a line")["stop"],
        "Completed (accepted)"
    );

    // Both on the case's branch, above the take; the base untouched.
    let branch = git(&repo, &["branch", "--show-current"]);
    assert!(branch.starts_with("takt/"), "{branch}");
    assert_eq!(
        git(&repo, &["log", "--format=%s", "main..HEAD"]),
        format!("takt: implemented {STORY}\ntakt: test_result {STORY}\ntakt: take {STORY}")
    );
    assert!(
        git(
            &repo,
            &["show", "main:.engineering/planning/story/ready-one.md"]
        )
        .contains("status: proposed")
    );
}
