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
    let validators: Vec<Box<dyn Validator>> = vec![];
    let mut out = Vec::new();
    let run = Run::open_next(&repo, options(), &frontier, &Never, &validators, &mut out)
        .expect("the frontier answers")
        .expect("one story is ready");
    let Unit::Story(story) = run.unit.clone() else {
        panic!("the unit is a story, not {:?}", run.unit);
    };
    assert_eq!(story.id, "story:ready-one");
    assert_eq!(story.title, "Make the check pass");
    assert!(
        story.body.starts_with("# Story: ready-one"),
        "{}",
        story.body
    );
    assert_eq!(status(&repo, "story:ready-one"), "active");
    assert_eq!(
        String::from_utf8_lossy(&out),
        "frontier: took story:ready-one: Make the check pass\n"
    );
    // The blocked story and the one with an unmet dependency were skipped, not moved.
    assert_eq!(status(&repo, "story:blocked-one"), "proposed");
    assert_eq!(status(&repo, "story:waits-one"), "proposed");
    assert_eq!(status(&repo, "story:draft-dep"), "draft");

    assert!(
        run.case.id.starts_with("story-ready-one-"),
        "the case id is derived from the story id and the instant, not {}",
        run.case.id
    );
    let intent = fs::read_to_string(run.case.dir.join("intent.md")).expect("intent.md");
    assert_eq!(intent, format!("{}\n\n{}", story.title, story.body));

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

/// Runs `aep plan artifact <args>` in `repo`, for a fixture copy the case shapes itself.
fn aep(repo: &Path, args: &[&str]) {
    let output = Command::new("aep")
        .args(["plan", "artifact"])
        .args(args)
        .current_dir(repo)
        .output()
        .expect("aep runs");
    assert!(
        output.status.success(),
        "aep plan artifact {args:?}: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Keeps a stub and its log out of the tree's uncommitted work: a case opens only on a clean
/// tree, and the stub is the test's, not work in progress.
fn ignore_stub(dir: &Path) {
    let exclude = dir.join(".git/info/exclude");
    let mut text = fs::read_to_string(&exclude).unwrap_or_default();
    for name in ["aep-stub*.sh", "calls.log"] {
        if !text.lines().any(|line| line == name) {
            text.push_str(name);
            text.push('\n');
        }
    }
    fs::write(&exclude, text).expect("the stub is ignored");
}

/// A stand-in for the `aep` command line, written as a shell script into `dir`: it lists three
/// `proposed` stories, shows any of them, refuses to move `story:a` and moves the others, and
/// appends every call to `calls.log` beside it. The store's refusal is the one shape the fixture
/// stores cannot produce, since the story lifecycle lets every `proposed` story go `active`.
fn refusing_aep(dir: &Path) -> String {
    let log = dir.join("calls.log");
    let script = r##"#!/bin/sh
printf '%s\n' "$*" >> "@LOG@"
case "$3" in
  list)
    printf '%s\n' '[{"id":"story:c","kind":"story","status":"proposed","title":"C","path":"story/c.md","relations":[],"refs":[],"blocked_by":[]},{"id":"story:a","kind":"story","status":"proposed","title":"A","path":"story/a.md","relations":[],"refs":[],"blocked_by":[]},{"id":"story:b","kind":"story","status":"proposed","title":"B","path":"story/b.md","relations":[],"refs":[],"blocked_by":[]}]'
    ;;
  show)
    printf '{"id":"%s","kind":"story","status":"proposed","title":"Title of %s","body":"# Story\\n"}\n' "$4" "$4"
    ;;
  move)
    if [ "$4" = "story:a" ]; then
      printf '%s\n' "story:a is proposed; the profile wants it to serve an objective first" >&2
      exit 1
    fi
    printf '%s moved proposed -> active (revision 2)\n' "$4"
    ;;
  *)
    printf 'unexpected: %s\n' "$*" >&2
    exit 2
    ;;
esac
"##
    .replace("@LOG@", &log.display().to_string());
    let path = dir.join("aep-stub.sh");
    fs::write(&path, script).expect("the stub is written");
    ignore_stub(dir);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("executable");
    }
    path.display().to_string()
}

/// A store that refuses to hand over the first ready story: the refusal is printed verbatim on
/// `out`, the story is skipped, and the next ready story in id order is taken. `story:c` is
/// never asked for, since `story:b` was taken.
#[test]
fn a_refused_take_is_printed_verbatim_and_the_next_ready_story_is_taken() {
    let repo = repository("one-ready", "refused");
    let frontier = Aep {
        command: refusing_aep(&repo),
    };
    let mut out = Vec::new();
    let story = frontier
        .next(&repo, &mut out)
        .expect("the frontier answers")
        .expect("story:b is ready and taken");
    assert_eq!(story.id, "story:b");
    assert_eq!(story.title, "Title of story:b");
    let out = String::from_utf8(out).expect("utf-8");
    assert_eq!(
        out,
        "frontier: story:a not taken: story:a is proposed; the profile wants it to serve an objective first\n"
    );
    let calls = fs::read_to_string(repo.join("calls.log")).expect("the stub logged its calls");
    let moves: Vec<&str> = calls
        .lines()
        .filter(|line| line.starts_with("plan artifact move "))
        .collect();
    assert_eq!(
        moves,
        [
            "plan artifact move story:a --to active --executor agent:takt",
            "plan artifact move story:b --to active --executor agent:takt",
        ],
        "{calls}"
    );
}

/// Two ready stories in the real store: the first in id order is taken first, the second on
/// the next ask, and the third ask finds nothing.
#[test]
fn two_ready_stories_are_taken_in_id_order_one_per_ask() {
    let repo = repository("one-ready", "two-ready");
    aep(
        &repo,
        &["new", "story", "aaa-first", "--title", "First by id"],
    );
    aep(
        &repo,
        &[
            "move",
            "story:aaa-first",
            "--to",
            "proposed",
            "--executor",
            "agent:fixture",
        ],
    );
    let frontier = Aep::default();
    let mut out = Vec::new();
    let first = frontier
        .next(&repo, &mut out)
        .expect("the frontier answers")
        .expect("a story is ready");
    assert_eq!(first.id, "story:aaa-first");
    assert_eq!(first.title, "First by id");
    assert_eq!(status(&repo, "story:aaa-first"), "active");
    assert_eq!(status(&repo, "story:ready-one"), "proposed");
    let second = frontier
        .next(&repo, &mut out)
        .expect("the frontier answers")
        .expect("the second story is ready");
    assert_eq!(second.id, "story:ready-one");
    assert_eq!(status(&repo, "story:ready-one"), "active");
    let third = frontier
        .next(&repo, &mut out)
        .expect("the frontier answers");
    assert_eq!(third, None);
    assert_eq!(
        String::from_utf8_lossy(&out),
        "",
        "nothing was refused along the way"
    );
}

/// A dependency at `archived` is done, so the story waiting on it becomes ready; one at
/// `rejected` is not, so it stays waiting.
#[test]
fn an_archived_dependency_is_done_and_a_rejected_one_is_not() {
    let archived = repository("none-ready", "dep-archived");
    aep(
        &archived,
        &[
            "move",
            "story:draft-dep",
            "--to",
            "archived",
            "--executor",
            "agent:fixture",
        ],
    );
    assert_eq!(status(&archived, "story:draft-dep"), "archived");
    let mut out = Vec::new();
    let story = Aep::default()
        .next(&archived, &mut out)
        .expect("the frontier answers")
        .expect("the waiting story is ready now");
    assert_eq!(story.id, "story:waits-one");
    assert_eq!(status(&archived, "story:waits-one"), "active");

    let rejected = repository("none-ready", "dep-rejected");
    aep(
        &rejected,
        &[
            "move",
            "story:draft-dep",
            "--to",
            "proposed",
            "--executor",
            "agent:fixture",
        ],
    );
    aep(
        &rejected,
        &[
            "move",
            "story:draft-dep",
            "--to",
            "rejected",
            "--executor",
            "agent:fixture",
        ],
    );
    assert_eq!(status(&rejected, "story:draft-dep"), "rejected");
    let unit = Aep::default()
        .next(&rejected, &mut out)
        .expect("the frontier answers");
    assert_eq!(unit, None, "{}", String::from_utf8_lossy(&out));
    assert_eq!(status(&rejected, "story:waits-one"), "proposed");
}

/// `show` and `take` are public and run `aep` in the repository given, like `list`, but only
/// `list` refuses a repository without a store. Called on one, `aep` walks up and reads (and
/// for `take`, tries to move) in the store above it: here, this repository's own. The id is
/// one no store holds, so the probe reads and nothing is written; the guard is still owed.
#[test]
fn show_and_take_refuse_a_repository_without_a_planning_store() {
    let dir = scratch("no-store-show");
    fs::write(dir.join("README.md"), "no store\n").expect("a file");
    let adapter = Aep::default();
    let shown = adapter
        .show(&dir, "story:adversary-absent")
        .expect_err("show on a repository without a store is refused")
        .to_string();
    assert!(shown.contains("no planning store"), "{shown}");
    let taken = adapter.take(&dir, "story:adversary-absent");
    let error = match taken {
        Ok(answer) => panic!("take on a repository without a store ran aep: {answer:?}"),
        Err(error) => error.to_string(),
    };
    assert!(error.contains("no planning store"), "{error}");
}

/// A run opens the case before the store is told and removes it when the store refuses: with
/// the refusing stand-in, `story:a`'s case is opened, the take is refused, the case directory
/// goes, the refusal is printed verbatim, and `story:b` gets the one case there is. The stub's
/// log shows the order: each story is shown, its case opened, then its move asked for.
#[test]
fn a_refused_take_removes_the_case_it_opened_and_the_next_story_gets_one() {
    let repo = repository("one-ready", "refused-case");
    let frontier = Aep {
        command: refusing_aep(&repo),
    };
    let validators: Vec<Box<dyn Validator>> = vec![];
    let mut out = Vec::new();
    let run = Run::open_next(&repo, options(), &frontier, &Never, &validators, &mut out)
        .expect("the frontier answers")
        .expect("story:b is taken");
    let Unit::Story(story) = &run.unit else {
        panic!("the unit is a story, not {:?}", run.unit);
    };
    assert_eq!(story.id, "story:b");
    assert!(run.case.id.starts_with("story-b-"), "{}", run.case.id);
    let cases: Vec<String> = fs::read_dir(repo.join(".engineering/cases"))
        .expect("a cases directory")
        .map(|e| {
            e.expect("an entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert_eq!(
        cases,
        std::slice::from_ref(&run.case.id),
        "one case directory, the taken story's; none for the refused one"
    );
    assert_eq!(
        String::from_utf8_lossy(&out),
        "frontier: story:a not taken: story:a is proposed; the profile wants it to serve an objective first\n\
         frontier: took story:b: Title of story:b\n"
    );
    let calls = fs::read_to_string(repo.join("calls.log")).expect("the stub logged its calls");
    let order: Vec<&str> = calls
        .lines()
        .filter(|line| {
            line.starts_with("plan artifact show ") || line.starts_with("plan artifact move ")
        })
        .collect();
    assert_eq!(
        order,
        [
            "plan artifact show story:a --format json",
            "plan artifact move story:a --to active --executor agent:takt",
            "plan artifact show story:b --format json",
            "plan artifact move story:b --to active --executor agent:takt",
        ],
        "{calls}"
    );
}

/// Nothing ready through the case-first path: no case directory is opened at all.
#[test]
fn nothing_ready_opens_no_case_through_the_run() {
    let repo = repository("none-ready", "nothing-run");
    let validators: Vec<Box<dyn Validator>> = vec![];
    let mut out = Vec::new();
    let run = Run::open_next(
        &repo,
        options(),
        &Aep::default(),
        &Never,
        &validators,
        &mut out,
    )
    .expect("the frontier answers");
    assert!(run.is_none(), "{}", String::from_utf8_lossy(&out));
    assert_eq!(String::from_utf8_lossy(&out), "");
    assert!(!repo.join(".engineering/cases").exists());
}

/// A stand-in for `aep` like [`refusing_aep`], refusing to move every id in `refused` and
/// moving the rest: the stores the fixtures can build never refuse, so the refusal paths are
/// reached only through a stub. Every call is appended to `calls.log` beside the script.
fn stub_aep(dir: &Path, refused: &[&str]) -> String {
    let log = dir.join("calls.log");
    let pattern = if refused.is_empty() {
        "never-matches".to_owned()
    } else {
        refused.join("|")
    };
    let script = r##"#!/bin/sh
printf '%s\n' "$*" >> "@LOG@"
case "$3" in
  list)
    printf '%s\n' '[{"id":"story:c","kind":"story","status":"proposed","title":"C","path":"story/c.md","relations":[],"refs":[],"blocked_by":[]},{"id":"story:a","kind":"story","status":"proposed","title":"A","path":"story/a.md","relations":[],"refs":[],"blocked_by":[]},{"id":"story:b","kind":"story","status":"proposed","title":"B","path":"story/b.md","relations":[],"refs":[],"blocked_by":[]}]'
    ;;
  show)
    printf '{"id":"%s","kind":"story","status":"proposed","title":"Title of %s","body":"# Story\\n"}\n' "$4" "$4"
    ;;
  move)
    case "$4" in
      @REFUSED@)
        printf '%s\n' "$4 is proposed; the profile wants it to serve an objective first" >&2
        exit 1
        ;;
    esac
    printf '%s moved proposed -> active (revision 2)\n' "$4"
    ;;
  *)
    printf 'unexpected: %s\n' "$*" >&2
    exit 2
    ;;
esac
"##
    .replace("@LOG@", &log.display().to_string())
    .replace("@REFUSED@", &pattern);
    let path = dir.join("aep-stub-adv2.sh");
    fs::write(&path, script).expect("the stub is written");
    ignore_stub(dir);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("executable");
    }
    path.display().to_string()
}

/// The names under `<repo>/.engineering/cases`, sorted; `None` when the directory is absent.
fn case_names(repo: &Path) -> Option<Vec<String>> {
    let dir = repo.join(".engineering/cases");
    if !dir.exists() {
        return None;
    }
    let mut names: Vec<String> = fs::read_dir(&dir)
        .expect("a readable cases directory")
        .map(|e| {
            e.expect("an entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    Some(names)
}

/// Every candidate refused: the frontier answers `None`, each refusal is printed in id order,
/// and nothing is left on disk, not even an empty `cases/` directory, since each refused
/// story's case was opened and removed in turn.
#[test]
fn a_store_refusing_every_candidate_leaves_no_cases_directory_behind() {
    let repo = repository("one-ready", "adv2-all-refused");
    let frontier = Aep {
        command: stub_aep(&repo, &["story:a", "story:b", "story:c"]),
    };
    let validators: Vec<Box<dyn Validator>> = vec![];
    let mut out = Vec::new();
    let run = Run::open_next(&repo, options(), &frontier, &Never, &validators, &mut out)
        .expect("the frontier answers");
    assert!(run.is_none(), "{}", String::from_utf8_lossy(&out));
    assert_eq!(
        String::from_utf8_lossy(&out),
        "frontier: story:a not taken: story:a is proposed; the profile wants it to serve an objective first\n\
         frontier: story:b not taken: story:b is proposed; the profile wants it to serve an objective first\n\
         frontier: story:c not taken: story:c is proposed; the profile wants it to serve an objective first\n"
    );
    assert_eq!(
        case_names(&repo),
        None,
        "no case and no empty cases directory is left after every candidate was refused"
    );
    let calls = fs::read_to_string(repo.join("calls.log")).expect("the stub logged its calls");
    let moves: Vec<&str> = calls
        .lines()
        .filter(|line| line.starts_with("plan artifact move "))
        .collect();
    assert_eq!(moves.len(), 3, "{calls}");
}

/// A given case id names the taken story's case, even when the first candidate was refused
/// and its case, under the same id, had to go first: the id is not silently dropped for a
/// story, and the second open under the same id succeeds because the first was removed.
#[test]
fn a_given_case_id_names_the_taken_story_case_after_a_refusal() {
    let repo = repository("one-ready", "adv2-given-id");
    let frontier = Aep {
        command: stub_aep(&repo, &["story:a"]),
    };
    let validators: Vec<Box<dyn Validator>> = vec![];
    let mut options = options();
    options.case_id = Some("given".to_owned());
    let mut out = Vec::new();
    let run = Run::open_next(&repo, options, &frontier, &Never, &validators, &mut out)
        .expect("the frontier answers")
        .expect("story:b is taken");
    let Unit::Story(story) = &run.unit else {
        panic!("the unit is a story, not {:?}", run.unit);
    };
    assert_eq!(story.id, "story:b");
    assert_eq!(
        run.case.id, "given",
        "the case id given is the case id used for a story"
    );
    assert_eq!(run.case.dir, repo.join(".engineering/cases/given"));
    assert_eq!(case_names(&repo), Some(vec!["given".to_owned()]));
    let intent = fs::read_to_string(run.case.dir.join("intent.md")).expect("intent.md");
    assert_eq!(
        intent, "Title of story:b\n\n# Story\n",
        "the case holds the taken story, not the refused one"
    );
    assert_eq!(
        String::from_utf8_lossy(&out),
        "frontier: story:a not taken: story:a is proposed; the profile wants it to serve an objective first\n\
         frontier: took story:b: Title of story:b\n"
    );
}

/// A refusal removes the case it opened and nothing beside it: a case from an earlier run in
/// the same `cases/` directory survives, and the directory stays because it is not empty.
#[test]
fn a_refusal_removes_only_the_case_it_opened_not_its_neighbours() {
    let repo = repository("one-ready", "adv2-neighbour");
    let earlier = repo.join(".engineering/cases/earlier");
    fs::create_dir_all(earlier.join("evidence")).expect("an earlier case");
    fs::write(
        earlier.join("case.yaml"),
        "format: canon-case/1\nid: earlier\n",
    )
    .expect("its snapshot");
    let frontier = Aep {
        command: stub_aep(&repo, &["story:a"]),
    };
    let validators: Vec<Box<dyn Validator>> = vec![];
    let mut out = Vec::new();
    let run = Run::open_next(&repo, options(), &frontier, &Never, &validators, &mut out)
        .expect("the frontier answers")
        .expect("story:b is taken");
    assert!(run.case.id.starts_with("story-b-"), "{}", run.case.id);
    assert_eq!(
        case_names(&repo),
        Some(vec!["earlier".to_owned(), run.case.id.clone()]),
        "the earlier case survives the refusal of story:a"
    );
    assert!(
        earlier.join("case.yaml").is_file(),
        "the earlier case's snapshot is untouched"
    );
}

/// The case is opened before the store is told: when the open fails (a protocol nothing
/// provides), the frontier asks for no move at all and leaves no case directory, so the story
/// stays `proposed` for the next run. Pinned at the library level, where the stub's log shows
/// `show` and never `move`.
#[test]
fn an_open_that_fails_asks_for_no_move_and_leaves_no_case() {
    let repo = repository("one-ready", "adv2-open-fails");
    let frontier = Aep {
        command: stub_aep(&repo, &[]),
    };
    let validators: Vec<Box<dyn Validator>> = vec![];
    let mut options = options();
    options.protocol = "no-such/1".to_owned();
    let mut out = Vec::new();
    let error = match Run::open_next(&repo, options, &frontier, &Never, &validators, &mut out) {
        Ok(_) => panic!("the protocol is not provided, yet a run opened"),
        Err(error) => error.to_string(),
    };
    assert!(error.contains("no-such/1"), "{error}");
    assert_eq!(
        String::from_utf8_lossy(&out),
        "",
        "nothing was taken or refused"
    );
    let calls = fs::read_to_string(repo.join("calls.log")).expect("the stub logged its calls");
    assert!(
        calls
            .lines()
            .any(|line| line == "plan artifact show story:a --format json"),
        "{calls}"
    );
    assert!(
        !calls
            .lines()
            .any(|line| line.starts_with("plan artifact move ")),
        "no move was asked for: {calls}"
    );
    assert_eq!(case_names(&repo), None, "no case directory was opened");
}

/// An open refused on a case input (a value outside the protocol's declared ones) asks for no
/// move either, so the story stays `proposed`, and leaves no case directory: `CaseDir::open`
/// checks the inputs before it writes anything, so the contract "one case directory per taken
/// story, none for a refused one" has no third outcome.
#[test]
fn an_open_refused_on_its_inputs_asks_for_no_move_and_leaves_no_case() {
    let repo = repository("one-ready", "adv2-input-refused");
    let frontier = Aep {
        command: stub_aep(&repo, &[]),
    };
    let validators: Vec<Box<dyn Validator>> = vec![];
    let mut options = options();
    options.inputs[0] = ("risk".to_owned(), "bogus".to_owned());
    let mut out = Vec::new();
    let error = match Run::open_next(&repo, options, &frontier, &Never, &validators, &mut out) {
        Ok(_) => panic!("the input is outside the declared values, yet a run opened"),
        Err(error) => error.to_string(),
    };
    assert!(error.contains("risk"), "{error}");
    let calls = fs::read_to_string(repo.join("calls.log")).expect("the stub logged its calls");
    assert!(
        !calls
            .lines()
            .any(|line| line.starts_with("plan artifact move ")),
        "no move was asked for: {calls}"
    );
    assert_eq!(
        case_names(&repo),
        None,
        "a story that was not taken has no case directory"
    );
}
