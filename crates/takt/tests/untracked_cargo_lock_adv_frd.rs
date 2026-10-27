//! Adversarial cases on story:untracked-cargo-lock-remedy. Its acceptance: "On a cargo repository
//! with no committed `Cargo.lock`, the `tests.run` refusal names the remedy: commit `Cargo.lock`,
//! or ignore it, or give a `--test-cmd` that does not write it." Every repository is a scratch one
//! under the crate's build directory.

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
/// `writes` (relative to the repository); `extra` runs on the tree before the first commit.
fn repository(name: &str, writes: &str, extra: impl FnOnce(&Path)) -> Scratch {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "untracked-cargo-lock-adv-frd-{}-{name}",
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
    extra(&repo);
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

/// The untracked test (`git ls-files -- <path>`) reads the path as a pathspec, not literally: a
/// directory `:` makes `:/Cargo.lock`, which git reads as the root's `Cargo.lock` (`:/` is the
/// top of the tree), so a tracked root lock hides the untracked one, and the refusal names no
/// remedy. story:a-gate-script-with-a-colon-is-run fixed the same class in the validator.
#[test]
fn an_untracked_cargo_lock_under_a_colon_directory_gets_the_remedy() {
    let s = repository("colon", ":/Cargo.lock", |repo| {
        fs::write(repo.join("Cargo.lock"), "version = 4\n").expect("a lock file");
    });
    let (stop, text) = go(&s);
    let why = match &stop {
        Ok(Stop::StepRefused(why)) => why.clone(),
        _ => panic!("expected a refusal, got {}\n{text}", said(&stop)),
    };
    assert!(
        why.starts_with("tests.run wrote what its validator does not: :/Cargo.lock"),
        "the untracked lock is named: {why}"
    );
    assert!(
        why.ends_with(REMEDY),
        "story:untracked-cargo-lock-remedy: an untracked `:/Cargo.lock` is taken for the tracked root lock, no remedy: {why}"
    );
}
