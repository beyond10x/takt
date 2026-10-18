//! Adversary pass 2 against story:validator-confinement, attacking correction 1. Pass 1's
//! findings F1-F5 are closed on this tree; this file attacks what the correction's own new code
//! opened. Correction 1 pins the base commit in `branch.yaml` at open, and for a case opened
//! before the pin existed (`base_commit` absent) falls back to the merge-base of HEAD and the
//! base branch. A test command that resets the base branch to HEAD collapses that merge-base
//! onto HEAD, so the gate is read from the model-edited tree again: the very F3 defect, revived
//! for every case still in flight across the migration that introduced the pin.

use std::cell::Cell;
use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::anyhow;
use takt::harness::{Harness, StepContext, StepReport};
use takt::validator::{GateRun, TestsRun, Validator};
use takt::{Run, RunOptions};

const PROTOCOL: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../protocols/story-delivery/1.yaml"
);

fn git(tree: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(args)
        .current_dir(tree)
        .output()
        .expect("git runs");
    assert!(output.status.success(), "git {args:?}");
    String::from_utf8(output.stdout)
        .expect("utf-8")
        .trim()
        .to_owned()
}

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

fn directory(name: &str) -> Scratch {
    let dir =
        Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("adv2-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(&dir).expect("scratch dir");
    Scratch(dir)
}

fn gates(assertion: &str) -> String {
    format!(
        "format: engineering-gates/1\nlanguage: canon-expr/1\ngates:\n  - id: check\n    \
         text: The check holds.\n    assertions:\n      - '{assertion}'\n"
    )
}

fn repository(name: &str, assertion: &str) -> Scratch {
    let dir = directory(name);
    fs::create_dir_all(dir.join(".engineering")).expect("the engineering directory");
    fs::write(dir.join("README.md"), "a story\n").expect("a file");
    fs::write(dir.join(".engineering/gates.yaml"), gates(assertion)).expect("the gates");
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "start"]);
    dir
}

fn options(id: &str, iterations: u64) -> RunOptions {
    RunOptions {
        protocol: PROTOCOL.to_owned(),
        intent: "deliver the story".to_owned(),
        inputs: Vec::new(),
        iterations: Some(iterations),
        hours: None,
        tokens: None,
        case_id: Some(id.to_owned()),
        gate: Some("repository.merge".to_owned()),
    }
}

fn records(case_dir: &Path) -> Vec<serde_json::Value> {
    let Ok(entries) = fs::read_dir(case_dir.join("evidence")) else {
        return Vec::new();
    };
    let mut paths: Vec<PathBuf> = entries
        .map(|entry| entry.expect("an entry").path())
        .collect();
    paths.sort();
    paths
        .iter()
        .map(|path| serde_json::from_slice(&fs::read(path).expect("a record")).expect("json"))
        .collect()
}

/// Writes `files` under `implementation.edit` on its first step; errors on any later one.
struct Writes {
    files: Vec<(&'static str, String)>,
    calls: Cell<usize>,
}

impl Writes {
    fn new(files: &[(&'static str, &str)]) -> Self {
        Self {
            files: files.iter().map(|(p, t)| (*p, (*t).to_owned())).collect(),
            calls: Cell::new(0),
        }
    }
}

impl Harness for Writes {
    fn name(&self) -> &str {
        "writes"
    }

    fn step(&self, context: &StepContext<'_>) -> anyhow::Result<StepReport> {
        self.calls.set(self.calls.get() + 1);
        if self.calls.get() > 1 {
            return Err(anyhow!("no further step"));
        }
        for (path, text) in &self.files {
            let path = context.tree.join(path);
            fs::create_dir_all(path.parent().expect("a parent"))?;
            fs::write(&path, text)?;
            if path.extension().is_some_and(|e| e == "sh") {
                fs::set_permissions(&path, fs::Permissions::from_mode(0o755))?;
            }
        }
        Ok(StepReport {
            action: "implementation.edit".to_owned(),
            summary: "edited".to_owned(),
            tokens: None,
        })
    }
}

/// Deletes the `base_commit:` line from the case's `branch.yaml`, so the case looks like one
/// opened before correction 1 recorded the pin: `base_revision` then falls back to the
/// merge-base of HEAD and the base branch.
fn unpin_base(case_dir: &Path) {
    let path = case_dir.join("branch.yaml");
    let text = fs::read_to_string(&path).expect("branch.yaml");
    assert!(
        text.lines()
            .any(|l| l.trim_start().starts_with("base_commit:")),
        "branch.yaml has no base_commit to remove:\n{text}"
    );
    let without: String = text
        .lines()
        .filter(|l| !l.trim_start().starts_with("base_commit:"))
        .map(|l| format!("{l}\n"))
        .collect();
    fs::write(&path, without).expect("rewrite branch.yaml");
}

/// A test command that resets the case's base branch `main` to HEAD without touching the tree.
/// For a case on the merge-base fallback, `merge-base(HEAD, main) == HEAD`, so the gate reads
/// its gates document from HEAD, the model-edited branch.
const RESET_MAIN_TO_HEAD: &str =
    "#!/bin/sh\ngit update-ref refs/heads/main \"$(git rev-parse HEAD)\"\nexit 0\n";

/// The acceptance (finding 3): a gate rewritten on the case branch does not change the verdict.
/// For a legacy, unpinned case the model rewrites the gates document on its branch and resets
/// the base branch to HEAD; the gate must still read the base revision's honest verdict (fail),
/// not the rewritten one.
#[test]
fn a_legacy_case_without_a_base_pin_lets_a_reset_of_main_decide_the_gate() {
    // The honest gate is false: the tree has no LICENSE.
    let tree = repository("legacy-reset", r#"fs.file.exists("LICENSE")"#);
    let rewritten = gates(r#"fs.file.exists("README.md")"#);
    let validators: Vec<Box<dyn Validator>> = vec![
        Box::new(TestsRun::new("sh tests/run.sh").expect("a command")),
        Box::new(GateRun::new(".engineering/gates.yaml")),
    ];
    let harness = Writes::new(&[
        (".engineering/gates.yaml", &rewritten),
        ("tests/run.sh", RESET_MAIN_TO_HEAD),
    ]);
    let mut run =
        Run::open(&tree, options("legacy-reset", 10), &harness, &validators).expect("opens");
    unpin_base(&run.case.dir);
    let start = git(&tree, &["rev-parse", "main"]);
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let text = String::from_utf8(out).expect("utf-8");
    let gate: Vec<_> = records(&run.case.dir)
        .into_iter()
        .filter(|r| r["kind"] == "gate_result")
        .collect();
    assert!(
        gate.iter().all(|r| r["result"] == "fail"),
        "main moved from {start} to {}; a gate rewritten on the case branch passed on a legacy case: {gate:?}\n{stop:?}\n{text}",
        git(&tree, &["rev-parse", "main"])
    );
}

/// A control: on a pinned case (correction 1's default for a freshly opened case) the same
/// attack is defeated, because the base commit is read from the pin, not a branch name. This
/// stays green and shows the finding is specific to the fallback.
#[test]
fn a_pinned_case_defeats_a_reset_of_main() {
    let tree = repository("pinned-reset", r#"fs.file.exists("LICENSE")"#);
    let rewritten = gates(r#"fs.file.exists("README.md")"#);
    let validators: Vec<Box<dyn Validator>> = vec![
        Box::new(TestsRun::new("sh tests/run.sh").expect("a command")),
        Box::new(GateRun::new(".engineering/gates.yaml")),
    ];
    let harness = Writes::new(&[
        (".engineering/gates.yaml", &rewritten),
        ("tests/run.sh", RESET_MAIN_TO_HEAD),
    ]);
    let mut run =
        Run::open(&tree, options("pinned-reset", 10), &harness, &validators).expect("opens");
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let text = String::from_utf8(out).expect("utf-8");
    let gate: Vec<_> = records(&run.case.dir)
        .into_iter()
        .filter(|r| r["kind"] == "gate_result")
        .collect();
    assert!(
        gate.iter().all(|r| r["result"] == "fail"),
        "a pinned case was decided by a reset of main: {gate:?}\n{stop:?}\n{text}"
    );
}
