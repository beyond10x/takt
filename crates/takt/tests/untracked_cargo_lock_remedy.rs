//! story:untracked-cargo-lock-remedy: a `tests.run` whose test command writes a `Cargo.lock` the
//! repository does not track is refused with the remedy named on the same line: commit it, ignore
//! it, or give a test command that does not write it. Any other untracked path keeps the refusal
//! it had, with no remedy. Every repository is a scratch one under the crate's build directory.

use std::cell::Cell;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

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

const REMEDY: &str = "commit Cargo.lock, or ignore it (.gitignore), or give --test-cmd a command that does not write it";

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

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// A cargo-shaped repository with a committed manifest and a committed test command that writes
/// `writes` (a path relative to the repository), and no `Cargo.lock` committed.
fn repository(name: &str, writes: &str) -> Scratch {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "untracked-cargo-lock-{}-{name}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    let repo = root.join("repo");
    fs::create_dir_all(repo.join("src")).expect("a directory");
    fs::write(root.join("protocol.yaml"), PROTOCOL).expect("the protocol");
    fs::write(
        repo.join("Cargo.toml"),
        "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("a manifest");
    fs::write(repo.join("src/lib.rs"), "").expect("a source");
    let script = format!(
        "#!/bin/sh\nmkdir -p \"$(dirname '{writes}')\"\nprintf 'version = 4\\n' > '{writes}'\n"
    );
    fs::write(repo.join("fake-cargo.sh"), script).expect("the test command");
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "start"]);
    Scratch { root, repo }
}

fn options(protocol: String) -> RunOptions {
    RunOptions {
        protocol,
        intent: "x".to_owned(),
        inputs: Vec::new(),
        iterations: Some(1),
        hours: None,
        tokens: None,
        case_id: Some("rv".to_owned()),
        gate: None,
    }
}

/// A harness whose every step edits `notes.md` and reports an implementation edit.
struct Harn {
    done: Cell<bool>,
}

impl Harness for Harn {
    fn name(&self) -> &str {
        "harn"
    }

    fn step(&self, context: &StepContext<'_>) -> Result<StepReport, anyhow::Error> {
        fs::write(context.tree.join("notes.md"), "a note\n").expect("a write");
        self.done.set(true);
        Ok(StepReport {
            action: "implementation.edit".to_owned(),
            summary: "s".to_owned(),
            tokens: Some(1),
        })
    }
}

/// One run of the repository's test command `sh ./fake-cargo.sh` under the library's run entry.
fn go(scratch: &Scratch) -> (anyhow::Result<Stop>, String) {
    let protocol = scratch.root.join("protocol.yaml").display().to_string();
    let validators: Vec<Box<dyn Validator>> = vec![Box::new(
        TestsRun::new("sh ./fake-cargo.sh").expect("a command"),
    )];
    let harness = Harn {
        done: Cell::new(false),
    };
    let mut run =
        Run::open(&scratch.repo, options(protocol), &harness, &validators).expect("opens");
    run.set_aside_in(scratch.root.join("aside"))
        .expect("a set-aside root");
    let mut out = Vec::new();
    run.continue_past_red_base();
    let stop = run.run(&mut out);
    (stop, String::from_utf8_lossy(&out).into_owned())
}

fn said(stop: &anyhow::Result<Stop>) -> String {
    match stop {
        Ok(stop) => format!("{stop:?}"),
        Err(error) => format!("Err({error:#})"),
    }
}

/// The refusal's reason, when the run stopped refused at `tests.run` on one line.
fn refusal(stop: &anyhow::Result<Stop>, text: &str) -> String {
    match stop {
        Ok(Stop::StepRefused(why)) => {
            assert!(why.starts_with("tests.run"), "{}\n{text}", said(stop));
            assert!(!why.contains('\n'), "a line break in the reason:\n{why}");
            why.clone()
        }
        _ => panic!("expected a refusal, got {}\n{text}", said(stop)),
    }
}

/// story:untracked-cargo-lock-remedy: a cargo repository with no committed `Cargo.lock`, whose
/// test command writes one, is refused naming the path and the three remedies.
#[test]
fn an_untracked_cargo_lock_a_test_command_writes_is_refused_with_the_remedy() {
    let s = repository("root", "Cargo.lock");
    let (stop, text) = go(&s);
    let why = refusal(&stop, &text);
    assert!(
        why.starts_with("tests.run wrote what its validator does not: Cargo.lock; "),
        "story:untracked-cargo-lock-remedy: the path is not named first: {why}"
    );
    assert!(
        why.ends_with(REMEDY),
        "story:untracked-cargo-lock-remedy: the remedy is not named: {why}"
    );
    assert!(
        text.contains(&format!("refused: {why}")),
        "story:untracked-cargo-lock-remedy: the output does not carry the reason:\n{text}"
    );
}

/// story:untracked-cargo-lock-remedy: the remedy holds at any directory level, a workspace
/// member's `Cargo.lock` included.
#[test]
fn a_nested_untracked_cargo_lock_is_refused_with_the_remedy() {
    let s = repository("nested", "crates/member/Cargo.lock");
    let (stop, text) = go(&s);
    let why = refusal(&stop, &text);
    assert!(
        why.starts_with("tests.run wrote what its validator does not: crates/member/Cargo.lock; "),
        "story:untracked-cargo-lock-remedy: the nested path is not named: {why}"
    );
    assert!(
        why.ends_with(REMEDY),
        "story:untracked-cargo-lock-remedy: the remedy is not named: {why}"
    );
}

/// story:untracked-cargo-lock-remedy: an untracked path that is not a `Cargo.lock` keeps its
/// refusal, with no remedy for a lock file.
#[test]
fn an_untracked_path_that_is_not_a_cargo_lock_gets_no_remedy() {
    let s = repository("other", "build/out.txt");
    let (stop, text) = go(&s);
    let why = refusal(&stop, &text);
    assert!(
        why.starts_with("tests.run wrote what its validator does not: build/out.txt"),
        "story:untracked-cargo-lock-remedy: the path is not named: {why}"
    );
    assert!(
        !why.contains(REMEDY) && !why.contains(".gitignore") && !why.contains("--test-cmd"),
        "story:untracked-cargo-lock-remedy: a remedy for another path: {why}"
    );
}

/// story:untracked-cargo-lock-remedy: a file only named like a lock file (`Cargo.lock.bak`)
/// is not a `Cargo.lock`, so it gets no remedy.
#[test]
fn a_file_named_like_a_cargo_lock_gets_no_remedy() {
    let s = repository("bak", "Cargo.lock.bak");
    let (stop, text) = go(&s);
    let why = refusal(&stop, &text);
    assert!(
        why.starts_with("tests.run wrote what its validator does not: Cargo.lock.bak"),
        "story:untracked-cargo-lock-remedy: the path is not named: {why}"
    );
    assert!(
        !why.contains(REMEDY) && !why.contains(".gitignore") && !why.contains("--test-cmd"),
        "story:untracked-cargo-lock-remedy: a remedy for another path: {why}"
    );
}

/// story:untracked-cargo-lock-remedy: a `Cargo.lock` the repository tracks is not untracked, so
/// a test command that rewrites it is refused without the remedy for an untracked lock file.
#[test]
fn a_tracked_cargo_lock_a_test_command_rewrites_gets_no_remedy() {
    let s = repository("tracked", "Cargo.lock");
    fs::write(s.repo.join("Cargo.lock"), "version = 3\n").expect("a lock file");
    git(&s.repo, &["add", "-A"]);
    git(&s.repo, &["commit", "-q", "-m", "lock"]);
    let (stop, text) = go(&s);
    let why = refusal(&stop, &text);
    assert!(
        why.starts_with("tests.run wrote what its validator does not: Cargo.lock"),
        "story:untracked-cargo-lock-remedy: the path is not named: {why}"
    );
    assert!(
        !why.contains(REMEDY),
        "story:untracked-cargo-lock-remedy: a remedy for a tracked lock file: {why}"
    );
}
