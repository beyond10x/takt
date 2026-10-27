//! Adversary pass 2 on `check_id` after correction 1: the shown form of a refused id, the
//! branch name `current` repeats, the lone `@` guard, and the ids takt mints itself; and, from
//! the review after it, a minted id too long for one story that must not stop the frontier,
//! and a branch name joined to a path before it is checked as a case id. Each case is named
//! for what it asserts.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use std::cell::RefCell;

use takt::case::{Approval, check_id, current};
use takt::frontier::Take;
use takt::validator::Validator;
use takt::{Frontier, Harness, Run, RunOptions, StepContext, StepReport, Story};

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

/// A scratch repository under this crate's build directory, removed on drop.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn repository(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("case-id-adv2-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(&dir).expect("scratch dir");
    fs::write(dir.join("README.md"), "a story\n").expect("a file");
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "start"]);
    Scratch(dir)
}

/// `shown` writes an invisible character as `\u{...}` and a backslash as itself, so two ids
/// typed differently can be shown alike: `a\` U+200B and `a\\u{200b}` (nine characters) both
/// come back as "the case id `a\\u{200b}` is refused: it contains a path separator", and the
/// operator cannot tell from the refusal which id it names. Decided: the message is for a
/// person and both ids are refused, so the case holds the refusal of both.
#[test]
fn two_ids_shown_alike_are_both_refused() {
    let invisible = "a\\\u{200b}";
    let typed = "a\\\\u{200b}";
    assert_ne!(invisible, typed);
    let first = format!("{:#}", check_id(invisible).expect_err("a backslash"));
    let second = format!("{:#}", check_id(typed).expect_err("a backslash"));
    assert!(
        first.contains("is refused") && second.contains("is refused"),
        "both ids are refused: {} / {}",
        first.escape_debug(),
        second.escape_debug()
    );
}

/// git accepts U+202E RIGHT-TO-LEFT OVERRIDE in a branch name; `check_id` refuses it and its
/// own refusal writes it as `\u{202e}`, but `current` wraps that refusal in "the branch
/// {branch} names no case" with the branch name as git gave it, so the override reaches the
/// terminal and reverses the rest of the line.
#[test]
fn current_writes_a_bidi_override_in_the_branch_name_as_its_code_point() {
    let tree = repository("bidi");
    git(&tree.0, &["checkout", "-q", "-b", "takt/evil\u{202e}kcab"]);
    let error = format!("{:#}", current(&tree.0).expect_err("refused"));
    assert!(error.contains("\\u{202e}"), "{}", error.escape_debug());
    assert!(
        !error.contains('\u{202e}'),
        "U+202E written raw: {}",
        error.escape_debug()
    );
}

/// `branch_refusal` refuses `@` alone, which git accepts as `takt/@`, so the 1,600-id corpus
/// (git as the oracle) never reaches the guard, and no other case names `@` alone: the
/// guard can be dropped with the suite green. This case holds it.
#[test]
fn a_lone_at_is_refused_naming_head() {
    let refusal = format!("{:#}", check_id("@").expect_err("`@` alone"));
    assert!(refusal.contains("the case id `@` is refused"), "{refusal}");
    assert!(refusal.contains("HEAD"), "{refusal}");
}

/// The case id takt mints for a story, `story-<name>-<instant>`, is 23 bytes longer than the
/// story's name, so a name of 228 bytes or more makes an id `check_id` refuses for its length:
/// the run takes no case for that story, whatever the operator gives. The unit's message says
/// the ids takt builds cannot fail the check. Decided: story-minted ids stay as they are, and
/// such an id is refused by `check_id`, naming the limit, before anything is written; the case
/// holds that refusal.
#[test]
fn a_case_id_takt_mints_for_a_story_can_fail_check_id() {
    let story = Story {
        id: format!("story:{}", "a".repeat(228)),
        title: "a long name".to_owned(),
        body: String::new(),
    };
    let id = story.case_id("2026-10-18T12:00:00Z");
    assert_eq!(id.len(), 251);
    let refusal = format!("{:#}", check_id(&id).expect_err("251 bytes"));
    assert!(
        refusal.contains("251 bytes") && refusal.contains("250"),
        "{refusal}"
    );
}

const PROTOCOL: &str = "story.delivery/1";

fn options() -> RunOptions {
    RunOptions {
        protocol: PROTOCOL.to_owned(),
        intent: String::new(),
        inputs: Vec::new(),
        iterations: Some(1),
        hours: None,
        tokens: None,
        case_id: None,
        gate: None,
    }
}

/// Never asked for a step: these cases stop before one.
struct NoHarness;

impl Harness for NoHarness {
    fn name(&self) -> &str {
        "none"
    }

    fn step(&self, _: &StepContext<'_>) -> anyhow::Result<StepReport> {
        panic!("no step is asked for")
    }
}

/// A store of the stories `ids`, each ready in that order; a take writes `taken.txt` naming the
/// story, and is remembered.
struct Stories {
    ids: Vec<String>,
    taken: RefCell<Vec<String>>,
}

impl Frontier for Stories {
    fn name(&self) -> &str {
        "stories"
    }

    fn candidates(&self, _: &Path) -> anyhow::Result<Vec<String>> {
        Ok(self.ids.clone())
    }

    fn active(&self, _: &Path) -> anyhow::Result<Vec<String>> {
        Ok(Vec::new())
    }

    fn show(&self, _: &Path, id: &str) -> anyhow::Result<Story> {
        Ok(Story {
            id: id.to_owned(),
            title: "a story".to_owned(),
            body: String::new(),
        })
    }

    fn take(&self, repo: &Path, id: &str) -> anyhow::Result<Take> {
        fs::write(repo.join("taken.txt"), format!("{id}\n"))?;
        self.taken.borrow_mut().push(id.to_owned());
        Ok(Take::Taken)
    }

    fn record_approval(&self, _: &Path, _: &str, _: &Approval) -> anyhow::Result<()> {
        Ok(())
    }
}

/// A story whose name takes the case id takt mints for it past 250 bytes is skipped with a line
/// naming it and the refusal, and the next ready story is taken: one story's name never stops
/// the frontier.
#[test]
fn take_next_skips_a_story_whose_minted_case_id_is_too_long_and_takes_the_next() {
    let tree = repository("minted-too-long");
    let long = format!("story:{}", "a".repeat(228));
    let store = Stories {
        ids: vec![long.clone(), "story:short".to_owned()],
        taken: RefCell::new(Vec::new()),
    };
    let validators: Vec<Box<dyn Validator>> = Vec::new();
    let mut out = Vec::new();
    let taken = Run::take_next(
        &tree.0,
        options(),
        &store,
        &NoHarness,
        &validators,
        &mut out,
    );
    let text = String::from_utf8_lossy(&out).into_owned();
    let run = taken
        .map_err(|error| format!("{error:#}"))
        .expect("the frontier goes on past the long story")
        .expect("the next story is taken");
    assert_eq!(*store.taken.borrow(), ["story:short"], "{text}");
    assert!(
        matches!(&run.unit, takt::Unit::Story(story) if story.id == "story:short"),
        "{text}"
    );
    assert!(
        text.contains(&format!("frontier: {long} not taken:")) && text.contains("251 bytes"),
        "the skip is not said: {text}"
    );
}

/// A branch `takt/x/y` and, committed under the cases directory, `x/y/branch.yaml` naming a
/// base: no case id holds `/`, so the branch names no case, and the path joined from it is no
/// case's directory.
fn on_a_nested_branch(name: &str) -> Scratch {
    let tree = repository(name);
    git(&tree.0, &["checkout", "-q", "-b", "takt/x/y"]);
    let planted = tree.0.join(".takt/cases/x/y");
    fs::create_dir_all(&planted).expect("a directory");
    fs::write(planted.join("branch.yaml"), "name: takt/x/y\nbase: main\n").expect("a file");
    git(&tree.0, &["add", "-A"]);
    git(&tree.0, &["commit", "-q", "-m", "a planted branch record"]);
    tree
}

/// The next story is read from the base of the case whose branch the tree is on. A branch
/// whose name after `takt/` is no case id is refused, naming it, before that name is joined to
/// the cases directory: the tree stays where it is and nothing is taken.
#[test]
fn take_next_refuses_a_branch_that_names_no_case_before_reading_a_path_from_it() {
    let tree = on_a_nested_branch("to-base");
    let store = Stories {
        ids: vec!["story:short".to_owned()],
        taken: RefCell::new(Vec::new()),
    };
    let validators: Vec<Box<dyn Validator>> = Vec::new();
    let mut out = Vec::new();
    let taken = Run::take_next(
        &tree.0,
        options(),
        &store,
        &NoHarness,
        &validators,
        &mut out,
    );
    let error = match taken {
        Ok(run) => panic!(
            "taken {:?} from the base the planted record names",
            run.map(|run| run.case.id.clone())
        ),
        Err(error) => format!("{error:#}"),
    };
    assert!(
        error.contains("the branch takt/x/y names no case"),
        "{error}"
    );
    assert_eq!(git(&tree.0, &["branch", "--show-current"]), "takt/x/y");
    assert!(store.taken.borrow().is_empty());
}

/// A case opened on a tree that is on another case's branch forks from that case's base. A
/// branch whose name after `takt/` is no case id is refused, naming it, before that name is
/// joined to the cases directory: no case and no branch is made.
#[test]
fn a_case_opened_on_a_branch_that_names_no_case_is_refused_before_reading_a_path_from_it() {
    let tree = on_a_nested_branch("open");
    let validators: Vec<Box<dyn Validator>> = Vec::new();
    let mut opening = options();
    opening.intent = "deliver".to_owned();
    opening.case_id = Some("fresh".to_owned());
    let error = match Run::open(&tree.0, opening, &NoHarness, &validators) {
        Ok(run) => panic!(
            "the case opened on the base the planted record names: {:?}",
            takt::case::Branch::read(&run.case.dir)
        ),
        Err(error) => format!("{error:#}"),
    };
    assert!(
        error.contains("the branch takt/x/y names no case"),
        "{error}"
    );
    assert_eq!(git(&tree.0, &["branch", "--show-current"]), "takt/x/y");
    assert!(!tree.0.join(".takt/cases/fresh").exists());
}
