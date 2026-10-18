//! The frontier end to end over a fixture planning store, read and written only through the
//! `aep` command line: the adapter lists the store, the ready rule picks the one `proposed`
//! story nothing blocks and nothing unmet precedes, the store records it taken (`proposed` →
//! `active`), and a run opens a case on it whose `intent.md` is the story's title followed by
//! its body. On a store with nothing ready the frontier gives no unit and moves nothing.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use takt::frontier::{Aep, Frontier, Unit};
use takt::harness::{Harness, StepContext, StepReport};
use takt::validator::Validator;
use takt::{Run, RunOptions};

/// The fixture stores, one directory each, relative to this crate.
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");

fn git(tree: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(args)
        .current_dir(tree)
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?}");
}

/// A scratch git repository under this crate's build directory. Dropping it removes the tree; a
/// removal that fails fails the case, unless the case is already failing.
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

/// An empty scratch repository with one commit.
fn scratch(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("frontier-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(&dir).expect("scratch dir");
    Scratch(dir)
}

/// A scratch repository holding a copy of the fixture store `fixture`, committed; `name` keeps
/// the cases of this process apart.
fn repository(fixture: &str, name: &str) -> Scratch {
    let dir = scratch(name);
    copy_dir(&Path::new(FIXTURES).join(fixture), &dir);
    fs::write(dir.join("check.txt"), "broken\n").expect("a file");
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "start"]);
    dir
}

/// A harness that is never asked: these cases open a case and take no judgement step.
struct Never;

impl Harness for Never {
    fn name(&self) -> &str {
        "never"
    }

    fn step(&self, context: &StepContext<'_>) -> Result<StepReport, anyhow::Error> {
        panic!(
            "a judgement step was asked for: {}",
            context.offered.join(", ")
        );
    }
}

/// The status the store lists `id` at.
fn status(repo: &Path, id: &str) -> String {
    Aep::default()
        .list(repo)
        .expect("the store lists")
        .into_iter()
        .find(|a| a.id == id)
        .map(|a| a.status)
        .unwrap_or_else(|| panic!("{id} is listed"))
}

fn options() -> RunOptions {
    RunOptions {
        protocol: "software.change/1".to_owned(),
        intent: String::new(),
        inputs: vec![
            ("risk".to_owned(), "trivial".to_owned()),
            ("change_kind".to_owned(), "feature".to_owned()),
            ("affects_security_boundary".to_owned(), "false".to_owned()),
        ],
        iterations: Some(1),
        hours: None,
        tokens: None,
        case_id: None,
        gate: None,
    }
}

#[test]
fn the_one_ready_story_is_taken_once_and_becomes_the_case() {
    let repo = repository("one-ready", "taken");
    assert_eq!(status(&repo, "story:ready-one"), "proposed");
    let frontier = Aep::default();
    let mut out = Vec::new();
    let story = frontier
        .next(&repo, &mut out)
        .expect("the frontier answers")
        .expect("one story is ready");
    assert_eq!(story.id, "story:ready-one");
    assert_eq!(story.title, "Make the check pass");
    assert!(
        story.body.starts_with("# Story: ready-one"),
        "{}",
        story.body
    );
    assert_eq!(status(&repo, "story:ready-one"), "active");
    // The blocked story and the one with an unmet dependency were skipped, not moved.
    assert_eq!(status(&repo, "story:blocked-one"), "proposed");
    assert_eq!(status(&repo, "story:waits-one"), "proposed");
    assert_eq!(status(&repo, "story:draft-dep"), "draft");

    let validators: Vec<Box<dyn Validator>> = vec![];
    let run = Run::open_unit(
        &repo,
        options(),
        Unit::Story(story.clone()),
        &Never,
        &validators,
    )
    .expect("the case opens");
    assert!(
        run.case.id.starts_with("story-ready-one-"),
        "the case id is derived from the story id and the instant, not {}",
        run.case.id
    );
    let intent = fs::read_to_string(run.case.dir.join("intent.md")).expect("intent.md");
    assert_eq!(intent, format!("{}\n\n{}", story.title, story.body));
    assert_eq!(run.unit, Unit::Story(story));

    // Taken once: the story is active now, so a second ask finds nothing ready.
    let again = frontier
        .next(&repo, &mut out)
        .expect("the frontier answers");
    assert_eq!(again, None, "{}", String::from_utf8_lossy(&out));
    assert_eq!(status(&repo, "story:ready-one"), "active");
}

#[test]
fn a_store_with_nothing_ready_gives_no_unit_and_moves_nothing() {
    let repo = repository("none-ready", "nothing");
    let mut out = Vec::new();
    let unit = Aep::default()
        .next(&repo, &mut out)
        .expect("the frontier answers");
    assert_eq!(unit, None, "{}", String::from_utf8_lossy(&out));
    assert!(!repo.join(".engineering/cases").exists());
    assert_eq!(status(&repo, "story:blocked-one"), "proposed");
    assert_eq!(status(&repo, "story:waits-one"), "proposed");
    assert_eq!(status(&repo, "story:draft-dep"), "draft");
}

#[test]
fn a_repository_without_a_planning_store_is_refused_not_searched_upward() {
    // The scratch tree lies under this repository's own build directory, below a real planning
    // store; `aep` would find that store by walking up, and the adapter must not let it.
    let dir = scratch("no-store");
    fs::write(dir.join("README.md"), "no store\n").expect("a file");
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "start"]);
    let mut out = Vec::new();
    let error = Aep::default()
        .next(&dir, &mut out)
        .expect_err("a repository without a store is refused")
        .to_string();
    assert!(error.contains("no planning store"), "{error}");
}

#[test]
fn an_intent_is_the_unit_when_given() {
    let repo = repository("none-ready", "intent");
    let validators: Vec<Box<dyn Validator>> = vec![];
    let mut options = options();
    options.case_id = Some("intent-unit".to_owned());
    let run = Run::open_unit(
        &repo,
        options,
        Unit::Intent("make the check pass".to_owned()),
        &Never,
        &validators,
    )
    .expect("the case opens");
    assert_eq!(run.case.id, "intent-unit");
    let intent = fs::read_to_string(run.case.dir.join("intent.md")).expect("intent.md");
    assert_eq!(intent, "make the check pass");
    assert_eq!(run.unit, Unit::Intent("make the check pass".to_owned()));
}
