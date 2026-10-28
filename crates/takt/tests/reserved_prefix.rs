//! A base branch under takt's reserved prefix `takt/`, that is no case's branch, is refused by
//! name before a case opens, and a case's own branch still resolves to its recorded base.

#[path = "support/own_home.rs"]
mod own_home;

use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use takt::case::Approval;
use takt::frontier::Take;
use takt::validator::Validator;
use takt::{Frontier, Harness, Run, RunOptions, StepContext, StepReport, Story};

const PROTOCOL: &str = "story.delivery/1";

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
        .join(format!("reserved-prefix-{}-{name}", std::process::id()));
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

/// A store of the stories `ids`, each ready in that order; a take writes `taken.txt`.
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

fn assert_named(error: &str) {
    assert!(error.contains("reserved prefix"), "{error}");
    assert!(error.contains("`takt/x`"), "{error}");
    assert!(error.contains("`takt/`"), "{error}");
    assert!(error.contains("git branch -m takt/x trial/x"), "{error}");
}

/// On branch `takt/x` with no case `x`, every way a run starts refuses by name, before any case
/// directory or branch is made, and the tree stays where it was.
#[test]
fn a_takt_prefixed_base_is_named() {
    let tree = repository("named");
    git(&tree.0, &["checkout", "-q", "-b", "takt/x"]);
    let validators: Vec<Box<dyn Validator>> = Vec::new();

    let store = Stories {
        ids: vec!["story:short".to_owned()],
        taken: RefCell::new(Vec::new()),
    };
    let mut out = Vec::new();
    let error = match Run::next_to_resume(&tree.0, &store, &mut out) {
        Ok(found) => panic!("resumed {found:?} from a reserved base"),
        Err(error) => format!("{error:#}"),
    };
    assert_named(&error);

    let mut out = Vec::new();
    let error = match Run::take_next(
        &tree.0,
        options(),
        &store,
        &NoHarness,
        &validators,
        &mut out,
    ) {
        Ok(run) => panic!("taken {:?}", run.map(|run| run.case.id.clone())),
        Err(error) => format!("{error:#}"),
    };
    assert_named(&error);

    let mut opening = options();
    opening.intent = "deliver".to_owned();
    opening.case_id = Some("fresh".to_owned());
    let error = match Run::open(&tree.0, opening, &NoHarness, &validators) {
        Ok(run) => panic!("opened {:?}", run.case.id),
        Err(error) => format!("{error:#}"),
    };
    assert_named(&error);

    assert!(store.taken.borrow().is_empty());
    assert!(!tree.0.join(".takt/cases").exists(), "a case directory");
    assert_eq!(git(&tree.0, &["branch", "--show-current"]), "takt/x");
    assert_eq!(git(&tree.0, &["branch", "--list", "takt/*"]), "* takt/x");
}

/// On `takt/<id>` with that case open, the run goes on from the case's recorded base as before.
#[test]
fn a_case_branch_still_resolves() {
    let tree = repository("resolves");
    let validators: Vec<Box<dyn Validator>> = Vec::new();
    let mut opening = options();
    opening.intent = "deliver".to_owned();
    opening.case_id = Some("open-one".to_owned());
    Run::open(&tree.0, opening, &NoHarness, &validators).expect("a case opens");
    assert_eq!(git(&tree.0, &["branch", "--show-current"]), "takt/open-one");

    let store = Stories {
        ids: vec!["story:short".to_owned()],
        taken: RefCell::new(Vec::new()),
    };
    let mut out = Vec::new();
    Run::next_to_resume(&tree.0, &store, &mut out).expect("a case branch is no reserved base");
    let mut out = Vec::new();
    let run = Run::take_next(
        &tree.0,
        options(),
        &store,
        &NoHarness,
        &validators,
        &mut out,
    )
    .expect("the next story is taken from the case's base")
    .expect("a case opens");
    let base = takt::case::Branch::read(&run.case.dir)
        .expect("a branch record")
        .base;
    assert_eq!(base.as_deref(), Some("main"));
    assert_eq!(*store.taken.borrow(), ["story:short"]);
}
