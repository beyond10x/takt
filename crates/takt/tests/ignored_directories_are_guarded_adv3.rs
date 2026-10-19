//! Review pass 3 against story:ignored-directories-are-guarded: the way to the set-aside
//! directory. A link a step plants at the root, the case's directory or the step's directory is
//! refused, never followed, and nothing moves into the tree through it; a file that cannot move
//! does not stop the next; a name under `.git/` or a reason with a line break shapes no line;
//! where the kernel has no `RENAME_NOREPLACE` the fallback still never replaces; across file
//! systems a file is copied and anything else named. Every repository is a scratch one under the
//! crate's build directory and every run sets its own set-aside root beside it.

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering::SeqCst};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use takt::harness::{Harness, StepContext, StepReport};
use takt::validator::{TestsRun, Validator};
use takt::{Run, RunOptions, Stop};

const PROTOCOL: &str = "format: protocol/1

protocol:
  id: edit.and.test
  revision: 1
  description: >-
    Edit and test.

artifacts:
  implementation:
    description: a revision of the code

evidence_kinds:
  test_result:
    description: the result of running tests

claims:
  tests.pass:
    description: pass
    true_when:
      evidence:
        kind: test_result
        result: pass
        subject: implementation

obligations:
  tests.required:
    description: pass
    discharged_when:
      claim: tests.pass

actions:
  implementation.edit:
    description: change the implementation
    effect: write
  tests.run:
    description: run the tests
    effect: read
    may_produce:
      - evidence: test_result

outcomes:
  accepted:
    description: pass
    requires:
      claim: tests.pass
";

fn git(tree: &Path, args: &[&str]) {
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
}

struct Scratch {
    root: PathBuf,
    repo: PathBuf,
}

impl Scratch {
    fn aside_root(&self) -> PathBuf {
        self.root.join("aside")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = Command::new("chmod")
            .args(["-R", "u+rwx"])
            .arg(&self.root)
            .status();
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn repository(name: &str, ignore: &str) -> Scratch {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("ignored-dirs-adv3-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let repo = root.join("repo");
    fs::create_dir_all(&repo).expect("a directory");
    fs::write(root.join("protocol.yaml"), PROTOCOL).expect("the protocol");
    fs::write(repo.join(".gitignore"), ignore).expect("a file");
    fs::write(repo.join("src.txt"), "x\n").expect("a file");
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "start"]);
    Scratch { root, repo }
}

type Attack = Box<dyn Fn(&Path) -> Result<(), anyhow::Error>>;

/// A harness whose first step does `attack` and whose every step edits `notes.md`.
struct Harn {
    attack: Attack,
    done: std::cell::Cell<bool>,
}

impl Harness for Harn {
    fn name(&self) -> &str {
        "harn"
    }

    fn step(&self, context: &StepContext<'_>) -> Result<StepReport, anyhow::Error> {
        fs::write(context.tree.join("notes.md"), "a note\n").expect("a write");
        if !self.done.replace(true) {
            (self.attack)(context.tree)?;
        }
        Ok(StepReport {
            action: "implementation.edit".to_owned(),
            summary: "s".to_owned(),
            tokens: Some(1),
        })
    }
}

fn options(protocol: String, iterations: u64) -> RunOptions {
    RunOptions {
        protocol,
        intent: "x".to_owned(),
        inputs: Vec::new(),
        iterations: Some(iterations),
        hours: None,
        tokens: None,
        case_id: Some("rv".to_owned()),
        gate: None,
    }
}

/// A run of the case `rv` with the set-aside root `root`: a failing `tests.run` (step 1), then
/// the harness's step (step 2).
fn go_root(scratch: &Scratch, root: &Path, attack: Attack) -> (anyhow::Result<Stop>, String) {
    let protocol = scratch.root.join("protocol.yaml").display().to_string();
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(TestsRun::new("false").expect("a command"))];
    let harness = Harn {
        attack,
        done: std::cell::Cell::new(false),
    };
    let mut run =
        Run::open(&scratch.repo, options(protocol, 2), &harness, &validators).expect("opens");
    run.set_aside_in(root.to_path_buf())
        .expect("a set-aside root");
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    (stop, String::from_utf8_lossy(&out).into_owned())
}

fn go(scratch: &Scratch, attack: Attack) -> (anyhow::Result<Stop>, String) {
    go_root(scratch, &scratch.aside_root(), attack)
}

fn said(stop: &anyhow::Result<Stop>) -> String {
    match stop {
        Ok(stop) => format!("{stop:?}"),
        Err(error) => format!("Err({error:#})"),
    }
}

/// Every path under `dir`, relative to it, links as links.
fn listing(dir: &Path) -> Vec<String> {
    let mut found = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(at) = stack.pop() {
        let Ok(entries) = fs::read_dir(&at) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if entry.file_type().expect("a type").is_dir() {
                stack.push(path.clone());
            }
            found.push(
                path.strip_prefix(dir)
                    .expect("under it")
                    .display()
                    .to_string(),
            );
        }
    }
    found.sort();
    found
}

/// A case resumes on a tree with uncommitted work; a set-aside that fails during the discard
/// keeps that work.
#[test]
fn a_resume_on_a_dirty_tree_keeps_the_operators_work_when_the_set_aside_fails() {
    let s = repository("dirty", "build/\n");
    fs::create_dir_all(s.repo.join("build")).expect("a directory");
    fs::write(s.repo.join("build/old.o"), "AAAA").expect("a file");
    let protocol = s.root.join("protocol.yaml").display().to_string();
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(TestsRun::new("false").expect("a command"))];
    let first = Harn {
        attack: Box::new(|_| Ok(())),
        done: std::cell::Cell::new(true),
    };
    {
        let mut run =
            Run::open(&s.repo, options(protocol.clone(), 1), &first, &validators).expect("opens");
        run.set_aside_in(s.aside_root()).expect("a root");
        let mut out = Vec::new();
        let _ = run.run(&mut out);
    }
    fs::write(s.repo.join("src.txt"), "operator edit\n").expect("a file");
    fs::write(s.repo.join("draft.txt"), "operator draft\n").expect("a file");
    let blocker = s.root.join("aside2");
    let second = Harn {
        attack: Box::new(move |tree: &Path| {
            fs::write(&blocker, "in the way").expect("a file");
            fs::write(tree.join("build/old.o"), "POISON").expect("a write");
            Ok(())
        }),
        done: std::cell::Cell::new(false),
    };
    let mut run = Run::resume(&s.repo, options(protocol, 3), "rv", &second, &validators)
        .unwrap_or_else(|error| panic!("a dirty tree does not resume: {error:#}"));
    run.set_aside_in(s.root.join("aside2")).expect("a root");
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let said = said(&stop);
    let src = fs::read_to_string(s.repo.join("src.txt")).unwrap_or_default();
    let draft = fs::read_to_string(s.repo.join("draft.txt")).ok();
    assert!(
        src == "operator edit\n" && draft.as_deref() == Some("operator draft\n"),
        "src.txt now {src:?}, draft.txt now {draft:?}: {said}"
    );
    assert!(!s.repo.join("notes.md").exists(), "notes.md stayed: {said}");
    assert!(
        said.contains("could not be moved out of the tree"),
        "the refusal does not name the failed set-aside: {said}"
    );
}

/// The step replaces the case's directory under the accepted root with a link into the tree's
/// ignored `build/`: nothing moves through it, the refusal names it, and the rewrite stays where
/// the undo left it.
#[test]
fn a_link_at_the_case_dir_does_not_carry_the_set_aside_into_the_tree() {
    let s = repository("link-case", "build/\n");
    fs::create_dir_all(s.repo.join("build")).expect("a directory");
    fs::write(s.repo.join("build/old.o"), "AAAA").expect("a file");
    let root = s.aside_root();
    let (stop, text) = go(
        &s,
        Box::new(move |tree: &Path| {
            fs::create_dir_all(&root).expect("a directory");
            std::os::unix::fs::symlink(tree.join("build"), root.join("rv")).expect("a link");
            fs::write(tree.join("build/old.o"), "POISON").expect("a write");
            Ok(())
        }),
    );
    let said = said(&stop);
    assert!(
        !s.repo.join("build/step-2").exists(),
        "the set-aside went into the tree through the link: {:?}; {said}",
        listing(&s.repo.join("build"))
    );
    assert!(
        matches!(&stop, Ok(Stop::StepRefused(why))
            if why.contains("could not be moved out of the tree")
                && why.contains("can be reached without a link")
                && why.contains("build/old.o")),
        "{said}\n{text}"
    );
    assert_eq!(
        fs::read_to_string(s.repo.join("build/old.o"))
            .ok()
            .as_deref(),
        Some("POISON"),
        "{said}"
    );
}

/// The root itself, absent when the run opened, is made a link into the tree by the step.
#[test]
fn a_link_at_the_root_does_not_carry_the_set_aside_into_the_tree() {
    let s = repository("link-root", "build/\n");
    fs::create_dir_all(s.repo.join("build")).expect("a directory");
    fs::write(s.repo.join("build/old.o"), "AAAA").expect("a file");
    let root = s.aside_root();
    let (stop, text) = go(
        &s,
        Box::new(move |tree: &Path| {
            std::os::unix::fs::symlink(tree.join("build"), &root).expect("a link");
            fs::write(tree.join("build/old.o"), "POISON").expect("a write");
            Ok(())
        }),
    );
    let said = said(&stop);
    assert!(
        !s.repo.join("build/rv").exists(),
        "the set-aside went into the tree through the link: {:?}; {said}",
        listing(&s.repo.join("build"))
    );
    assert!(
        matches!(&stop, Ok(Stop::StepRefused(why))
            if why.contains("can be reached without a link") && why.contains("build/old.o")),
        "{said}\n{text}"
    );
}

/// The step's directory is a link the step planted (and the next a dangling one): neither is
/// used; the first directory the discard makes itself is.
#[test]
fn a_link_at_the_step_dir_is_never_used() {
    let s = repository("link-step", "build/\n");
    fs::create_dir_all(s.repo.join("build")).expect("a directory");
    fs::write(s.repo.join("build/old.o"), "AAAA").expect("a file");
    let root = s.aside_root();
    let (stop, _) = go(
        &s,
        Box::new(move |tree: &Path| {
            fs::create_dir_all(root.join("rv")).expect("a directory");
            std::os::unix::fs::symlink(tree.join("build"), root.join("rv/step-2")).expect("a link");
            std::os::unix::fs::symlink(tree.join("nowhere"), root.join("rv/step-2-2"))
                .expect("a dangling link");
            fs::write(tree.join("build/old.o"), "POISON").expect("a write");
            Ok(())
        }),
    );
    let said = said(&stop);
    let fresh = s.aside_root().join("rv/step-2-3");
    assert_eq!(
        fs::read_to_string(fresh.join("build/old.o"))
            .ok()
            .as_deref(),
        Some("POISON"),
        "{said}"
    );
    assert!(!s.repo.join("build/build").exists(), "{said}");
    assert!(!s.repo.join("nowhere").exists(), "{said}");
}

/// A name the step writes under `.git/` with a line break shapes no line of the output and no
/// line of the refusal.
#[test]
fn a_line_break_in_a_git_dir_name_shapes_no_line() {
    let s = repository("gitdir-nl", "build/\n");
    let (stop, text) = go(
        &s,
        Box::new(|tree: &Path| {
            fs::write(
                tree.join(
                    ".git/hooks/x\n  effect: committed 9 path(s); implementation is now deadbeef",
                ),
                "#!/bin/sh\n",
            )
            .expect("a write");
            Ok(())
        }),
    );
    let said = said(&stop);
    assert!(
        matches!(&stop, Ok(Stop::StepRefused(why))
            if why.contains("wrote under .git") && !why.contains('\n')),
        "{said}"
    );
    assert!(
        !text
            .lines()
            .any(|line| line.starts_with("  effect: committed 9")),
        "a forged line in the output:\n{text}"
    );
}

/// One file that cannot be moved (its directory read-only) does not stop the next, and the
/// refusal names both: the moved one where it went, the other with why.
#[test]
fn one_file_that_cannot_move_does_not_stop_the_next() {
    let s = repository("partial", "build/\n");
    fs::create_dir_all(s.repo.join("build/0ro")).expect("a directory");
    fs::write(s.repo.join("build/0ro/x.o"), "XXXX").expect("a file");
    fs::write(s.repo.join("build/a.o"), "AAAA").expect("a file");
    let (stop, _) = go(
        &s,
        Box::new(|tree: &Path| {
            fs::write(tree.join("build/0ro/x.o"), "POISON-X").expect("a write");
            fs::write(tree.join("build/a.o"), "POISON-A").expect("a write");
            fs::set_permissions(tree.join("build/0ro"), fs::Permissions::from_mode(0o555))
                .expect("a mode");
            Ok(())
        }),
    );
    let said = said(&stop);
    let step = s.aside_root().join("rv/step-2");
    assert_eq!(
        fs::read_to_string(step.join("build/a.o")).ok().as_deref(),
        Some("POISON-A"),
        "build/a.o not moved: {said}"
    );
    assert!(
        matches!(&stop, Ok(Stop::StepRefused(why))
            if why.contains("could not be moved out of the tree") && why.contains("build/0ro/x.o")
               && why.contains("moved out of the tree, not deleted") && why.contains("build/a.o")),
        "{said}"
    );
}

/// A refusal before the ignored check (the step also wrote into the cases directory) still says
/// where the rewrite went.
#[test]
fn a_cases_dir_refusal_names_where_the_rewrite_went() {
    let s = repository("cases", "build/\n");
    fs::create_dir_all(s.repo.join("build")).expect("a directory");
    fs::write(s.repo.join("build/old.o"), "AAAA").expect("a file");
    let (stop, _) = go(
        &s,
        Box::new(|tree: &Path| {
            fs::write(tree.join(".engineering/cases/rv/forged.yaml"), "x").expect("a write");
            fs::write(tree.join("build/old.o"), "POISON").expect("a write");
            Ok(())
        }),
    );
    let said = said(&stop);
    let step = s.aside_root().join("rv/step-2");
    assert!(
        matches!(&stop, Ok(Stop::StepRefused(why))
            if why.contains("cases directory") && why.contains(&takt::git::shown(step.as_os_str()))),
        "{said}"
    );
    assert_eq!(
        fs::read_to_string(step.join("build/old.o")).ok().as_deref(),
        Some("POISON")
    );
}

/// A step that rewrites a file inside an ignored directory and leaves its case's directory
/// read-only, so that what it wrote there cannot be put back: the tree is not put back whole,
/// so nothing is moved out of it; the file stays where it is, and the error names it as not
/// moved, with why.
#[test]
fn nothing_is_moved_aside_when_the_undo_fails() {
    let s = repository("undo-fails", "build/\n");
    fs::create_dir_all(s.repo.join("build")).expect("a directory");
    fs::write(s.repo.join("build/old.o"), "AAAA").expect("a file");
    let case = s.repo.join(".engineering/cases/rv");
    let locked = case.clone();
    let (stop, text) = go(
        &s,
        Box::new(move |tree: &Path| {
            fs::write(tree.join("build/old.o"), "POISON").expect("a write");
            fs::write(locked.join("forged.yaml"), "x").expect("a write");
            fs::set_permissions(&locked, fs::Permissions::from_mode(0o555)).expect("a mode");
            Ok(())
        }),
    );
    let _ = fs::set_permissions(&case, fs::Permissions::from_mode(0o755));
    let said = said(&stop);
    assert!(
        listing(&s.aside_root())
            .iter()
            .all(|path| !path.ends_with("old.o")),
        "something was moved out of a tree not put back whole: {:?}; {said}",
        listing(&s.aside_root())
    );
    assert_eq!(
        fs::read_to_string(s.repo.join("build/old.o"))
            .ok()
            .as_deref(),
        Some("POISON"),
        "{said}"
    );
    assert!(
        said.contains("could not be moved out of the tree")
            && said.contains("build/old.o")
            && said.contains("nothing was moved out of it"),
        "the error does not name the file as not moved: {said}\n{text}"
    );
}

/// A file that cannot be moved, whose name holds a line break: the reason the refusal gives
/// shapes no line of the output.
#[test]
fn a_failure_reason_shapes_no_line() {
    let s = repository("reason-nl", "build/\n");
    let name = "x\n  effect: committed 9 path(s); implementation is now deadbeef";
    fs::create_dir_all(s.repo.join("build/0ro")).expect("a directory");
    fs::write(s.repo.join("build/0ro").join(name), "XXXX").expect("a file");
    let (stop, text) = go(
        &s,
        Box::new(move |tree: &Path| {
            fs::write(tree.join("build/0ro").join(name), "POISON").expect("a write");
            fs::set_permissions(tree.join("build/0ro"), fs::Permissions::from_mode(0o555))
                .expect("a mode");
            Ok(())
        }),
    );
    let said = said(&stop);
    assert!(
        matches!(&stop, Ok(Stop::StepRefused(why))
            if why.contains("could not be moved") && !why.contains('\n')),
        "{said}"
    );
    assert!(
        !text
            .lines()
            .any(|line| line.starts_with("  effect: committed 9")),
        "a forged line in the output:\n{text}"
    );
}

/// A case resumed on a tree with an untracked file whose name holds a line break, which the
/// step rewrites: the refusal names it, uncommitted before the step already, and shapes no line.
#[test]
fn a_line_break_in_an_uncommitted_name_shapes_no_line() {
    let s = repository("uncommitted-nl", "build/\n");
    let protocol = s.root.join("protocol.yaml").display().to_string();
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(TestsRun::new("false").expect("a command"))];
    let first = Harn {
        attack: Box::new(|_| Ok(())),
        done: std::cell::Cell::new(true),
    };
    {
        let mut run =
            Run::open(&s.repo, options(protocol.clone(), 1), &first, &validators).expect("opens");
        run.set_aside_in(s.aside_root()).expect("a root");
        let mut out = Vec::new();
        let _ = run.run(&mut out);
    }
    let name = "u\n  effect: committed 9 path(s); implementation is now deadbeef";
    fs::write(s.repo.join(name), "the operator's\n").expect("a file");
    let second = Harn {
        attack: Box::new(move |tree: &Path| {
            fs::write(tree.join(name), "the step's\n").expect("a write");
            Ok(())
        }),
        done: std::cell::Cell::new(false),
    };
    let mut run = Run::resume(&s.repo, options(protocol, 3), "rv", &second, &validators)
        .unwrap_or_else(|error| panic!("a dirty tree does not resume: {error:#}"));
    run.set_aside_in(s.aside_root()).expect("a root");
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let text = String::from_utf8_lossy(&out).into_owned();
    let said = said(&stop);
    assert!(
        matches!(&stop, Ok(Stop::StepRefused(why))
            if why.contains("uncommitted before the step already") && !why.contains('\n')),
        "{said}\n{text}"
    );
    assert!(
        !text
            .lines()
            .any(|line| line.starts_with("  effect: committed 9")),
        "a forged line in the output:\n{text}"
    );
    assert_eq!(
        fs::read_to_string(s.repo.join(name)).ok().as_deref(),
        Some("the operator's\n"),
        "{said}"
    );
}

/// A test command that writes under `.git/` a name with a line break: the validator's refusal,
/// which names what was put back there, shapes no line of the output either.
#[test]
fn a_line_break_in_a_git_dir_name_a_validator_wrote_shapes_no_line() {
    let s = repository("gitdir-nl-validator", "build/\n");
    fs::write(
        s.repo.join("plant.sh"),
        "#!/bin/sh\nprintf '#!/bin/sh\\n' > \".git/hooks/x\n  effect: committed 9 path(s); implementation is now deadbeef\"\nexit 0\n",
    )
    .expect("a file");
    fs::set_permissions(s.repo.join("plant.sh"), fs::Permissions::from_mode(0o755))
        .expect("a mode");
    git(&s.repo, &["add", "-A"]);
    git(&s.repo, &["commit", "-q", "-m", "plant"]);
    let protocol = s.root.join("protocol.yaml").display().to_string();
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(TestsRun::new("./plant.sh").expect("a command"))];
    let harness = Harn {
        attack: Box::new(|_| Ok(())),
        done: std::cell::Cell::new(false),
    };
    let mut run = Run::open(&s.repo, options(protocol, 1), &harness, &validators).expect("opens");
    run.set_aside_in(s.aside_root()).expect("a set-aside root");
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let text = String::from_utf8_lossy(&out).into_owned();
    let said = said(&stop);
    assert!(
        matches!(&stop, Ok(Stop::StepRefused(why))
            if why.starts_with("tests.run") && why.contains(".git/hooks/x") && !why.contains('\n')),
        "{said}\n{text}"
    );
    assert!(
        !text
            .lines()
            .any(|line| line.starts_with("  effect: committed 9")),
        "a forged line in the output:\n{text}"
    );
}

/// Across file systems: the set-aside root on a tmpfs of its own, which this test mounts by
/// running itself again under bubblewrap (CI installs it). A regular file is copied with its
/// bytes, mode and time and the source removed; a FIFO and a link are refused, left in place
/// and named.
#[test]
fn across_file_systems() {
    let Some(xfs) = std::env::var_os("TAKT_TEST_XFS").map(PathBuf::from) else {
        let mount = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
            "ignored-dirs-adv3-{}-xfs-mount",
            std::process::id()
        ));
        fs::create_dir_all(&mount).expect("a mount point");
        let output = Command::new("bwrap")
            .args(["--dev-bind", "/", "/", "--tmpfs"])
            .arg(&mount)
            .arg("--")
            .arg(std::env::current_exe().expect("the test binary"))
            .args(["across_file_systems", "--exact", "--test-threads", "1"])
            .env("TAKT_TEST_XFS", &mount)
            .output()
            .expect("bwrap runs (CI installs bubblewrap)");
        let _ = fs::remove_dir(&mount);
        assert!(
            output.status.success(),
            "under bwrap:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    };
    let s = repository("xfs", "build/\n");
    fs::create_dir_all(s.repo.join("build")).expect("a directory");
    fs::write(s.repo.join("build/old.o"), "AAAA").expect("a file");
    fs::write(s.repo.join("build/lnk.o"), "LLLL").expect("a file");
    let fifo = s.repo.join("build/pipe");
    assert!(
        Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .expect("mkfifo")
            .success()
    );
    let when = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000);
    let (stop, text) = go_root(
        &s,
        &xfs.join("aside"),
        Box::new(move |tree: &Path| {
            fs::write(tree.join("build/old.o"), "POISON").expect("a write");
            fs::set_permissions(tree.join("build/old.o"), fs::Permissions::from_mode(0o640))
                .expect("a mode");
            fs::File::options()
                .write(true)
                .open(tree.join("build/old.o"))
                .expect("opens")
                .set_modified(when)
                .expect("a time");
            fs::set_permissions(tree.join("build/pipe"), fs::Permissions::from_mode(0o600))
                .expect("a mode");
            fs::remove_file(tree.join("build/lnk.o")).expect("removed");
            std::os::unix::fs::symlink("/srv/elsewhere", tree.join("build/lnk.o")).expect("a link");
            Ok(())
        }),
    );
    let said = said(&stop);
    let step = xfs.join("aside/rv/step-2");
    let copy = step.join("build/old.o");
    assert_eq!(
        fs::read_to_string(&copy).ok().as_deref(),
        Some("POISON"),
        "{said}\n{text}"
    );
    let meta = fs::metadata(&copy).expect("the copy");
    assert_eq!(meta.permissions().mode() & 0o777, 0o640, "the mode");
    assert_eq!(meta.modified().expect("a time"), when, "the time");
    assert!(!s.repo.join("build/old.o").exists(), "the source stayed");
    assert!(
        fs::symlink_metadata(&fifo).is_ok_and(|meta| {
            use std::os::unix::fs::FileTypeExt as _;
            meta.file_type().is_fifo()
        }),
        "the FIFO is gone"
    );
    assert!(
        fs::symlink_metadata(s.repo.join("build/lnk.o"))
            .is_ok_and(|meta| meta.file_type().is_symlink()),
        "the link is gone"
    );
    assert!(
        fs::symlink_metadata(step.join("build/lnk.o")).is_err(),
        "the link was copied"
    );
    assert!(
        matches!(&stop, Ok(Stop::StepRefused(why))
            if why.contains("could not be moved") && why.contains("build/pipe") && why.contains("build/lnk.o")),
        "{said}"
    );
}

/// Installs, on the calling thread only, a seccomp filter under which `renameat2` with
/// `RENAME_NOREPLACE` fails with `errno`: a kernel or file system without the flag.
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn no_noreplace(errno: u32) {
    #[repr(C)]
    struct SockFilter {
        code: u16,
        jt: u8,
        jf: u8,
        k: u32,
    }
    #[repr(C)]
    struct SockFprog {
        len: u16,
        filter: *const SockFilter,
    }
    const LD_W_ABS: u16 = 0x20;
    const JEQ_K: u16 = 0x15;
    const RET_K: u16 = 0x06;
    const ALLOW: u32 = 0x7fff_0000;
    const ERRNO: u32 = 0x0005_0000;
    const ARCH_X86_64: u32 = 0xc000_003e;
    let nr = u32::try_from(libc::SYS_renameat2).expect("a number");
    let prog = [
        SockFilter {
            code: LD_W_ABS,
            jt: 0,
            jf: 0,
            k: 4,
        },
        SockFilter {
            code: JEQ_K,
            jt: 0,
            jf: 5,
            k: ARCH_X86_64,
        },
        SockFilter {
            code: LD_W_ABS,
            jt: 0,
            jf: 0,
            k: 0,
        },
        SockFilter {
            code: JEQ_K,
            jt: 0,
            jf: 3,
            k: nr,
        },
        SockFilter {
            code: LD_W_ABS,
            jt: 0,
            jf: 0,
            k: 16 + 4 * 8,
        },
        SockFilter {
            code: JEQ_K,
            jt: 0,
            jf: 1,
            k: 1,
        },
        SockFilter {
            code: RET_K,
            jt: 0,
            jf: 0,
            k: ERRNO | errno,
        },
        SockFilter {
            code: RET_K,
            jt: 0,
            jf: 0,
            k: ALLOW,
        },
    ];
    let fprog = SockFprog {
        len: u16::try_from(prog.len()).expect("a length"),
        filter: prog.as_ptr(),
    };
    // SAFETY: a filter program that outlives the call, installed on this thread only.
    unsafe {
        assert_eq!(libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0), 0);
        assert_eq!(
            libc::prctl(
                libc::PR_SET_SECCOMP,
                libc::SECCOMP_MODE_FILTER,
                &raw const fprog,
            ),
            0,
            "seccomp: {}",
            std::io::Error::last_os_error()
        );
    }
}

/// Where `renameat2` has no `RENAME_NOREPLACE` (`EINVAL`, `ENOSYS`), the link-and-unlink
/// fallback moves the rewritten file (and a link as the link) and never touches what a run
/// before left.
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn the_fallback_moves_and_never_replaces() {
    for errno in [libc::EINVAL, libc::ENOSYS] {
        let errno = u32::try_from(errno).expect("an errno");
        let s = repository(&format!("fallback-{errno}"), "build/\n");
        fs::create_dir_all(s.repo.join("build")).expect("a directory");
        fs::write(s.repo.join("build/old.o"), "AAAA").expect("a file");
        fs::write(s.repo.join("build/lnk.o"), "LLLL").expect("a file");
        let decoy = s.aside_root().join("rv/step-2/build/old.o");
        fs::create_dir_all(decoy.parent().expect("a parent")).expect("a directory");
        fs::write(&decoy, "decoy").expect("a file");
        let probe = s.root.join("probe");
        let (stop, _) = std::thread::scope(|scope| {
            scope
                .spawn(|| {
                    no_noreplace(errno);
                    // The filter is live on this thread: a NOREPLACE rename fails.
                    fs::write(&probe, "p").expect("a file");
                    let from = std::ffi::CString::new(probe.as_os_str().as_encoded_bytes())
                        .expect("a name");
                    let to =
                        std::ffi::CString::new(format!("{}2", probe.display())).expect("a name");
                    // SAFETY: NUL-terminated names that outlive the call.
                    let renamed = unsafe {
                        libc::renameat2(
                            libc::AT_FDCWD,
                            from.as_ptr(),
                            libc::AT_FDCWD,
                            to.as_ptr(),
                            libc::RENAME_NOREPLACE,
                        )
                    };
                    assert_eq!(renamed, -1, "the filter is not live");
                    go(
                        &s,
                        Box::new(|tree: &Path| {
                            fs::write(tree.join("build/old.o"), "POISON").expect("a write");
                            fs::remove_file(tree.join("build/lnk.o")).expect("removed");
                            std::os::unix::fs::symlink("/srv/elsewhere", tree.join("build/lnk.o"))
                                .expect("a link");
                            Ok(())
                        }),
                    )
                })
                .join()
                .expect("the thread")
        });
        let said = said(&stop);
        let fresh = s.aside_root().join("rv/step-2-2");
        assert_eq!(
            fs::read_to_string(&decoy).ok().as_deref(),
            Some("decoy"),
            "{said}"
        );
        assert_eq!(
            fs::read_to_string(fresh.join("build/old.o"))
                .ok()
                .as_deref(),
            Some("POISON"),
            "{errno}: {said}"
        );
        assert!(
            fs::symlink_metadata(fresh.join("build/lnk.o"))
                .is_ok_and(|meta| meta.file_type().is_symlink()),
            "{errno}: the link was not moved as a link: {said}"
        );
        assert!(!s.repo.join("build/old.o").exists(), "{errno}: {said}");
        assert!(
            fs::symlink_metadata(s.repo.join("build/lnk.o")).is_err(),
            "{errno}: {said}"
        );
    }
}

// Two passes against the wave close: a process the step left behind, active while the
// discard moves files aside, is caught by the read of the tree after the move.

type Handles = Arc<Mutex<Vec<std::thread::JoinHandle<u64>>>>;

fn files(dir: &Path, n: usize, body: &str) {
    fs::create_dir_all(dir).expect("a directory");
    for i in 0..n {
        fs::write(dir.join(format!("f{i:04}.o")), body).expect("a file");
    }
}

/// Every regular file under `dir` whose bytes are `body`.
fn count_body(dir: &Path, body: &str) -> usize {
    listing(dir)
        .iter()
        .filter(|rel| {
            let p = dir.join(rel);
            fs::symlink_metadata(&p).is_ok_and(|m| m.is_file())
                && fs::read_to_string(&p).ok().as_deref() == Some(body)
        })
        .count()
}

fn wait_for(path: &Path, stop: &AtomicBool) -> bool {
    let deadline = Instant::now() + Duration::from_secs(30);
    while !stop.load(SeqCst) && Instant::now() < deadline {
        if path.exists() {
            return true;
        }
        std::hint::spin_loop();
    }
    false
}

/// A process the step left behind writes an untracked file once the discard has begun moving
/// files aside: the read of the tree after the move names it, and the error still says where
/// the files went.
#[test]
fn a_lingering_writer_reaches_the_read_after_the_move() {
    let (mut reached, mut named) = (0, 0);
    let mut last = String::new();
    for attempt in 0..6 {
        let s = repository(&format!("l1-{attempt}"), "build/\n");
        files(&s.repo.join("build"), 300, "AAAA");
        let stop = Arc::new(AtomicBool::new(false));
        let handles: Handles = Arc::default();
        let (st, hs, aside) = (stop.clone(), handles.clone(), s.aside_root());
        let (run, _text) = go(
            &s,
            Box::new(move |tree: &Path| {
                files(&tree.join("build"), 300, "POISON");
                let (tree, st, aside) = (tree.to_path_buf(), st.clone(), aside.clone());
                hs.lock().unwrap().push(std::thread::spawn(move || {
                    let mut n = 0;
                    if wait_for(&aside.join("rv/step-2"), &st) {
                        while !st.load(SeqCst) {
                            let _ = fs::write(tree.join("racer.txt"), "r");
                            n += 1;
                            std::thread::sleep(Duration::from_micros(200));
                        }
                    }
                    n
                }));
                Ok(())
            }),
        );
        stop.store(true, SeqCst);
        for h in handles.lock().unwrap().drain(..) {
            let _ = h.join();
        }
        let said = said(&run);
        if said.contains("could not be put back") {
            reached += 1;
            if said.contains("moved out of the tree, not deleted, into") {
                named += 1;
            }
        }
        last = said;
    }
    eprintln!(
        "R1 reached {reached}/6, named {named}; last: {}",
        &last[..last.len().min(600)]
    );
    assert!(reached > 0, "the post-move bail was never reached: {last}");
    assert_eq!(
        named, reached,
        "the post-move bail did not name the set-aside: {last}"
    );
}

/// A process the step left behind renames the case's set-aside directory into the tree's
/// ignored directory once the discard has opened it. Nothing may be lost, and the run may not
/// end as if the tree were put back.
#[test]
fn a_case_dir_renamed_into_the_tree_after_open_is_caught() {
    let s = repository("case-into-tree", "build/\n");
    files(&s.repo.join("build"), 300, "AAAA");
    let stop = Arc::new(AtomicBool::new(false));
    let handles: Handles = Arc::default();
    let (st, hs, aside) = (stop.clone(), handles.clone(), s.aside_root());
    let (run, _text) = go(
        &s,
        Box::new(move |tree: &Path| {
            files(&tree.join("build"), 300, "POISON");
            let (tree, st, aside) = (tree.to_path_buf(), st.clone(), aside.clone());
            hs.lock().unwrap().push(std::thread::spawn(move || {
                if wait_for(&aside.join("rv/step-2"), &st)
                    && fs::rename(aside.join("rv"), tree.join("build/zz")).is_ok()
                {
                    return 1;
                }
                0
            }));
            Ok(())
        }),
    );
    stop.store(true, SeqCst);
    let mut swapped = 0;
    for h in handles.lock().unwrap().drain(..) {
        swapped += h.join().unwrap_or(0);
    }
    let said = said(&run);
    let in_aside = count_body(&s.aside_root(), "POISON");
    let in_tree = count_body(&s.repo.join("build"), "POISON");
    eprintln!(
        "R3 swapped {swapped}; POISON in aside {in_aside}, in tree {in_tree}; {}",
        &said[..said.len().min(500)]
    );
    assert_eq!(in_aside + in_tree, 300, "files lost: {said}");
    if swapped == 1 {
        assert!(
            !matches!(&run, Ok(Stop::StepRefused(_)))
                || in_tree == 0
                || said.contains("could not be put back"),
            "the run ended as a refusal with files moved into the tree: {said}"
        );
    }
}
