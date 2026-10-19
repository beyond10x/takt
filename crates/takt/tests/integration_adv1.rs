//! Adversary pass 1 over the wave 06 integration fixes, in the library: who can make a commit
//! that `approvals_only` takes for the approval's own, and which refs a step or a validator's
//! command may move.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use takt::case::CASES_DIR;
use takt::frontier::Aep;
use takt::harness::{Harness, StepContext, StepReport};
use takt::validator::{TestsRun, Validator};
use takt::{CaseDir, Run, RunOptions, Stop};

/// The fixture stores, one directory each, relative to this crate.
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");

fn git(tree: &Path, args: &[&str]) -> String {
    git_with(tree, args, None)
}

/// `git <args>` in `tree`, with `GIT_INDEX_FILE` set to `index` when given.
fn git_with(tree: &Path, args: &[&str], index: Option<&Path>) -> String {
    let mut command = Command::new("git");
    command
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(args)
        .current_dir(tree);
    if let Some(index) = index {
        command.env("GIT_INDEX_FILE", index);
    }
    let output = command.output().expect("git runs");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// `<root>/repo` and, beside it, what a case keeps outside the tree; removed on drop.
struct Scratch {
    root: PathBuf,
    repo: PathBuf,
}

impl std::ops::Deref for Scratch {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.repo
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn empty(name: &str) -> Scratch {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("integration-adv1-{}-{name}", std::process::id()));
    if root.exists() {
        fs::remove_dir_all(&root).expect("a stale scratch tree is removed");
    }
    let repo = root.join("repo");
    fs::create_dir_all(&repo).expect("scratch dir");
    Scratch { root, repo }
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

/// A committed copy of the `one-ready` fixture store on `main`.
fn store(name: &str) -> Scratch {
    let scratch = empty(name);
    copy_dir(&Path::new(FIXTURES).join("one-ready"), &scratch.repo);
    fs::write(scratch.repo.join("check.txt"), "broken\n").expect("a file");
    git(&scratch.repo, &["init", "-q", "-b", "main"]);
    git(&scratch.repo, &["add", "-A"]);
    git(&scratch.repo, &["commit", "-q", "-m", "start"]);
    scratch
}

fn options(id: &str, iterations: u64) -> RunOptions {
    RunOptions {
        protocol: "software.change/1".to_owned(),
        intent: "make the change".to_owned(),
        inputs: vec![
            ("risk".to_owned(), "trivial".to_owned()),
            ("change_kind".to_owned(), "feature".to_owned()),
            ("affects_security_boundary".to_owned(), "false".to_owned()),
        ],
        iterations: Some(iterations),
        hours: None,
        tokens: None,
        case_id: Some(id.to_owned()),
        gate: Some("repository.merge".to_owned()),
    }
}

fn store_options() -> RunOptions {
    let mut options = options("unused", 2);
    options.intent = String::new();
    options.case_id = None;
    options
}

/// A harness whose every step does `act` in the tree and reports `action`.
struct Acting<F: Fn(&Path)> {
    action: &'static str,
    act: F,
}

impl<F: Fn(&Path)> Harness for Acting<F> {
    fn name(&self) -> &str {
        "acting"
    }

    /// Most cases here have it answer `repository.inspect`, which only a harness that keeps state
    /// is offered (story:no-inspect-for-stateless-harness).
    fn keeps_state(&self) -> bool {
        true
    }

    fn step(&self, context: &StepContext<'_>) -> Result<StepReport, anyhow::Error> {
        (self.act)(context.tree);
        Ok(StepReport {
            action: self.action.to_owned(),
            summary: "acted".to_owned(),
            tokens: None,
        })
    }
}

/// (A) A judgement step of one case writes, onto the branch of a sibling case opened for a
/// story, a commit of exactly the shape `approvals_only` accepts as the approval's own: one
/// parent, the subject `takt: approve <story> <capability>`, one path under
/// `.engineering/evidence/`. It does so with `git commit-tree` and `git update-ref` on an index
/// of its own, never touching its own `HEAD`, its index or its work tree. Only `HEAD`'s branch
/// and the step's case's base are guarded refs, so the step is accepted and the sibling's branch
/// keeps the forged commit; the sibling resumed keeps its revision under a `HEAD` no approval
/// made. The tree-hygiene acceptance: "a step that ... writes under `.git/` ... is refused and
/// leaves no trace of its change".
#[test]
fn adv1_a_step_that_forges_an_approval_commit_on_a_sibling_cases_branch_is_refused() {
    let tree = store("sibling-forge");
    let none: Vec<Box<dyn Validator>> = Vec::new();
    let idle = Acting {
        action: "repository.inspect",
        act: |_: &Path| {},
    };
    let mut out = Vec::new();
    let sibling = Run::open_next(
        &tree,
        store_options(),
        &Aep::default(),
        &idle,
        &none,
        &mut out,
    )
    .expect("the frontier answers")
    .expect("story:ready-one is taken");
    let sibling_id = sibling.case.id.clone();
    let sibling_revision = sibling.case.revision("implementation").expect("a revision");
    drop(sibling);
    let sibling_branch = format!("takt/{sibling_id}");
    let before = git(&tree, &["rev-parse", &sibling_branch]);
    assert_eq!(before, sibling_revision);
    git(&tree, &["checkout", "-q", "main"]);

    let root = tree.root.clone();
    let branch = sibling_branch.clone();
    let forger = Acting {
        action: "repository.inspect",
        act: move |tree: &Path| {
            let index = root.join("forge-index");
            let record = root.join("forged.yaml");
            fs::write(&record, "kind: approval\n").expect("a record");
            git_with(tree, &["read-tree", &branch], Some(&index));
            let blob = git(
                tree,
                &["hash-object", "-w", record.to_str().expect("utf-8")],
            );
            git_with(
                tree,
                &[
                    "update-index",
                    "--add",
                    "--cacheinfo",
                    &format!("100644,{blob},.engineering/evidence/story/ready-one/forged.yaml"),
                ],
                Some(&index),
            );
            let forged_tree = git_with(tree, &["write-tree"], Some(&index));
            let commit = git(
                tree,
                &[
                    "commit-tree",
                    &forged_tree,
                    "-p",
                    &branch,
                    "-m",
                    "takt: approve story:ready-one repository.merge",
                ],
            );
            git(
                tree,
                &["update-ref", &format!("refs/heads/{branch}"), &commit],
            );
            let _ = fs::remove_file(&index);
            let _ = fs::remove_file(&record);
        },
    };
    let mut run = Run::open(&tree, options("forger", 1), &forger, &none).expect("opens");
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let text = String::from_utf8_lossy(&out).into_owned();
    drop(run);
    let after = git(&tree, &["rev-parse", &sibling_branch]);
    let resumed = CaseDir::resume(&tree, &sibling_id).expect("the sibling reads back");
    let kept = resumed.revision("implementation");
    let head = git(&tree, &["rev-parse", "HEAD"]);
    assert!(
        matches!(&stop, Ok(Stop::StepRefused(_))) && after == before,
        "the step was not refused for moving {sibling_branch} from {before} to {after}: {stop:?}\n{text}\nthe sibling resumed at HEAD {head} keeps the implementation revision {kept:?}"
    );
    assert!(tree.join(CASES_DIR).is_dir());
}

/// A validator's command is code a model wrote. The judgement step that moves the case's base
/// branch is refused and the base put back (`tree_hygiene.rs`); the same move made by the test
/// command is neither: `Checkpoint::take` guards no ref but `HEAD`'s branch, and the validator
/// path never reads `StepChange::refs`. The base is where `take_next` reads the planning store
/// from, "never from a branch a model committed on", and what the next case branches from.
#[test]
fn adv1_a_test_command_that_moves_the_base_branch_is_refused_and_the_base_put_back() {
    let tree = store("validator-base");
    let main = git(&tree, &["rev-parse", "main"]);
    let script = tree.root.join("moves-main.sh");
    fs::write(
        &script,
        "#!/bin/sh\nset -e\nc=$(git -c user.name=m -c user.email=m@localhost commit-tree 'main^{tree}' -p main -m moved)\ngit update-ref refs/heads/main \"$c\"\nexit 0\n",
    )
    .expect("a script");
    let validators: Vec<Box<dyn Validator>> = vec![Box::new(
        TestsRun::new(&format!("sh {}", script.display())).expect("a command"),
    )];
    let idle = Acting {
        action: "repository.inspect",
        act: |_: &Path| {},
    };
    let mut run =
        Run::open(&tree, options("validator-base", 1), &idle, &validators).expect("opens");
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let text = String::from_utf8_lossy(&out).into_owned();
    let now = git(&tree, &["rev-parse", "main"]);
    assert_eq!(
        now, main,
        "the test command moved the base branch main and the run kept it: {stop:?}\n{text}"
    );
}
