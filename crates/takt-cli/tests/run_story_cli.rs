//! `takt run --story <id>` through the binary, over a copy of a fixture planning store: the
//! named story is the unit instead of the frontier's next ready one, taken as a frontier take
//! takes it; a story the store does not list or the frontier would not take is refused before
//! any case opens; `--story` does not combine with `--intent` or `--case`; a story with an open
//! case resumes that case; and without `--story` the frontier's next ready story is taken as
//! before (story:run-names-its-story).

#[path = "../../takt/tests/support/own_home.rs"]
mod own_home;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use takt::Frontier as _;

/// The fixture stores of the library crate, one directory each.
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../takt/tests/fixtures");

fn git(tree: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(args)
        .current_dir(tree)
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?}");
}

fn git_out(tree: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(tree)
        .output()
        .expect("git runs");
    assert!(output.status.success(), "git {args:?}");
    String::from_utf8_lossy(&output.stdout).into_owned()
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

fn scratch(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("run-story-cli-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(&dir).expect("scratch dir");
    Scratch(dir)
}

const STORIES: &str = ".engineering/planning/story";

/// A committed copy of the `one-ready` fixture store in a scratch tree of its own, with two
/// more stories made from its ready one: `story:second-ready`, ready too and after
/// `story:ready-one` in id order, so the frontier's next is not it; and `story:worked`, listed
/// `active` with no case.
fn repository(name: &str) -> Scratch {
    let dir = scratch(name);
    copy_dir(&Path::new(FIXTURES).join("one-ready"), &dir);
    let ready = fs::read_to_string(dir.join(STORIES).join("ready-one.md")).expect("ready-one.md");
    let second = ready
        .replace("ready-one", "second-ready")
        .replace("Make the check pass", "Make the second check pass")
        .replace("The one ready story", "A second ready story");
    fs::write(dir.join(STORIES).join("second-ready.md"), second).expect("a story");
    let worked = ready
        .replace("ready-one", "worked")
        .replace("Make the check pass", "A story being worked")
        .replace("The one ready story", "Active, with no case");
    fs::write(dir.join(STORIES).join("worked.md"), worked).expect("a story");
    fs::write(dir.join("check.txt"), "broken\n").expect("a file");
    git(&dir, &["init", "-q", "-b", "main"]);
    // Moved `active` by the store's own verb, which records the transition that says so.
    match takt::frontier::Aep::default()
        .take(&dir, "story:worked")
        .expect("the store moves story:worked")
    {
        takt::frontier::Take::Taken => {}
        takt::frontier::Take::Refused(refusal) => panic!("story:worked not moved: {refusal}"),
    }
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "start"]);
    dir
}

/// Runs the built `takt run` on `repo` with no harness, one iteration and a passing test
/// command, plus `extra`: the exit code, stdout and stderr.
fn run(repo: &Path, extra: &[&str]) -> (i32, String, String) {
    // These tests run the first delivery protocol; the second one declares inputs they do not set.
    let pinned: &[&str] = if extra.contains(&"--protocol") {
        &[]
    } else {
        &["--protocol", "story.delivery/1"]
    };
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
        ])
        .args(pinned)
        .args(extra)
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

/// The case directories under `repo`, sorted; empty when there is no cases directory.
fn cases(repo: &Path) -> Vec<PathBuf> {
    let dir = repo.join(".takt/cases");
    if !dir.exists() {
        return Vec::new();
    }
    let mut cases: Vec<PathBuf> = fs::read_dir(&dir)
        .expect("a readable cases directory")
        .map(|e| e.expect("an entry").path())
        .collect();
    cases.sort();
    cases
}

/// Acceptance 1: `--story` opens a case on `story.delivery/1` for the story it names, not the
/// frontier's next ready one, and the store records it taken, the take committed on the case's
/// branch as a frontier take commits it.
#[test]
fn a_named_story_is_taken_instead_of_the_frontiers_next() {
    let repo = repository("taken");
    let (code, out, err) = run(&repo, &["--story", "story:second-ready"]);
    assert_eq!(
        code, 7,
        "the gate with no gates document stops the run: stdout {out} stderr {err}"
    );
    assert!(
        out.starts_with("frontier: took story:second-ready: Make the second check pass\n"),
        "{out}"
    );
    let cases = cases(&repo);
    assert_eq!(cases.len(), 1, "{cases:?}");
    let case = &cases[0];
    let id = case.file_name().expect("a name").to_string_lossy();
    assert!(id.starts_with("story-second-ready-"), "{id}");
    let intent = fs::read_to_string(case.join("intent.md")).expect("intent.md");
    assert!(
        intent.starts_with("Make the second check pass\n\n# Story: second-ready\n"),
        "{intent}"
    );
    let protocol = fs::read_to_string(case.join("protocol.yaml")).expect("protocol.yaml");
    assert!(
        protocol.contains("\n  id: story.delivery\n  revision: 1\n"),
        "the case opens on story.delivery/1: {protocol}"
    );
    assert!(
        git_out(&repo, &["log", "--format=%s"])
            .lines()
            .any(|subject| subject == "takt: take story:second-ready"),
        "the take is committed on the case's branch"
    );
    assert_eq!(status(&repo, "story:second-ready"), "active");
    assert_eq!(status(&repo, "story:ready-one"), "proposed");
}

/// Acceptance 4: without `--story` the run takes the frontier's next ready story, the first in
/// id order, as before.
#[test]
fn without_story_the_frontiers_next_is_taken() {
    let repo = repository("frontier");
    let (code, out, err) = run(&repo, &[]);
    assert_eq!(code, 7, "stdout {out} stderr {err}");
    assert!(
        out.starts_with("frontier: took story:ready-one: Make the check pass\n"),
        "{out}"
    );
    assert_eq!(status(&repo, "story:ready-one"), "active");
    assert_eq!(status(&repo, "story:second-ready"), "proposed");
}

/// Acceptance 2: a story the store does not list, or one the frontier would not take (not a
/// story, not `proposed`, blocked, or a dependency not done), is refused before any case opens,
/// exit 1, the refusal naming the story and why; the store is left as it was.
#[test]
fn a_story_not_ready_is_refused_before_any_case_opens() {
    for (story, why) in [
        ("story:no-such", "the store lists no story:no-such"),
        (
            "decision-blocker:open-question",
            "is a decision-blocker, not a story",
        ),
        ("story:draft-dep", "is draft, not proposed"),
        (
            "story:blocked-one",
            "blocked by decision-blocker:open-question",
        ),
        ("story:waits-one", "depends on story:draft-dep, draft"),
        (
            "story:worked",
            "is active, and no open case of it is under the tree to resume",
        ),
    ] {
        let repo = repository(&story.replace(':', "-"));
        let (code, out, err) = run(&repo, &["--story", story]);
        assert_eq!(code, 1, "{story}: stdout {out} stderr {err}");
        assert!(
            err.contains(&format!("--story {story}")) && err.contains(why),
            "{story}: the refusal names the story and why ({why}): {err}"
        );
        assert!(err.contains("no case was opened"), "{story}: {err}");
        assert!(!out.contains("frontier: took"), "{story}: {out}");
        assert!(cases(&repo).is_empty(), "{story}: no case is opened");
        assert_eq!(status(&repo, "story:ready-one"), "proposed", "{story}");
        assert_eq!(status(&repo, "story:second-ready"), "proposed", "{story}");
        assert_eq!(status(&repo, "story:draft-dep"), "draft", "{story}");
        assert_eq!(status(&repo, "story:worked"), "active", "{story}");
    }
}

/// Acceptance 3: `--story` with `--intent` or `--case` is refused by the command line, exit 2,
/// with nothing read or written.
#[test]
fn story_with_intent_or_case_is_refused_by_the_command_line() {
    let repo = repository("conflicts");
    for other in [["--intent", "do something"], ["--case", "some-case"]] {
        let (code, out, err) = run(
            &repo,
            &["--story", "story:second-ready", other[0], other[1]],
        );
        assert_eq!(code, 2, "{other:?}: stdout {out} stderr {err}");
        assert!(
            err.contains("--story") && err.contains(other[0]) && err.contains("cannot be used"),
            "{other:?}: {err}"
        );
        assert_eq!(out, "", "{other:?}");
        assert!(cases(&repo).is_empty(), "{other:?}");
        assert_eq!(status(&repo, "story:second-ready"), "proposed", "{other:?}");
    }
}

/// A named story whose case is open is resumed, as a run without `--story` resumes the open
/// case of a story being worked: no second case is opened and nothing is taken again.
#[test]
fn a_named_story_with_an_open_case_resumes_it() {
    let repo = repository("resume");
    let (code, out, err) = run(&repo, &["--story", "story:second-ready"]);
    assert_eq!(code, 7, "stdout {out} stderr {err}");
    let opened = cases(&repo);
    assert_eq!(opened.len(), 1, "{opened:?}");
    let id = opened[0]
        .file_name()
        .expect("a name")
        .to_string_lossy()
        .into_owned();
    let (code, out, err) = run(&repo, &["--story", "story:second-ready"]);
    assert_eq!(code, 7, "stdout {out} stderr {err}");
    assert!(
        out.starts_with(&format!(
            "frontier: resumed story:second-ready: Make the second check pass (case {id})\n"
        )),
        "{out}"
    );
    assert!(!out.contains("frontier: took"), "{out}");
    assert_eq!(cases(&repo), opened, "no second case is opened");
}

/// A named story is the unit even while another story's case is open, which a run without
/// `--story` would resume: the named one is taken, read from the open case's base, and the
/// other case is left as it was.
#[test]
fn a_named_story_is_taken_while_another_case_is_open() {
    let repo = repository("other-open");
    let (code, out, err) = run(&repo, &[]);
    assert_eq!(code, 7, "stdout {out} stderr {err}");
    assert!(out.starts_with("frontier: took story:ready-one"), "{out}");
    let (code, out, err) = run(&repo, &["--story", "story:second-ready"]);
    assert_eq!(code, 7, "stdout {out} stderr {err}");
    assert!(
        out.starts_with("frontier: took story:second-ready: Make the second check pass\n"),
        "{out}"
    );
    let names: Vec<String> = cases(&repo)
        .iter()
        .map(|case| {
            case.file_name()
                .expect("a name")
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert_eq!(names.len(), 2, "{names:?}");
    assert!(names[0].starts_with("story-ready-one-"), "{names:?}");
    assert!(names[1].starts_with("story-second-ready-"), "{names:?}");
}

/// A `self.improvement/1` turn opens on its finding, never on a named story: `--story` with it
/// is refused before anything is read for the turn or written, exit 1.
#[test]
fn story_with_an_improvement_turn_is_refused() {
    let repo = repository("turn");
    let (code, out, err) = run(
        &repo,
        &[
            "--story",
            "story:second-ready",
            "--protocol",
            "self.improvement/1",
        ],
    );
    assert_eq!(code, 1, "stdout {out} stderr {err}");
    assert!(
        err.contains("--story story:second-ready: a self.improvement/1 turn opens on its finding"),
        "{err}"
    );
    assert!(cases(&repo).is_empty());
    assert_eq!(status(&repo, "story:second-ready"), "proposed");
}

// story:run-configuration-names-the-story-and-token-variable: the run configuration file's
// `story` names the story as `--story` does.

/// A run configuration file holding `text`, beside the scratch tree `repo` and out of it, so
/// the tree stays clean; named after it.
fn config_beside(repo: &Path, text: &str) -> PathBuf {
    let mut name = repo.file_name().expect("a name").to_owned();
    name.push(".config.toml");
    let path = repo.with_file_name(name);
    fs::write(&path, text).expect("a run configuration file");
    path
}

/// Acceptance 1: `story = "<id>"` in `--config`'s file takes that story as `--story <id>` does,
/// not the frontier's next ready one.
#[test]
fn the_configurations_story_is_taken_as_the_flags_is() {
    let repo = repository("config-taken");
    let config = config_beside(&repo, "story = \"story:second-ready\"\n");
    let (code, out, err) = run(&repo, &["--config", config.to_str().expect("utf-8")]);
    let _ = fs::remove_file(&config);
    assert_eq!(code, 7, "stdout {out} stderr {err}");
    assert!(
        out.starts_with("frontier: took story:second-ready: Make the second check pass\n"),
        "{out}"
    );
    let cases = cases(&repo);
    assert_eq!(cases.len(), 1, "{cases:?}");
    let id = cases[0].file_name().expect("a name").to_string_lossy();
    assert!(id.starts_with("story-second-ready-"), "{id}");
    assert_eq!(status(&repo, "story:second-ready"), "active");
    assert_eq!(status(&repo, "story:ready-one"), "proposed");
}

/// Acceptance 1: a story the file names that is not ready is refused as the flag's is, before
/// any case opens, exit 1, the store left as it was; the refusal names the file's key and the
/// file, not `--story`.
#[test]
fn the_configurations_story_not_ready_is_refused_as_the_flags_is() {
    let repo = repository("config-not-ready");
    let config = config_beside(&repo, "story = \"story:draft-dep\"\n");
    let (code, out, err) = run(&repo, &["--config", config.to_str().expect("utf-8")]);
    let _ = fs::remove_file(&config);
    assert_eq!(code, 1, "stdout {out} stderr {err}");
    assert!(
        err.contains(&format!("story story:draft-dep from {}", config.display()))
            && err.contains("is draft, not proposed"),
        "{err}"
    );
    assert!(!err.contains("--story"), "{err}");
    assert!(err.contains("no case was opened"), "{err}");
    assert!(cases(&repo).is_empty(), "no case is opened");
    assert_eq!(status(&repo, "story:draft-dep"), "draft");
    assert_eq!(status(&repo, "story:ready-one"), "proposed");
}

/// Acceptance 1: `--story` on the command line wins over the file's `story`; so does
/// `--intent`, which names the work too: the file's story is not taken.
#[test]
fn a_story_or_intent_flag_wins_over_the_configurations_story() {
    let repo = repository("config-flag-wins");
    let config = config_beside(&repo, "story = \"story:draft-dep\"\n");
    let path = config.to_str().expect("utf-8");
    let (code, out, err) = run(&repo, &["--config", path, "--story", "story:second-ready"]);
    assert_eq!(code, 7, "stdout {out} stderr {err}");
    assert!(
        out.starts_with("frontier: took story:second-ready"),
        "{out}"
    );

    let repo = repository("config-intent-wins");
    let (code, out, err) = run(&repo, &["--config", path, "--intent", "probe"]);
    let _ = fs::remove_file(&config);
    assert_ne!(code, 1, "stdout {out} stderr {err}");
    assert!(!out.contains("frontier: took"), "{out}");
    assert!(!err.contains("story:draft-dep"), "{err}");
    assert_eq!(cases(&repo).len(), 1);
    assert_eq!(status(&repo, "story:draft-dep"), "draft");
}

/// A `self.improvement/1` turn opens no story: the file's `story` is left out for it, where
/// `--story` is refused; the turn is refused only for what it lacks itself (its finding).
#[test]
fn an_improvement_turn_leaves_the_configurations_story_out() {
    let repo = repository("config-turn");
    let config = config_beside(&repo, "story = \"story:second-ready\"\n");
    let (code, out, err) = run(
        &repo,
        &[
            "--config",
            config.to_str().expect("utf-8"),
            "--protocol",
            "self.improvement/1",
        ],
    );
    let _ = fs::remove_file(&config);
    assert_eq!(code, 1, "stdout {out} stderr {err}");
    assert!(!err.contains("never on a named story"), "{err}");
    assert!(!err.contains("story:second-ready"), "{err}");
    assert!(err.contains("finding"), "the turn's own refusal: {err}");
    assert!(!out.contains("frontier: took"), "{out}");
    assert_eq!(status(&repo, "story:second-ready"), "proposed");
}
