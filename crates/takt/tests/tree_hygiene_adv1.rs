//! Adversary pass 1 against tree hygiene: what the `.git/` guard and the ignored-file guard do
//! at their edges. A foreign write to the shared git directory during a step (an editor, the
//! planning store's lock in another worktree) is not the step's and must survive it; a step
//! that rewrites an ignored file that already existed writes an ignored file; a step that
//! leaves something unreadable or blocking under `.git/` must still be refused with nothing it
//! planted left behind; a hook directory inside the tree must not run what a refused step
//! planted there while takt puts the tree back.

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc;
use std::time::Duration;

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
        .join(format!("hygiene-adv1-{}-{name}", std::process::id()));
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

/// The main worktree `main/` and a linked worktree `linked/` on `side`, sharing one git
/// directory, as a repository worked by several sessions at once is.
fn linked(scratch: &Scratch) -> (PathBuf, PathBuf) {
    let main = scratch.root.join("main");
    fs::create_dir_all(&main).expect("main");
    repository(&main);
    let linked = scratch.root.join("linked");
    git(
        &main,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "side",
            linked.to_str().expect("utf-8"),
        ],
    );
    (main, linked)
}

#[test]
fn an_editors_config_write_in_the_shared_git_directory_during_a_step_survives_it() {
    let scratch = scratch("foreign-config");
    let (main, linked) = linked(&scratch);
    let other = main.clone();
    // During the step, an editor watching the other worktree records a branch's merge base in
    // the shared config, as VS Code does (`branch.<name>.vscode-merge-base`).
    let harness = Acting {
        action: "implementation.edit",
        act: move |tree: &Path| {
            git(
                &other,
                &["config", "branch.main.vscode-merge-base", "origin/main"],
            );
            fs::write(tree.join("src/lib.rs"), "b\n").expect("a write");
        },
    };
    let validators = tests_run("true");
    let (stop, text) = run(&linked, "foreign-config", 2, &harness, &validators);
    let stop = stop.expect("the run stops");
    let config = read(&main.join(".git/config"));
    assert!(
        config.contains("vscode-merge-base"),
        "the other worktree's config write was reverted by the step's guard ({stop:?})\n{config}\n{text}"
    );
}

#[test]
fn a_planning_store_lock_made_in_another_worktree_during_a_step_survives_it() {
    let scratch = scratch("foreign-lock");
    let (main, linked) = linked(&scratch);
    let lock = main.join(".git/aep/planning-writer-store-0123abcd.lock");
    let made = lock.clone();
    // During the step, the planning store's writer in the other worktree takes its lock in the
    // shared git directory, as `aep` does (`.git/aep/planning-writer-*.lock`).
    let harness = Acting {
        action: "implementation.edit",
        act: move |tree: &Path| {
            fs::create_dir_all(made.parent().expect("a parent")).expect("a dir");
            fs::write(&made, "").expect("the lock");
            fs::write(tree.join("src/lib.rs"), "b\n").expect("a write");
        },
    };
    let validators = tests_run("true");
    let (stop, text) = run(&linked, "foreign-lock", 2, &harness, &validators);
    let stop = stop.expect("the run stops");
    assert!(
        lock.exists(),
        "another process's lock file was deleted by the step's guard ({stop:?})\n{text}"
    );
}

#[test]
fn a_step_that_rewrites_an_ignored_file_that_existed_is_refused_and_the_file_put_back() {
    let scratch = scratch("ignored-rewrite");
    let tree = scratch.root.join("repo");
    fs::create_dir_all(&tree).expect("repo");
    repository(&tree);
    // The operator's ignored override, which the test command may read.
    fs::write(tree.join("override.local"), "PASS=0\n").expect("a write");
    let harness = Acting {
        action: "implementation.edit",
        act: |tree: &Path| {
            fs::write(tree.join("override.local"), "PASS=1\n").expect("a write");
            fs::write(tree.join("src/lib.rs"), "b\n").expect("a write");
        },
    };
    let validators = tests_run("true");
    let (stop, text) = run(&tree, "ignored-rewrite", 2, &harness, &validators);
    let stop = stop.expect("the run stops");
    assert!(
        matches!(&stop, Stop::StepRefused(why) if why.contains("override.local")),
        "{stop:?}\n{text}"
    );
    assert_eq!(read(&tree.join("override.local")), "PASS=0\n", "{text}");
    assert_eq!(read(&tree.join("src/lib.rs")), "a\n", "{text}");
}

#[test]
fn a_step_that_leaves_an_unreadable_file_under_git_is_refused_and_nothing_it_planted_stays() {
    let scratch = scratch("dotgit-unreadable");
    let tree = scratch.root.join("repo");
    fs::create_dir_all(&tree).expect("repo");
    repository(&tree);
    let marker = scratch.root.join("planted-ran");
    let script = format!("#!/bin/sh\necho ran >> {}\n", marker.display());
    let harness = Acting {
        action: "implementation.edit",
        act: move |tree: &Path| {
            let hook = tree.join(".git/hooks/post-checkout");
            fs::write(&hook, &script).expect("a hook");
            fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).expect("exec bit");
            let blocker = tree.join(".git/info/zz-unreadable");
            fs::write(&blocker, "x").expect("a file");
            fs::set_permissions(&blocker, fs::Permissions::from_mode(0o000)).expect("mode 000");
            fs::write(tree.join("src/lib.rs"), "model\n").expect("a write");
        },
    };
    let validators = tests_run("true");
    let (stop, text) = run(&tree, "dotgit-unreadable", 2, &harness, &validators);
    let hook_left = tree.join(".git/hooks/post-checkout").exists();
    let source = read(&tree.join("src/lib.rs"));
    let stop = match stop {
        Ok(stop) => stop,
        Err(error) => panic!(
            "a write under .git ended the run with an error instead of a refusal; the planted hook is {}, src/lib.rs is {source:?}: {error:#}\n{text}",
            if hook_left { "still there" } else { "gone" }
        ),
    };
    assert!(
        matches!(&stop, Stop::StepRefused(why) if why.contains(".git")),
        "{stop:?}\n{text}"
    );
    assert!(!hook_left, "the planted hook stayed\n{text}");
    assert_eq!(source, "a\n", "{text}");
}

#[test]
fn a_step_that_leaves_a_fifo_under_git_does_not_hang_the_run() {
    let scratch = scratch("dotgit-fifo");
    let tree = scratch.root.join("repo");
    fs::create_dir_all(&tree).expect("repo");
    repository(&tree);
    let fifo = tree.join(".git/info/pipe");
    let (sent, received) = mpsc::channel();
    let worker = {
        let tree = tree.clone();
        std::thread::spawn(move || {
            let harness = Acting {
                action: "implementation.edit",
                act: |tree: &Path| {
                    let made = Command::new("mkfifo")
                        .arg(tree.join(".git/info/pipe"))
                        .status()
                        .expect("mkfifo runs");
                    assert!(made.success(), "mkfifo");
                    fs::write(tree.join("src/lib.rs"), "model\n").expect("a write");
                },
            };
            let validators = tests_run("true");
            let (stop, text) = run(&tree, "dotgit-fifo", 2, &harness, &validators);
            let _ = sent.send(format!("{stop:?}\n{text}"));
        })
    };
    match received.recv_timeout(Duration::from_secs(30)) {
        Ok(result) => {
            worker.join().expect("the run's thread ends");
            assert!(result.starts_with("Ok(StepRefused("), "{result}");
        }
        Err(_) => {
            // Unblock every read of the FIFO (it is read more than once) until the thread
            // ends, then report the hang.
            let mut unblocked = 0;
            while !worker.is_finished() && unblocked < 50 {
                if fifo.exists() {
                    let opened = Command::new("timeout")
                        .args([
                            "2",
                            "sh",
                            "-c",
                            "exec 3>\"$0\"",
                            fifo.to_str().expect("utf-8"),
                        ])
                        .status();
                    if opened.is_ok_and(|status| status.success()) {
                        unblocked += 1;
                    }
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            let _ = worker.join();
            panic!(
                "the run hung for 30s reading the FIFO a step left under .git/; it went on only after {unblocked} writer(s) opened it"
            );
        }
    }
}

#[test]
fn a_hook_a_refused_step_planted_in_the_trees_hook_directory_does_not_run_while_it_is_undone() {
    let scratch = scratch("tree-hooks");
    let tree = scratch.root.join("repo");
    fs::create_dir_all(&tree).expect("repo");
    fs::create_dir_all(tree.join("githooks")).expect("hooks");
    fs::write(tree.join("githooks/README"), "the repository's hooks\n").expect("a file");
    repository(&tree);
    // The repository keeps its hooks in the tree, as husky or a `.githooks/` convention does.
    git(&tree, &["config", "core.hooksPath", "githooks"]);
    let marker = scratch.root.join("planted-ran");
    let script = format!("#!/bin/sh\necho \"$1\" >> {}\n", marker.display());
    let harness = Acting {
        action: "not.offered",
        act: move |tree: &Path| {
            let hook = tree.join("githooks/reference-transaction");
            fs::write(&hook, &script).expect("a hook");
            fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).expect("exec bit");
        },
    };
    let validators = tests_run("true");
    let (stop, text) = run(&tree, "tree-hooks", 2, &harness, &validators);
    let stop = stop.expect("the run stops");
    assert!(matches!(&stop, Stop::StepRefused(_)), "{stop:?}\n{text}");
    assert!(
        !tree.join("githooks/reference-transaction").exists(),
        "{text}"
    );
    assert!(
        !marker.exists(),
        "the refused step's hook ran inside takt's own git calls: {}\n{text}",
        read(&marker)
    );
}
