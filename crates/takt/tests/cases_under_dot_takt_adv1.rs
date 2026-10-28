//! Adversarial cases for story:cases-live-under-dot-takt, library side. A judgement step that
//! writes into the tree is refused and undone; these cases aim at what lies around the new cases
//! root: `.takt/` itself, its `.gitignore` and the other files a repository or a step keeps in it.

#[path = "support/own_home.rs"]
mod own_home;

use std::cell::Cell;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::anyhow;
use takt::harness::{Harness, StepContext, StepReport};
use takt::validator::{TestsRun, Validator};
use takt::{Run, RunOptions, Stop};

/// Runs `git <args>` in `tree`; its trimmed standard output.
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

/// A scratch directory under this crate's build directory holding the tree at `tree/`, so what
/// a step writes at `../x` stays inside it; removed when dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn tree(&self) -> PathBuf {
        self.0.join("tree")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// A committed repository on `main` with a README and `files`, at `tree/` in a scratch
/// directory named for this process and `name`.
fn repository(name: &str, files: &[(&str, &str)]) -> Scratch {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("dot-takt-adv1-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    let scratch = Scratch(dir);
    let tree = scratch.tree();
    fs::create_dir_all(&tree).expect("the tree");
    fs::write(tree.join("README.md"), "a story\n").expect("a file");
    for (path, text) in files {
        let path = tree.join(path);
        fs::create_dir_all(path.parent().expect("a parent")).expect("a parent directory");
        fs::write(&path, text).expect("a file");
    }
    git(&tree, &["init", "-q", "-b", "main"]);
    git(&tree, &["add", "-A"]);
    git(&tree, &["commit", "-q", "-m", "start"]);
    scratch
}

fn options(id: &str, iterations: u64) -> RunOptions {
    RunOptions {
        protocol: "story.delivery/1".to_owned(),
        intent: "deliver the story".to_owned(),
        inputs: Vec::new(),
        iterations: Some(iterations),
        hours: None,
        tokens: None,
        case_id: Some(id.to_owned()),
        gate: None,
    }
}

/// A harness whose first judgement step runs `script` with `sh` in the tree and reports an
/// edit; any later step ends the run with an error.
struct Script {
    script: String,
    calls: Cell<usize>,
}

impl Script {
    fn new(script: &str) -> Self {
        Self {
            script: script.to_owned(),
            calls: Cell::new(0),
        }
    }
}

impl Harness for Script {
    fn name(&self) -> &str {
        "script"
    }

    fn step(&self, context: &StepContext<'_>) -> anyhow::Result<StepReport> {
        self.calls.set(self.calls.get() + 1);
        if self.calls.get() > 1 {
            return Err(anyhow!("no further step"));
        }
        let status = Command::new("sh")
            .args(["-c", &self.script])
            .current_dir(context.tree)
            .status()
            .expect("sh runs");
        assert!(status.success(), "{}", self.script);
        Ok(StepReport {
            action: "implementation.edit".to_owned(),
            summary: "edited".to_owned(),
            tokens: None,
        })
    }
}

/// Opens the case `id` in `tree`, runs `before` in the tree once it is open, then runs the case
/// with one judgement step running `script`: its stop and its output.
fn run_step(tree: &Path, id: &str, before: &str, script: &str) -> (anyhow::Result<Stop>, String) {
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(TestsRun::new("true").expect("a command"))];
    let harness = Script::new(script);
    let mut run = Run::open(tree, options(id, 4), &harness, &validators).expect("the case opens");
    if !before.is_empty() {
        let status = Command::new("sh")
            .args(["-c", before])
            .current_dir(tree)
            .status()
            .expect("sh runs");
        assert!(status.success(), "{before}");
    }
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    drop(run);
    let text = format!(
        "calls {}\n{stop:?}\n{}",
        harness.calls.get(),
        String::from_utf8_lossy(&out)
    );
    (stop, text)
}

/// The paths `HEAD` tracks under `.takt`, one a line.
fn tracked_under_dot_takt(tree: &Path) -> String {
    git(
        tree,
        &["ls-tree", "-r", "--name-only", "HEAD", "--", ".takt"],
    )
}

/// A judgement step that puts a link in `.takt`'s place, leading to an identical copy of it
/// beside the tree, is refused and undone like a link in the cases root's place: `.takt` is a
/// directory again holding the case and takt's `.gitignore`, nothing of `.takt` is committed and
/// `git status` is clean. The new root's parent is not on the judge surface, as `.engineering` is,
/// and a snapshot of `.takt/cases` read through the link sees the copy, identical, so nothing
/// else catches it. Without a root `.gitignore`, and with one ignoring `.takt`.
#[test]
fn a_step_that_replaces_dot_takt_with_a_link_to_a_copy_is_refused_and_undone() {
    for (n, files) in [vec![], vec![(".gitignore", ".takt\n")]]
        .into_iter()
        .enumerate()
    {
        let id = format!("dot-takt-link-{n}");
        let scratch = repository(&id, &files);
        let tree = scratch.tree();
        let (stop, text) = run_step(
            &tree,
            &id,
            "",
            "cp -r .takt ../x && rm -rf .takt && ln -s ../x .takt",
        );
        assert!(
            matches!(stop, Ok(Stop::StepRefused(_))),
            "config {n}: the step is refused\n{text}"
        );
        assert!(
            fs::symlink_metadata(tree.join(".takt")).is_ok_and(|meta| meta.is_dir()),
            "config {n}: .takt is a directory again, not a link\n{text}"
        );
        assert!(
            tree.join(".takt/cases")
                .join(&id)
                .join("case.yaml")
                .is_file(),
            "config {n}: the case is under .takt/cases in the tree\n{text}"
        );
        assert_eq!(
            fs::read_to_string(tree.join(".takt/.gitignore")).unwrap_or_default(),
            "*\n",
            "config {n}: takt's .takt/.gitignore is back\n{text}"
        );
        assert_eq!(
            tracked_under_dot_takt(&tree),
            "",
            "config {n}: nothing of .takt is committed\n{text}"
        );
        assert_eq!(
            git(&tree, &["status", "--porcelain"]),
            "",
            "config {n}: git status is clean\n{text}"
        );
    }
}

/// A judgement step that rewrites a file the operator keeps in an ignored `.takt/` (beside the
/// cases root) is refused, and its bytes do not stay in the tree: the file is put back, or moved
/// aside as a file the step rewrote inside an ignored directory is. With `.takt/` ignored by
/// takt's own `.takt/.gitignore` alone, git names the file by itself and it is put back; with
/// `.takt` also ignored by the repository's root `.gitignore`, git names `.takt/` as one ignored
/// directory, which the checkpoint keeps as what is in it, never as itself.
#[test]
fn a_file_a_refused_step_rewrote_in_an_ignored_dot_takt_does_not_keep_its_bytes() {
    for (n, files) in [vec![], vec![(".gitignore", ".takt/\n")]]
        .into_iter()
        .enumerate()
    {
        let id = format!("dot-takt-note-{n}");
        let scratch = repository(&id, &files);
        let tree = scratch.tree();
        let (stop, text) = run_step(
            &tree,
            &id,
            "printf 'operator\\n' > .takt/note",
            "printf 'step\\n' > .takt/note",
        );
        assert!(
            matches!(stop, Ok(Stop::StepRefused(_))),
            "config {n}: a step rewriting an ignored file is refused\n{text}"
        );
        let note = fs::read_to_string(tree.join(".takt/note")).unwrap_or_default();
        assert_ne!(
            note, "step\n",
            "config {n}: the refused step's bytes stay in .takt/note\n{text}"
        );
    }
}

/// A judgement step that rewrites `.takt/.gitignore` to ignore nothing is refused and the file
/// put back as takt wrote it, so `git status` stays clean, with and without a root `.gitignore`
/// that ignores `.takt/`.
#[test]
fn a_step_rewriting_the_takt_gitignore_is_refused_and_put_back() {
    for (n, files) in [vec![], vec![(".gitignore", ".takt/\n")]]
        .into_iter()
        .enumerate()
    {
        let id = format!("dot-takt-ignore-{n}");
        let scratch = repository(&id, &files);
        let tree = scratch.tree();
        let (stop, text) = run_step(&tree, &id, "", "printf '!*\\n' > .takt/.gitignore");
        assert!(
            matches!(stop, Ok(Stop::StepRefused(_))),
            "config {n}: the step is refused\n{text}"
        );
        assert_eq!(
            fs::read_to_string(tree.join(".takt/.gitignore")).unwrap_or_default(),
            "*\n",
            "config {n}: .takt/.gitignore is put back\n{text}"
        );
        assert_eq!(
            git(&tree, &["status", "--porcelain"]),
            "",
            "config {n}: git status is clean\n{text}"
        );
    }
}
