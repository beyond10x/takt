//! story:git-excluded-files-are-not-step-changes. A path the run file names as the deployment's
//! (`deployment_paths`, `crates/takt/src/run_config.rs`) is no change a step made: a step that
//! writes there is neither refused, nor set aside, nor reverted for it, and no step commit
//! carries it. The list is the one read when the run opens: a step that rewrites the run file or
//! `.git/info/exclude` does not move it. A file git ignores only through `.git/info/exclude` or a
//! `core.excludesFile`, and not named in the list, stays a change
//! (story:git-reads-no-user-excludes holds). Every repository is a scratch one under the crate's
//! build directory; nothing is written outside it. A deployment's file in the tree before the
//! run opens keeps neither the case from opening nor the story from being taken, and no take
//! or store write commits or discards it.

#[path = "support/own_home.rs"]
mod own_home;

use std::cell::{Cell, RefCell};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use loom_sdk::session::{Proposal, ProposedFile};
use takt::case::{Approval, CommitAs};
use takt::frontier::{Frontier, Story, Take, Written};
use takt::git::DeploymentPaths;
use takt::harness::{Harness, StepContext, StepReport};
use takt::run_config::RunConfig;
use takt::validator::{TestsRun, Validator};
use takt::{Run, RunOptions, Stop};

const EDIT_AND_TEST: &str = "format: protocol/1

protocol:
  id: edit.and.test
  revision: 1
  description: >-
    Edit the implementation and run the tests against it; accepted once they pass.

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
  implementation.edit:
    description: change the implementation
    effect: write
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

/// The deployment's log, as the trial's wrapper wrote it into the workspace.
const LOG: &str = ".autodev/run-2.log";

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
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

struct Scratch {
    root: PathBuf,
    repo: PathBuf,
    /// The commit the repository starts at, before any step.
    start: String,
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// A repository whose test passes once `HEAD` holds `done.txt`, the protocol beside it.
fn repository(name: &str) -> Scratch {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("deployment-paths-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let repo = root.join("repo");
    fs::create_dir_all(&repo).expect("a directory");
    fs::write(root.join("protocol.yaml"), EDIT_AND_TEST).expect("the protocol");
    fs::write(repo.join("README.md"), "a repository\n").expect("a file");
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "start"]);
    let start = git(&repo, &["rev-parse", "HEAD"]);
    Scratch { root, repo, start }
}

fn options(scratch: &Scratch, case: &str, iterations: u64) -> RunOptions {
    RunOptions {
        protocol: scratch.root.join("protocol.yaml").display().to_string(),
        intent: "make the test pass".to_owned(),
        inputs: Vec::new(),
        iterations: Some(iterations),
        hours: None,
        tokens: None,
        case_id: Some(case.to_owned()),
        gate: None,
    }
}

fn validators() -> Vec<Box<dyn Validator>> {
    // `done.txt` read through `HEAD`, never named as a path: a path the test command names is on
    // the judge surface, which a step may not write.
    vec![Box::new(
        TestsRun::new("git cat-file -e HEAD:done.txt").expect("a command"),
    )]
}

/// The list a run file at `path` holding `text` gives, as `takt run --config` reads it.
fn list_of(path: &Path, text: &str) -> DeploymentPaths {
    fs::write(path, text).expect("the run file");
    RunConfig::read(path)
        .expect("the run file reads")
        .deployment_paths
        .expect("the run file names the deployment's paths")
}

/// The paths the commits after `start` changed, one a line.
fn committed_since(tree: &Path, start: &str) -> String {
    let range = format!("{start}..HEAD");
    git(tree, &["log", "--name-only", "--format=", &range])
}

/// Appends `text` to the file at `path`, making its directory.
fn append(path: &Path, text: &str) {
    use std::io::Write as _;
    fs::create_dir_all(path.parent().expect("a parent")).expect("a directory");
    fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .and_then(|mut file| file.write_all(text.as_bytes()))
        .expect("an append");
}

/// Every file of the case's records that names `needle`, by path.
fn records_naming(case_dir: &Path, needle: &str) -> Vec<PathBuf> {
    fn walk(dir: &Path, needle: &str, found: &mut Vec<PathBuf>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, needle, found);
            } else if fs::read(&path)
                .is_ok_and(|bytes| String::from_utf8_lossy(&bytes).contains(needle))
            {
                found.push(path);
            }
        }
    }
    let mut found = Vec::new();
    walk(case_dir, needle, &mut found);
    found
}

/// A harness whose step appends a line to the deployment's log and, unless `log_only`, writes
/// `done.txt`, which makes the test pass.
struct Logs {
    log_only: bool,
}

impl Harness for Logs {
    fn name(&self) -> &str {
        "logs"
    }

    fn step(&self, context: &StepContext<'_>) -> Result<StepReport, anyhow::Error> {
        append(&context.tree.join(LOG), "a line of the deployment's log\n");
        if !self.log_only {
            fs::write(context.tree.join("done.txt"), "done\n").expect("a write");
        }
        Ok(StepReport {
            action: "implementation.edit".to_owned(),
            summary: "made the test pass".to_owned(),
            tokens: Some(1),
        })
    }
}

/// Point 1: with the run file naming `.autodev/`, a step that writes `.autodev/run-2.log`
/// beside its own change is accepted; the log is in no step commit and no record, and it is in
/// the tree as the step left it, untracked, through the `tests.run` that follows.
#[test]
fn a_step_that_writes_the_deployments_log_beside_its_change_is_accepted() {
    let scratch = repository("beside");
    let tree = &scratch.repo;
    let list = list_of(
        &scratch.root.join("run.yaml"),
        "deployment_paths:\n  - .autodev/\n",
    );
    let validators = validators();
    let harness = Logs { log_only: false };
    let mut run =
        Run::open(tree, options(&scratch, "beside", 4), &harness, &validators).expect("opens");
    run.declare_deployment_paths(list);
    run.continue_past_red_base();
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let text = String::from_utf8_lossy(&out).into_owned();
    assert!(
        matches!(&stop, Ok(Stop::Completed(outcome)) if outcome == "accepted"),
        "the step that wrote {LOG} beside done.txt was not accepted: {stop:?}\n{text}"
    );
    let committed = committed_since(tree, &scratch.start);
    assert!(
        committed.lines().any(|path| path == "done.txt"),
        "the step's own change was committed: {committed}\n{text}"
    );
    let everything = git(tree, &["log", "--all", "--name-only", "--format="]);
    assert!(
        !everything.contains("run-2.log"),
        "a commit carries {LOG}: {everything}\n{text}"
    );
    assert_eq!(
        fs::read_to_string(tree.join(LOG)).expect("the log is there"),
        "a line of the deployment's log\n",
        "{text}"
    );
    assert_eq!(
        git(
            tree,
            &[
                "status",
                "--porcelain=v1",
                "--untracked-files=all",
                "--",
                ".autodev"
            ]
        ),
        format!("?? {LOG}"),
        "{text}"
    );
    let naming = records_naming(&run.case.dir, "run-2.log");
    assert!(
        naming.is_empty(),
        "the case records {LOG} as a change: {naming:?}\n{text}"
    );
}

/// Point 1, the step's only new file: it records no change for the log and is not refused.
#[test]
fn a_step_whose_only_new_file_is_the_deployments_log_is_not_refused() {
    let scratch = repository("only");
    let tree = &scratch.repo;
    let list = list_of(
        &scratch.root.join("run.toml"),
        "deployment_paths = [\".autodev/\"]\n",
    );
    let validators = validators();
    let harness = Logs { log_only: true };
    let mut run =
        Run::open(tree, options(&scratch, "only", 4), &harness, &validators).expect("opens");
    run.declare_deployment_paths(list);
    run.continue_past_red_base();
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let text = String::from_utf8_lossy(&out).into_owned();
    assert!(
        matches!(&stop, Ok(Stop::NoProgress)),
        "a step that wrote only {LOG} changed no file of the step's: {stop:?}\n{text}"
    );
    assert!(!text.contains("run-2.log"), "{text}");
    assert!(
        records_naming(&run.case.dir, "run-2.log").is_empty(),
        "the case records {LOG}\n{text}"
    );
    assert_eq!(
        fs::read_to_string(tree.join(LOG)).expect("the log is there"),
        "a line of the deployment's log\n"
    );
}

/// Point 1 as trial run 2 had it: `.autodev/` also listed in `.git/info/exclude` and the log
/// there before the run, so the step rewrites a file inside an ignored directory. Named in the
/// run file, it is neither refused nor set aside, and keeps what the step appended.
#[test]
fn the_deployments_log_inside_an_excluded_directory_is_neither_refused_nor_set_aside() {
    let scratch = repository("excluded");
    let tree = &scratch.repo;
    append(&tree.join(".git/info/exclude"), ".autodev/\n");
    append(&tree.join(LOG), "before the run\n");
    let list = list_of(
        &scratch.root.join("run.yaml"),
        "deployment_paths: [.autodev/]\n",
    );
    let validators = validators();
    let harness = Logs { log_only: false };
    let mut run = Run::open(
        tree,
        options(&scratch, "excluded", 4),
        &harness,
        &validators,
    )
    .expect("opens");
    let aside = scratch.root.join("aside");
    run.set_aside_in(aside.clone()).expect("a set-aside root");
    run.declare_deployment_paths(list);
    run.continue_past_red_base();
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let text = String::from_utf8_lossy(&out).into_owned();
    assert!(
        matches!(&stop, Ok(Stop::Completed(outcome)) if outcome == "accepted"),
        "the step that appended to {LOG} was not accepted: {stop:?}\n{text}"
    );
    assert_eq!(
        fs::read_to_string(tree.join(LOG)).expect("the log is there"),
        "before the run\na line of the deployment's log\n",
        "{text}"
    );
    assert!(
        !aside.join("excluded").exists(),
        "something was set aside: {}\n{text}",
        aside.display()
    );
    assert!(
        records_naming(&run.case.dir, "run-2.log").is_empty(),
        "the case records {LOG}\n{text}"
    );
}

/// A harness whose first step rewrites the run file to name `extra/` as the deployment's too
/// and writes `first.txt`; whose second writes `extra/new.txt` and `done.txt`.
struct RewritesTheRunFile {
    run_file: PathBuf,
    steps: Cell<u32>,
}

impl Harness for RewritesTheRunFile {
    fn name(&self) -> &str {
        "rewrites-the-run-file"
    }

    fn step(&self, context: &StepContext<'_>) -> Result<StepReport, anyhow::Error> {
        self.steps.set(self.steps.get() + 1);
        if self.steps.get() == 1 {
            fs::write(
                &self.run_file,
                "deployment_paths:\n  - .autodev/\n  - extra/\n",
            )
            .expect("a write");
            fs::write(context.tree.join("first.txt"), "first\n").expect("a write");
        } else {
            fs::create_dir_all(context.tree.join("extra")).expect("a directory");
            fs::write(context.tree.join("extra/new.txt"), "new\n").expect("a write");
            fs::write(context.tree.join("done.txt"), "done\n").expect("a write");
        }
        Ok(StepReport {
            action: "implementation.edit".to_owned(),
            summary: "edited".to_owned(),
            tokens: Some(1),
        })
    }
}

/// Point 2, the run file: a step that rewrites it to name `extra/` too moves the list neither for
/// itself nor for the step after it: `extra/new.txt` is the later step's change and is committed
/// with it.
#[test]
fn a_step_that_rewrites_the_run_file_does_not_move_the_list() {
    let scratch = repository("run-file");
    let tree = &scratch.repo;
    let run_file = scratch.root.join("run.yaml");
    let list = list_of(&run_file, "deployment_paths:\n  - .autodev/\n");
    let validators = validators();
    let harness = RewritesTheRunFile {
        run_file: run_file.clone(),
        steps: Cell::new(0),
    };
    let mut run = Run::open(
        tree,
        options(&scratch, "run-file", 6),
        &harness,
        &validators,
    )
    .expect("opens");
    run.declare_deployment_paths(list);
    run.continue_past_red_base();
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let text = String::from_utf8_lossy(&out).into_owned();
    assert_eq!(harness.steps.get(), 2, "{stop:?}\n{text}");
    assert!(
        matches!(&stop, Ok(Stop::Completed(outcome)) if outcome == "accepted"),
        "{stop:?}\n{text}"
    );
    let committed = committed_since(tree, &scratch.start);
    for path in ["first.txt", "extra/new.txt", "done.txt"] {
        assert!(
            committed.lines().any(|line| line == path),
            "{path} is a step's change and was not committed: {committed}\n{text}"
        );
    }
    assert_eq!(
        fs::read_to_string(&run_file).expect("the run file"),
        "deployment_paths:\n  - .autodev/\n  - extra/\n",
        "the step rewrote the run file"
    );
}

/// Point 2, the run file inside the tree: a list that covers the run file itself is refused,
/// since a step could rewrite it there, unseen by the guard, for the next run that reads it. A
/// list that does not cover it, and a run file outside the tree, are taken.
#[test]
fn a_list_that_covers_the_run_file_inside_the_tree_is_refused() {
    let scratch = repository("covers-run-file");
    let tree = &scratch.repo;
    fs::create_dir_all(tree.join(".autodev")).expect("a directory");
    let inside = tree.join(".autodev/run.yaml");
    fs::write(&inside, "deployment_paths:\n  - .autodev/\n").expect("the run file");
    let config = RunConfig::read(&inside).expect("the run file reads");
    let error = config
        .check_run_file_outside_deployment(&inside, tree)
        .expect_err("the list covers the run file");
    let message = format!("{error:#}");
    assert!(
        message.contains("deployment_paths") && message.contains(".autodev/run.yaml"),
        "{message}"
    );
    fs::write(&inside, "deployment_paths:\n  - .autodev/logs/\n").expect("the run file");
    let config = RunConfig::read(&inside).expect("the run file reads");
    config
        .check_run_file_outside_deployment(&inside, tree)
        .expect("the list does not cover the run file");
    let outside = scratch.root.join("run.yaml");
    fs::write(&outside, "deployment_paths:\n  - .autodev/\n").expect("the run file");
    let config = RunConfig::read(&outside).expect("the run file reads");
    config
        .check_run_file_outside_deployment(&outside, tree)
        .expect("the run file is outside the tree");
}

/// A harness whose step lists `extra/` in `.git/info/exclude` and writes `extra/new.txt` and
/// `done.txt`.
struct WritesTheExcludes;

impl Harness for WritesTheExcludes {
    fn name(&self) -> &str {
        "writes-the-excludes"
    }

    fn step(&self, context: &StepContext<'_>) -> Result<StepReport, anyhow::Error> {
        append(&context.tree.join(".git/info/exclude"), "extra/\n");
        fs::create_dir_all(context.tree.join("extra")).expect("a directory");
        fs::write(context.tree.join("extra/new.txt"), "new\n").expect("a write");
        fs::write(context.tree.join("done.txt"), "done\n").expect("a write");
        Ok(StepReport {
            action: "implementation.edit".to_owned(),
            summary: "edited".to_owned(),
            tokens: Some(1),
        })
    }
}

/// Point 2, `.git/info/exclude`: a step that adds `extra/` there is refused for it, and what it
/// wrote under `extra/` is discarded with the rest of its change; the exclude file is as it was,
/// and the deployment's log, which the step did not write, is untouched.
#[test]
fn a_step_that_writes_the_excludes_file_does_not_move_the_list() {
    let scratch = repository("info-exclude");
    let tree = &scratch.repo;
    let exclude = tree.join(".git/info/exclude");
    let before = fs::read_to_string(&exclude).unwrap_or_default();
    let list = list_of(
        &scratch.root.join("run.yaml"),
        "deployment_paths:\n  - .autodev/\n",
    );
    let validators = validators();
    let mut run = Run::open(
        tree,
        options(&scratch, "info-exclude", 4),
        &WritesTheExcludes,
        &validators,
    )
    .expect("opens");
    run.declare_deployment_paths(list);
    run.continue_past_red_base();
    append(&tree.join(LOG), "the deployment's own line\n");
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let text = String::from_utf8_lossy(&out).into_owned();
    assert!(
        matches!(&stop, Ok(Stop::StepRefused(why)) if why.contains("info/exclude")),
        "the step that wrote .git/info/exclude was not refused naming it: {stop:?}\n{text}"
    );
    assert_eq!(fs::read_to_string(&exclude).unwrap_or_default(), before);
    assert!(
        !tree.join("extra/new.txt").exists() && !tree.join("done.txt").exists(),
        "the step's change was not discarded\n{text}"
    );
    assert_eq!(
        fs::read_to_string(tree.join(LOG)).expect("the log is there"),
        "the deployment's own line\n"
    );
}

/// A harness whose step writes the deployment's log, a file under `cache/`, which
/// `.git/info/exclude` lists, a `scratch.tmp`, which the repository's `core.excludesFile`
/// lists, and `done.txt`.
struct WritesExcludedFiles;

impl Harness for WritesExcludedFiles {
    fn name(&self) -> &str {
        "writes-excluded-files"
    }

    fn step(&self, context: &StepContext<'_>) -> Result<StepReport, anyhow::Error> {
        append(&context.tree.join(LOG), "a line of the deployment's log\n");
        fs::create_dir_all(context.tree.join("cache")).expect("a directory");
        fs::write(context.tree.join("cache/new.txt"), "cached\n").expect("a write");
        fs::write(context.tree.join("scratch.tmp"), "scratch\n").expect("a write");
        fs::write(context.tree.join("done.txt"), "done\n").expect("a write");
        Ok(StepReport {
            action: "implementation.edit".to_owned(),
            summary: "edited".to_owned(),
            tokens: Some(1),
        })
    }
}

/// Point 3: a file git ignores only through `.git/info/exclude` or a `core.excludesFile`, and
/// not named in the run file, stays a change: the step is refused naming each, as a write git
/// ignores, and its change is discarded; the deployment's log it wrote beside them is neither
/// named nor reverted. (Once takt's git reads no user excludes, story:git-reads-no-user-excludes,
/// each is an untracked file of the step's and still a change: committed with the step.)
#[test]
fn a_file_excluded_only_by_the_excludes_files_stays_a_change() {
    let scratch = repository("excludes");
    let tree = &scratch.repo;
    append(&tree.join(".git/info/exclude"), "cache/\n");
    let global = scratch.root.join("global-ignore");
    fs::write(&global, "*.tmp\n").expect("an excludes file");
    git(
        tree,
        &["config", "core.excludesFile", &global.display().to_string()],
    );
    assert_eq!(
        git(
            tree,
            &["check-ignore", "--no-index", "cache/new.txt", "scratch.tmp"]
        ),
        "cache/new.txt\nscratch.tmp",
        "git ignores both through the excludes files"
    );
    let list = list_of(
        &scratch.root.join("run.yaml"),
        "deployment_paths:\n  - .autodev/\n",
    );
    let validators = validators();
    let mut run = Run::open(
        tree,
        options(&scratch, "excludes", 4),
        &WritesExcludedFiles,
        &validators,
    )
    .expect("opens");
    run.set_aside_in(scratch.root.join("aside"))
        .expect("a set-aside root");
    run.declare_deployment_paths(list);
    run.continue_past_red_base();
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let text = String::from_utf8_lossy(&out).into_owned();
    match &stop {
        Ok(Stop::StepRefused(why)) => {
            assert!(
                why.contains("cache/") && why.contains("scratch.tmp"),
                "the refusal does not name both excluded files: {why}"
            );
            assert!(!why.contains("run-2.log"), "{why}");
            for path in ["cache/new.txt", "scratch.tmp", "done.txt"] {
                assert!(
                    !tree.join(path).exists(),
                    "{path} is still there after the step was discarded\n{text}"
                );
            }
        }
        Ok(Stop::Completed(_)) => {
            let committed = committed_since(tree, &scratch.start);
            for path in ["cache/new.txt", "scratch.tmp", "done.txt"] {
                assert!(
                    committed.lines().any(|line| line == path),
                    "{path} is the step's change and was neither refused nor committed: {committed}\n{text}"
                );
            }
        }
        _ => panic!(
            "the step that wrote excluded files counted neither as a change: {stop:?}\n{text}"
        ),
    }
    assert_eq!(
        fs::read_to_string(tree.join(LOG)).expect("the log is there"),
        "a line of the deployment's log\n",
        "{text}"
    );
}

/// The list is refused, naming the entry, when an entry is absolute, holds a `..` or `.` part
/// or an empty one, is empty, or names the repository's `.git`.
#[test]
fn an_entry_that_is_no_plain_path_of_the_tree_is_refused_naming_it() {
    let scratch = repository("refused");
    let path = scratch.root.join("run.yaml");
    for entry in [
        "/etc/",
        "../outside/",
        "a/../b",
        "./a",
        "a//b",
        "",
        ".git/",
        ".git/info/exclude",
        "sub/.GIT/x",
    ] {
        fs::write(&path, format!("deployment_paths:\n  - \"{entry}\"\n")).expect("a file");
        let error = RunConfig::read(&path).expect_err(entry);
        let message = format!("{error:#}");
        assert!(
            message.contains("deployment_paths") && message.contains(&format!("{entry:?}")),
            "the refusal of {entry:?} does not name the key and the entry: {message}"
        );
    }
    for entry in [
        ".autodev/",
        ".autodev/run-2.log",
        "logs/a b.txt",
        ".gitignored/",
    ] {
        fs::write(&path, format!("deployment_paths:\n  - \"{entry}\"\n")).expect("a file");
        let read = RunConfig::read(&path);
        assert!(read.is_ok(), "{entry:?} is refused: {read:?}");
    }
}

/// A harness whose one step proposes its change for takt to write.
struct Proposing(RefCell<Option<Proposal>>);

impl Harness for Proposing {
    fn name(&self) -> &str {
        "proposing"
    }

    fn step(&self, _: &StepContext<'_>) -> Result<StepReport, anyhow::Error> {
        let proposal = self.0.borrow().clone().expect("one step");
        Ok(StepReport {
            action: proposal.action.clone(),
            summary: proposal.summary.clone(),
            tokens: None,
        })
    }

    fn take_proposal(&self) -> Option<Proposal> {
        self.0.borrow_mut().take()
    }
}

/// A proposal is the step's change, which no step commit of a deployment's path carries: one
/// that names such a path is refused, and takt writes none of it; the deployment's log is as the
/// deployment left it.
#[test]
fn a_proposal_of_a_deployments_path_is_refused_and_nothing_written() {
    let scratch = repository("proposal");
    let tree = &scratch.repo;
    let list = list_of(
        &scratch.root.join("run.yaml"),
        "deployment_paths:\n  - .autodev/\n",
    );
    let harness = Proposing(RefCell::new(Some(Proposal {
        action: "implementation.edit".to_owned(),
        summary: "scripted".to_owned(),
        files: vec![
            ProposedFile {
                path: "done.txt".to_owned(),
                content: Some("done\n".to_owned()),
            },
            ProposedFile {
                path: LOG.to_owned(),
                content: Some("rewritten by the step\n".to_owned()),
            },
        ],
    })));
    let validators = validators();
    let mut run = Run::open(
        tree,
        options(&scratch, "proposal", 4),
        &harness,
        &validators,
    )
    .expect("opens");
    run.declare_deployment_paths(list);
    run.continue_past_red_base();
    append(&tree.join(LOG), "the deployment's own line\n");
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let text = String::from_utf8_lossy(&out).into_owned();
    assert!(
        matches!(&stop, Ok(Stop::StepRefused(why)) if why.contains(LOG) && why.contains("deployment's")),
        "the proposal of {LOG} was not refused naming it: {stop:?}\n{text}"
    );
    assert!(!tree.join("done.txt").exists(), "{text}");
    assert_eq!(
        fs::read_to_string(tree.join(LOG)).expect("the log is there"),
        "the deployment's own line\n",
        "{text}"
    );
}

/// The deployment's log of an earlier run, in the tree, untracked, before this run opens.
const EARLIER_LOG: &str = ".autodev/run-1.log";

/// What the wrapper wrote into [`EARLIER_LOG`] before the run opened.
const EARLIER_LINE: &str = "the wrapper's line before the run\n";

/// A store double with one ready story, `story:x`: its take writes the story's artifact in the
/// planning store, or, when `refuses`, is refused with nothing written.
struct Store {
    refuses: bool,
}

impl Frontier for Store {
    fn name(&self) -> &str {
        "store"
    }
    fn candidates(&self, _: &Path) -> anyhow::Result<Vec<String>> {
        Ok(vec!["story:x".to_owned()])
    }
    fn active(&self, _: &Path) -> anyhow::Result<Vec<String>> {
        Ok(Vec::new())
    }
    fn show(&self, _: &Path, id: &str) -> anyhow::Result<Story> {
        Ok(Story {
            id: id.to_owned(),
            title: "make the test pass".to_owned(),
            body: "make the test pass".to_owned(),
        })
    }
    fn take(&self, repo: &Path, _: &str) -> anyhow::Result<Take> {
        if self.refuses {
            return Ok(Take::Refused("the story is not ready".to_owned()));
        }
        let story = repo.join(STORY_FILE);
        fs::create_dir_all(story.parent().expect("a parent"))?;
        fs::write(story, "status: active\n")?;
        Ok(Take::Taken)
    }
    fn record_approval(&self, _: &Path, _: &str, _: &Approval) -> anyhow::Result<()> {
        Ok(())
    }
}

/// The artifact of `story:x` in the planning store.
const STORY_FILE: &str = ".engineering/planning/story/x.md";

/// Every path any commit of any ref of `tree` changed, one a line.
fn ever_committed(tree: &Path) -> String {
    git(tree, &["log", "--all", "--name-only", "--format="])
}

/// With `.autodev/run-1.log` in the tree, untracked, before the run opens and the run file
/// naming `.autodev/`: the case opens, the story is taken and its step accepted, and no commit
/// of any ref names either log; the log is where the wrapper left it. Taking again from the
/// case's branch puts the tree back on its base with the log still there.
#[test]
fn a_deployments_log_there_before_the_run_opens_is_in_no_commit_of_the_take_or_the_steps() {
    let scratch = repository("before");
    let tree = &scratch.repo;
    append(&tree.join(EARLIER_LOG), EARLIER_LINE);
    let list = list_of(
        &scratch.root.join("run.yaml"),
        "deployment_paths:\n  - .autodev/\n",
    );
    let validators = validators();
    let harness = Logs { log_only: false };
    let store = Store { refuses: false };
    let mut out = Vec::new();
    let taken = Run::take_next_owning(
        tree,
        options(&scratch, "before", 4),
        &store,
        &CommitAs::Takt,
        None,
        &list,
        &harness,
        &validators,
        &mut out,
    );
    let text = String::from_utf8_lossy(&out).into_owned();
    let mut run = match taken {
        Ok(Some(run)) => run,
        other => panic!(
            "the story was not taken with {EARLIER_LOG} in the tree: {:?}\n{text}",
            other.map(|run| run.is_some())
        ),
    };
    run.continue_past_red_base();
    let stop = run.run(&mut out);
    let text = String::from_utf8_lossy(&out).into_owned();
    assert!(
        matches!(&stop, Ok(Stop::Completed(outcome)) if outcome == "accepted"),
        "the taken story's step was not accepted: {stop:?}\n{text}"
    );
    let committed = committed_since(tree, &scratch.start);
    assert!(
        committed.lines().any(|path| path == STORY_FILE)
            && committed.lines().any(|path| path == "done.txt"),
        "the take and the step were committed: {committed}\n{text}"
    );
    let everything = ever_committed(tree);
    assert!(
        !everything.contains("run-1.log") && !everything.contains("run-2.log"),
        "a commit carries a deployment's log: {everything}\n{text}"
    );
    assert_eq!(
        fs::read_to_string(tree.join(EARLIER_LOG)).expect("the earlier log is there"),
        EARLIER_LINE,
        "{text}"
    );
    assert_eq!(
        git(tree, &["branch", "--show-current"]),
        "takt/before",
        "{text}"
    );
    drop(run);

    // From the case's branch the next take reads the store at the base: the log keeps the
    // tree from neither.
    let mut out = Vec::new();
    let again = Run::take_next_owning(
        tree,
        options(&scratch, "again", 4),
        &store,
        &CommitAs::Takt,
        None,
        &list,
        &harness,
        &validators,
        &mut out,
    );
    let text = String::from_utf8_lossy(&out).into_owned();
    assert!(
        matches!(&again, Ok(None)),
        "the story has its case already: {:?}\n{text}",
        again.map(|run| run.is_some())
    );
    assert_eq!(git(tree, &["branch", "--show-current"]), "main", "{text}");
    assert_eq!(
        fs::read_to_string(tree.join(EARLIER_LOG)).expect("the earlier log is there"),
        EARLIER_LINE,
        "{text}"
    );
    assert!(!ever_committed(tree).contains("run-1.log"), "{text}");
}

/// A take the store refuses removes the case it opened, and the deployment's log that was in
/// the tree before stays: the case's discard keeps the run file's paths.
#[test]
fn a_take_the_store_refuses_leaves_the_deployments_log_where_it_was() {
    let scratch = repository("refused-take");
    let tree = &scratch.repo;
    append(&tree.join(EARLIER_LOG), EARLIER_LINE);
    let list = list_of(
        &scratch.root.join("run.yaml"),
        "deployment_paths:\n  - .autodev/run-1.log\n",
    );
    let validators = validators();
    let harness = Logs { log_only: false };
    let mut out = Vec::new();
    let taken = Run::take_next_owning(
        tree,
        options(&scratch, "refused-take", 4),
        &Store { refuses: true },
        &CommitAs::Takt,
        None,
        &list,
        &harness,
        &validators,
        &mut out,
    );
    let text = String::from_utf8_lossy(&out).into_owned();
    assert!(
        matches!(&taken, Ok(None)),
        "the store refused the take: {:?}\n{text}",
        taken.map(|run| run.is_some())
    );
    assert!(text.contains("not taken: the story is not ready"), "{text}");
    assert_eq!(git(tree, &["branch", "--show-current"]), "main", "{text}");
    assert_eq!(git(tree, &["branch", "--list", "takt/*"]), "", "{text}");
    assert_eq!(
        fs::read_to_string(tree.join(EARLIER_LOG)).expect("the earlier log is there"),
        EARLIER_LINE,
        "{text}"
    );
}

/// A store write on the case's branch with the deployment's log in the tree: the write is
/// committed without it, and a write the store refuses after changing a file is discarded
/// without it.
#[test]
fn a_store_write_on_the_case_branch_neither_commits_nor_discards_the_deployments_log() {
    let scratch = repository("store-write");
    let tree = &scratch.repo;
    git(tree, &["checkout", "-q", "-b", "work"]);
    append(&tree.join(EARLIER_LOG), EARLIER_LINE);
    let list = list_of(
        &scratch.root.join("run.yaml"),
        "deployment_paths:\n  - .autodev/\n",
    );
    let identity = takt::git::CommitIdentity::takt();
    let written = takt::frontier::write_on_case_branch(
        tree,
        "work",
        &identity,
        "story:x",
        "takt: test_result story:x",
        &list,
        || {
            let story = tree.join(STORY_FILE);
            fs::create_dir_all(story.parent().expect("a parent"))?;
            fs::write(story, "status: implemented\n")?;
            Ok(Written::Done)
        },
        |_| Ok(()),
    );
    assert!(
        matches!(&written, Ok(Ok(Some(_)))),
        "the store's write was committed: {written:?}"
    );
    assert_eq!(committed_since(tree, &scratch.start), STORY_FILE);

    let refused = takt::frontier::write_on_case_branch(
        tree,
        "work",
        &identity,
        "story:x",
        "takt: implemented story:x",
        &list,
        || {
            fs::write(tree.join(STORY_FILE), "half a move\n")?;
            Ok(Written::Refused("the move is refused".to_owned()))
        },
        |_| Ok(()),
    );
    let error = match refused {
        Err(error) => format!("{error:#}"),
        Ok(answer) => panic!("a refused write that changed a file is discarded: {answer:?}"),
    };
    assert!(error.contains("what it changed was discarded"), "{error}");
    assert_eq!(
        fs::read_to_string(tree.join(STORY_FILE)).expect("the story"),
        "status: implemented\n"
    );
    assert_eq!(
        fs::read_to_string(tree.join(EARLIER_LOG)).expect("the earlier log is there"),
        EARLIER_LINE
    );
    assert!(!ever_committed(tree).contains("run-1.log"));
}

/// A link a step puts at or inside a path of the list is the step's change, so putting the
/// tree back removes it, as itself and never followed; and an entry without `/` keeps no
/// directory of its name from being put back. The deployment's own file beside the link stays.
#[test]
#[cfg(unix)]
fn putting_a_step_back_removes_a_link_it_put_at_a_deployments_path() {
    let scratch = repository("link-restore");
    let tree = &scratch.repo;
    append(&tree.join(LOG), "the wrapper, before the step\n");
    let own =
        DeploymentPaths::new(vec![".autodev/".to_owned(), "logs".to_owned()]).expect("the list");
    let before = takt::git::Checkpoint::take_owning(tree, &[], &own).expect("a checkpoint");
    std::os::unix::fs::symlink(&scratch.root, tree.join(".autodev/out")).expect("a link");
    append(&tree.join("logs/new.txt"), "the step's\n");
    let change = before.change(tree).expect("the change");
    assert!(
        change.paths.iter().any(|path| path == ".autodev/out")
            && change.paths.iter().any(|path| path == "logs/new.txt"),
        "{:?}",
        change.paths
    );
    before.restore(tree).expect("the tree is put back");
    assert!(
        fs::symlink_metadata(tree.join(".autodev/out")).is_err(),
        "the link is removed"
    );
    assert!(
        !tree.join("logs").exists(),
        "the step's directory is removed"
    );
    assert!(
        scratch.root.join("repo").exists(),
        "the link's target is kept"
    );
    assert_eq!(
        fs::read_to_string(tree.join(LOG)).expect("the deployment's log"),
        "the wrapper, before the step\n"
    );
}

/// A harness whose edit step replaces the deployment's directory `.autodev/` with a link to
/// `src` and writes `done.txt`, which makes the test pass.
struct LinksTheDeploymentsDirectory;

impl Harness for LinksTheDeploymentsDirectory {
    fn name(&self) -> &str {
        "links"
    }

    fn step(&self, context: &StepContext<'_>) -> Result<StepReport, anyhow::Error> {
        let at = context.tree.join(".autodev");
        if fs::symlink_metadata(&at).is_ok_and(|meta| meta.is_dir()) {
            fs::remove_dir_all(&at).expect("remove the deployment's directory");
        }
        std::os::unix::fs::symlink("src", &at).expect("a link");
        fs::write(context.tree.join("done.txt"), "done\n").expect("a write");
        Ok(StepReport {
            action: "implementation.edit".to_owned(),
            summary: "made the test pass".to_owned(),
            tokens: Some(1),
        })
    }
}

/// An edit step that replaces `.autodev/`, the deployment's, with a link to `src` is refused
/// and put back, naming the path: no commit of any ref names `.autodev`, and no link is left in
/// the tree, so what the deployment writes there later lands in no commit.
#[test]
#[cfg(unix)]
fn an_edit_step_that_puts_a_link_at_the_deployments_directory_is_refused_and_put_back() {
    let scratch = repository("edit-link");
    let tree = &scratch.repo;
    fs::create_dir_all(tree.join("src")).expect("a directory");
    fs::write(tree.join("src/lib.rs"), "pub fn f() {}\n").expect("a file");
    git(tree, &["add", "-A"]);
    git(tree, &["commit", "-q", "-m", "sources"]);
    let list = list_of(
        &scratch.root.join("run.yaml"),
        "deployment_paths:\n  - .autodev/\n",
    );
    let validators = validators();
    let harness = LinksTheDeploymentsDirectory;
    let mut run = Run::open(
        tree,
        options(&scratch, "edit-link", 4),
        &harness,
        &validators,
    )
    .expect("opens");
    run.declare_deployment_paths(list);
    run.continue_past_red_base();
    append(&tree.join(EARLIER_LOG), EARLIER_LINE);
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let text = String::from_utf8_lossy(&out).into_owned();
    assert!(
        matches!(&stop, Ok(Stop::StepRefused(why)) if why.contains(".autodev")),
        "the step that put a link at .autodev was not refused naming it: {stop:?}\n{text}"
    );
    let everything = ever_committed(tree);
    assert!(
        !everything.lines().any(|path| path.starts_with(".autodev")),
        "a commit names .autodev: {everything}\n{text}"
    );
    assert!(
        !fs::symlink_metadata(tree.join(".autodev"))
            .is_ok_and(|meta| meta.file_type().is_symlink()),
        "the link is left in the tree\n{text}"
    );
    append(&tree.join(LOG), "the wrapper, after the step\n");
    assert_eq!(
        fs::read_to_string(tree.join("src/lib.rs")).expect("the source"),
        "pub fn f() {}\n"
    );
    assert!(
        !tree.join("src/run-2.log").exists(),
        "the deployment's write lands in src/\n{text}"
    );
}

/// The links a step's change names as the deployment's ([`takt::git::StepChange`]): one at a
/// path an entry covers, one inside a covered directory, one at a directory above an entry and
/// one at an entry without `/`, whatever git ignores; never a link elsewhere, nor a file or a
/// directory at a covered path.
#[test]
#[cfg(unix)]
fn every_link_at_or_above_a_deployments_path_is_named_and_nothing_else() {
    let scratch = repository("link-names");
    let tree = &scratch.repo;
    fs::write(tree.join(".gitignore"), "/hidden\n").expect("the ignore file");
    git(tree, &["add", "-A"]);
    git(tree, &["commit", "-q", "-m", "ignore"]);
    let own = DeploymentPaths::new(vec![
        ".autodev/".to_owned(),
        "deep/out/".to_owned(),
        "notes.txt".to_owned(),
        "hidden".to_owned(),
        "plain/".to_owned(),
    ])
    .expect("the list");
    fs::create_dir_all(tree.join(".autodev")).expect("a directory");
    let before = takt::git::Checkpoint::take_owning(tree, &[], &own).expect("a checkpoint");
    let link =
        |path: &str| std::os::unix::fs::symlink("README.md", tree.join(path)).expect("a link");
    link(".autodev/out");
    link("deep");
    link("notes.txt");
    link("hidden");
    link("elsewhere");
    append(&tree.join("plain/file.txt"), "the deployment's\n");
    let change = before.change(tree).expect("the change");
    assert_eq!(
        change.deployment_links,
        [".autodev/out", "deep", "hidden", "notes.txt"],
        "paths {:?}, ignored_added {:?}",
        change.paths,
        change.ignored_added
    );
}
