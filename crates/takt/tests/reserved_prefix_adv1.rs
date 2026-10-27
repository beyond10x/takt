//! Adversarial cases for story:a-takt-prefixed-base-is-named-as-reserved: a base branch under
//! `takt/` that is no case's branch is refused by naming the reserved prefix and a rename,
//! whatever follows the prefix.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use takt::case::Approval;
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

struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn repository(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "reserved-prefix-adv1-{}-{name}",
        std::process::id()
    ));
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
        protocol: "story.delivery/1".to_owned(),
        intent: "deliver".to_owned(),
        inputs: Vec::new(),
        iterations: Some(1),
        hours: None,
        tokens: None,
        case_id: Some("fresh".to_owned()),
        gate: None,
    }
}

struct NoHarness;

impl Harness for NoHarness {
    fn name(&self) -> &str {
        "none"
    }

    fn step(&self, _: &StepContext<'_>) -> anyhow::Result<StepReport> {
        panic!("no step is asked for")
    }
}

struct OneStory;

impl Frontier for OneStory {
    fn name(&self) -> &str {
        "stories"
    }

    fn candidates(&self, _: &Path) -> anyhow::Result<Vec<String>> {
        Ok(vec!["story:short".to_owned()])
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

    fn take(&self, _: &Path, _: &str) -> anyhow::Result<Take> {
        Ok(Take::Taken)
    }

    fn record_approval(&self, _: &Path, _: &str, _: &Approval) -> anyhow::Result<()> {
        Ok(())
    }
}

/// A base named `takt/feature/x`, the common `<prefix>/<topic>/<name>` shape, is no case's branch
/// either: the story's outcome refuses every such base by naming `takt/` and a rename. The run is
/// refused, but as "names no case", the message about a case that does not exist which the story
/// set out to replace.
#[test]
fn a_nested_takt_prefixed_base_is_named() {
    let tree = repository("nested");
    git(&tree.0, &["checkout", "-q", "-b", "takt/feature/x"]);
    let validators: Vec<Box<dyn Validator>> = Vec::new();
    let error = match Run::open(&tree.0, options(), &NoHarness, &validators) {
        Ok(run) => panic!("opened {:?}", run.case.id),
        Err(error) => format!("{error:#}"),
    };
    assert!(
        error.contains("reserved prefix") && error.contains("git branch -m"),
        "Run::open: {error}"
    );
    let mut out = Vec::new();
    let error = match Run::next_to_resume(&tree.0, &OneStory, &mut out) {
        Ok(found) => panic!("resumed {found:?} from a reserved base"),
        Err(error) => format!("{error:#}"),
    };
    assert!(
        error.contains("reserved prefix") && error.contains("git branch -m"),
        "Run::next_to_resume: {error}"
    );
    assert!(!tree.0.join(".takt/cases").exists(), "a case directory");
}
