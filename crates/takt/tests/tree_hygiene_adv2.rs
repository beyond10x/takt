//! Adversary pass 2 against tree hygiene, after correction 1: where the narrowed `.git/` guard,
//! the opaque-entry comparison and the put-back that goes on past errors still leave a step's
//! change behind. Each case asserts what the story's acceptance asks: a step that writes under
//! `.git/` or rewrites an ignored file is refused and leaves no trace of its change.

#[path = "support/own_home.rs"]
mod own_home;

use std::fs;
use std::io::{Seek as _, SeekFrom, Write as _};
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use takt::harness::{Harness, StepContext, StepReport};
use takt::validator::{TestsRun, Validator};
use takt::{Run, RunOptions, Stop};

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
    String::from_utf8_lossy(&output.stdout)
        .trim_end()
        .to_owned()
}

/// A scratch root under this crate's build directory; dropping it removes it.
struct Scratch {
    root: PathBuf,
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = Command::new("chmod")
            .args(["-R", "u+rwX"])
            .arg(&self.root)
            .status();
        if let Err(error) = fs::remove_dir_all(&self.root) {
            eprintln!("remove the scratch tree {}: {error}", self.root.display());
        }
    }
}

fn scratch(name: &str) -> Scratch {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("hygiene-adv2-{}-{name}", std::process::id()));
    if root.exists() {
        fs::remove_dir_all(&root).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(&root).expect("scratch dir");
    Scratch { root }
}

/// A committed repository at `dir` on `main`: a source file and `*.local` ignored.
fn repository(dir: &Path) {
    fs::create_dir_all(dir.join("src")).expect("src");
    fs::write(dir.join("src/lib.rs"), "a\n").expect("a file");
    fs::write(dir.join(".gitignore"), "*.local\n").expect("a file");
    git(dir, &["init", "-q", "-b", "main"]);
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", "start"]);
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

/// A harness whose every step does `act` in the tree and reports `action`.
struct Acting<F: Fn(&Path)> {
    action: &'static str,
    act: F,
}

impl<F: Fn(&Path)> Harness for Acting<F> {
    fn name(&self) -> &str {
        "acting"
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

fn tests_run(command: &str) -> Vec<Box<dyn Validator>> {
    vec![Box::new(TestsRun::new(command).expect("a command"))]
}

fn run(
    tree: &Path,
    id: &str,
    iterations: u64,
    harness: &dyn Harness,
    validators: &[Box<dyn Validator>],
) -> (anyhow::Result<Stop>, String) {
    let mut run = Run::open(tree, options(id, iterations), harness, validators).expect("opens");
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    (stop, String::from_utf8_lossy(&out).into_owned())
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

fn new_repo(scratch: &Scratch) -> PathBuf {
    let tree = scratch.root.join("repo");
    fs::create_dir_all(&tree).expect("repo");
    repository(&tree);
    tree
}

/// `HEAD` is guarded (the coordinator's F3 decision). A step that leaves `.git/HEAD` unreadable
/// as a ref makes git stop taking `.git` for a repository, and nothing puts `HEAD` back before
/// takt's next git call: git looks for a repository further up, and takt's calls land there.
/// The tree here sits inside an outer repository of its own, so that what lands up there lands
/// in the scratch directory and not in the checkout this suite is built in.
#[test]
fn a_step_that_garbles_head_is_refused_and_its_change_discarded() {
    let scratch = scratch("garbled-head");
    let outer = scratch.root.join("outer");
    fs::create_dir_all(&outer).expect("outer");
    fs::write(outer.join("outer.txt"), "an unrelated repository\n").expect("a file");
    repository(&outer);
    let outer_head = read(&outer.join(".git/HEAD"));
    let tree = outer.join("repo");
    fs::create_dir_all(&tree).expect("repo");
    repository(&tree);
    let harness = Acting {
        action: "implementation.edit",
        act: |tree: &Path| {
            fs::write(tree.join("src/lib.rs"), "model\n").expect("a write");
            fs::write(tree.join(".git/HEAD"), "not a ref\n").expect("a write");
        },
    };
    let validators = tests_run("true");
    let (stop, text) = run(&tree, "garbled-head", 2, &harness, &validators);
    let source = fs::read_to_string(tree.join("src/lib.rs")).unwrap_or_default();
    let head = fs::read_to_string(tree.join(".git/HEAD")).unwrap_or_default();
    assert_eq!(
        read(&outer.join(".git/HEAD")),
        outer_head,
        "takt's git calls after the step ran in the repository enclosing the tree and moved its HEAD; the run ended {stop:?}\n{text}"
    );
    let stop = match stop {
        Ok(stop) => stop,
        Err(error) => panic!(
            "a step that garbled HEAD ended the run with an error, not a refusal; .git/HEAD is {head:?}, src/lib.rs is {source:?}: {error:#}\n{text}"
        ),
    };
    assert!(matches!(&stop, Stop::StepRefused(_)), "{stop:?}\n{text}");
    assert_eq!(source, "a\n", "{text}");
    assert!(head.starts_with("ref: refs/heads/"), "{head:?}\n{text}");
}

/// The put-back goes on past an error, but it never makes a directory the step made read-only
/// writable again: the hook the step planted in `.git/hooks/` stays in the repository.
#[test]
fn a_hook_planted_under_a_hooks_directory_made_read_only_does_not_stay() {
    let scratch = scratch("readonly-hooks");
    let tree = new_repo(&scratch);
    let harness = Acting {
        action: "implementation.edit",
        act: |tree: &Path| {
            let hooks = tree.join(".git/hooks");
            let hook = hooks.join("pre-commit");
            fs::write(&hook, "#!/bin/sh\nexit 0\n").expect("a hook");
            fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).expect("exec bit");
            fs::set_permissions(&hooks, fs::Permissions::from_mode(0o555)).expect("read-only");
            fs::write(tree.join("src/lib.rs"), "model\n").expect("a write");
        },
    };
    let validators = tests_run("true");
    let (stop, text) = run(&tree, "readonly-hooks", 2, &harness, &validators);
    let left = tree.join(".git/hooks/pre-commit").exists();
    let stop = stop.expect("the run stops");
    assert!(
        matches!(&stop, Stop::StepRefused(why) if why.contains(".git")),
        "{stop:?}\n{text}"
    );
    assert!(
        !left,
        "the hook the refused step planted is still in the repository ({stop:?})\n{text}"
    );
}

/// An ignored file too large to keep is compared by kind, size, mode and modification time.
/// All four are the step's to set back; the change time is not, and is not compared.
#[test]
fn a_large_ignored_file_rewritten_with_its_size_and_time_kept_is_refused() {
    let scratch = scratch("opaque-rewrite");
    let tree = new_repo(&scratch);
    let big = tree.join("model.local");
    {
        let file = fs::File::create(&big).expect("a file");
        file.set_len(64 * 1024 * 1024 + 1).expect("the size");
    }
    let harness = Acting {
        action: "implementation.edit",
        act: |tree: &Path| {
            let big = tree.join("model.local");
            let modified = fs::metadata(&big)
                .and_then(|meta| meta.modified())
                .expect("a time");
            let mut file = fs::OpenOptions::new()
                .write(true)
                .open(&big)
                .expect("opens");
            file.seek(SeekFrom::Start(0)).expect("seeks");
            file.write_all(b"PASS=1\n").expect("a write");
            file.set_modified(modified).expect("the time set back");
            drop(file);
            fs::write(tree.join("src/lib.rs"), "model\n").expect("a write");
        },
    };
    let validators = tests_run("true");
    let (stop, text) = run(&tree, "opaque-rewrite", 2, &harness, &validators);
    let stop = stop.expect("the run stops");
    assert!(
        matches!(&stop, Stop::StepRefused(why) if why.contains("model.local")),
        "a rewrite of an ignored file that kept its size, mode and time was not noticed: {stop:?}\n{text}"
    );
}

/// With `extensions.worktreeConfig` on (as `git sparse-checkout` turns it on), every linked
/// worktree has a configuration file of its own under the shared git directory. Only the
/// tree's own are guarded; a sibling's is configuration git reads all the same.
#[test]
fn a_guarded_key_written_into_a_sibling_worktrees_config_is_refused_and_put_back() {
    let scratch = scratch("sibling-config");
    let tree = new_repo(&scratch);
    git(&tree, &["config", "extensions.worktreeConfig", "true"]);
    let sibling = scratch.root.join("sibling");
    git(
        &tree,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "side",
            sibling.to_str().expect("utf-8"),
        ],
    );
    let config = tree.join(".git/worktrees/sibling/config.worktree");
    let harness = Acting {
        action: "implementation.edit",
        act: |tree: &Path| {
            fs::write(
                tree.join(".git/worktrees/sibling/config.worktree"),
                "[core]\n\thooksPath = set-by-the-step\n",
            )
            .expect("a write");
            fs::write(tree.join("src/lib.rs"), "model\n").expect("a write");
        },
    };
    let validators = tests_run("true");
    let (stop, text) = run(&tree, "sibling-config", 2, &harness, &validators);
    let stop = stop.expect("the run stops");
    // Read without asserting success: once the key is put back, `--get` finds none and exits 1.
    let seen = Command::new("git")
        .args(["config", "--get", "core.hooksPath"])
        .current_dir(&sibling)
        .output()
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .unwrap_or_default();
    let _ = fs::remove_file(&config);
    assert!(
        matches!(&stop, Stop::StepRefused(why) if why.contains(".git")),
        "a guarded key written into the sibling worktree's config was not noticed; the sibling's git now reads core.hooksPath={seen}: {stop:?}\n{text}"
    );
}

/// A git process the step started and that was killed leaves `index.lock` in the tree's own git
/// directory; no other worktree's process takes that lock. The step's change is then neither
/// committed nor discarded.
#[test]
fn a_step_that_leaves_the_index_locked_leaves_no_uncommitted_change() {
    let scratch = scratch("index-lock");
    let tree = new_repo(&scratch);
    let harness = Acting {
        action: "implementation.edit",
        act: |tree: &Path| {
            fs::write(tree.join("src/lib.rs"), "model\n").expect("a write");
            fs::write(tree.join(".git/index.lock"), "").expect("a lock");
        },
    };
    let validators = tests_run("true");
    let (stop, text) = run(&tree, "index-lock", 2, &harness, &validators);
    let _ = fs::remove_file(tree.join(".git/index.lock"));
    let dirty = git(&tree, &["status", "--porcelain", "--", "src"]);
    assert!(
        dirty.is_empty(),
        "the step's change was left uncommitted in the tree: {dirty:?}; the run ended {stop:?}\n{text}"
    );
}
