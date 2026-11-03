//! story:an-unrelated-program-install-refuses-no-step, acceptance 3: what the step guard does on a
//! write into a program or import directory outside the tree that it cannot attribute to the
//! step. Secure before autonomous (`product-requirements:takt` § 3): such a write still refuses
//! the step, since a step's own write there looks exactly the same and is what a hostile step
//! would do. The refusal names the path as changed during the step with its origin unknown, the
//! step or another process, apart from what the step is known to have written in the tree, and
//! never says the step wrote it.
//!
//! Every write lands in this test binary's own program directory (`support/own_home.rs`), never
//! in a directory of the machine's home.

#[path = "support/own_home.rs"]
mod own_home;

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Mutex, MutexGuard};

use takt::harness::{Harness, StepContext, StepReport};
use takt::validator::{TestsRun, Validator};
use takt::{Run, RunOptions, Stop};

/// A judgement action, `implementation.edit`, and one validator-owned action, `tests.run`;
/// accepted once the tests pass at the current revision.
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

/// The words a refusal names a path of unknown origin with.
const UNKNOWN: &str = "origin unknown: the step or another process";

/// The cases share this binary's one program directory, so a program one of them writes would
/// reach another's step: they run one at a time.
static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

fn one_at_a_time() -> MutexGuard<'static, ()> {
    ONE_AT_A_TIME
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

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

/// This binary's own `$CARGO_HOME/bin`, which the step guard reads as a program directory, at
/// its real path; made when it is not there yet. Fails the case unless it lies under the
/// private directory `own_home` made.
fn own_program_dir() -> PathBuf {
    let own = std::env::var_os("TAKT_TEST_OWN_HOME")
        .map(PathBuf::from)
        .expect("TAKT_TEST_OWN_HOME: this test binary has no program directories of its own");
    let own = fs::canonicalize(own).expect("the private directory exists");
    let cargo = std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .expect("a CARGO_HOME of this binary's own");
    let bin = cargo.join("bin");
    fs::create_dir_all(&bin).expect("the private cargo bin");
    let bin = fs::canonicalize(bin).expect("the private cargo bin exists");
    assert!(
        bin.starts_with(&own),
        "{} lies outside this binary's own directory {}",
        bin.display(),
        own.display()
    );
    bin
}

/// Writes an executable program at `path`.
fn install(path: &Path) {
    fs::write(path, "#!/bin/sh\nexit 0\n").expect("a program");
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("a mode");
}

/// A scratch repository under this crate's build directory whose test command is `check`, the
/// protocol file beside it; removed on drop, with the program `program` if it is there.
struct Scratch {
    root: PathBuf,
    repo: PathBuf,
    program: PathBuf,
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.program);
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn scratch(name: &str, check: &str) -> Scratch {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("unattributed-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let repo = root.join("repo");
    fs::create_dir_all(&repo).expect("a directory");
    fs::write(root.join("protocol.yaml"), EDIT_AND_TEST).expect("the protocol");
    fs::write(repo.join(".gitignore"), "target/\n").expect("a file");
    fs::write(repo.join("check.sh"), check).expect("the check");
    fs::set_permissions(repo.join("check.sh"), fs::Permissions::from_mode(0o755)).expect("a mode");
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "start"]);
    let program =
        own_program_dir().join(format!("takt-unattributed-{}-{name}", std::process::id()));
    let _ = fs::remove_file(&program);
    Scratch {
        root,
        repo,
        program,
    }
}

/// An `implementation.edit` step that changes nothing; the first time it is offered it lets
/// another writer go and waits until that writer is done.
struct Idle {
    go: Sender<()>,
    done: Receiver<()>,
    first: std::cell::Cell<bool>,
}

impl Harness for Idle {
    fn name(&self) -> &str {
        "idle"
    }

    fn step(&self, context: &StepContext<'_>) -> Result<StepReport, anyhow::Error> {
        if !context.offered.iter().any(|a| a == "implementation.edit") {
            anyhow::bail!("implementation.edit not offered");
        }
        if self.first.replace(false) {
            self.go.send(()).expect("the other writer waits");
            self.done.recv().expect("the other writer answers");
        }
        Ok(StepReport {
            action: "implementation.edit".to_owned(),
            summary: "changed nothing".to_owned(),
            tokens: Some(1),
        })
    }
}

/// An `implementation.edit` step that installs the program at `program` itself the first time.
struct Installs {
    program: PathBuf,
    first: std::cell::Cell<bool>,
}

impl Harness for Installs {
    fn name(&self) -> &str {
        "installs"
    }

    fn step(&self, context: &StepContext<'_>) -> Result<StepReport, anyhow::Error> {
        if !context.offered.iter().any(|a| a == "implementation.edit") {
            anyhow::bail!("implementation.edit not offered");
        }
        if self.first.replace(false) {
            install(&self.program);
        }
        Ok(StepReport {
            action: "implementation.edit".to_owned(),
            summary: "installed a program".to_owned(),
            tokens: Some(1),
        })
    }
}

fn options(protocol: String) -> RunOptions {
    RunOptions {
        protocol,
        intent: "make the check pass".to_owned(),
        inputs: Vec::new(),
        iterations: Some(4),
        hours: None,
        tokens: None,
        case_id: None,
        gate: None,
    }
}

/// Runs the edit-and-test protocol on `scratch` under `harness`; its stop and what it printed.
fn run(scratch: &Scratch, harness: &dyn Harness) -> (anyhow::Result<Stop>, String) {
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(TestsRun::new("./check.sh").expect("a command"))];
    let protocol = scratch.root.join("protocol.yaml").display().to_string();
    let mut run =
        Run::open(&scratch.repo, options(protocol), harness, &validators).expect("the case opens");
    run.continue_past_red_base();
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    (stop, String::from_utf8_lossy(&out).into_owned())
}

/// The reason the run stopped `StepRefused` with; fails the case on any other stop.
fn refusal(stop: &anyhow::Result<Stop>, text: &str) -> String {
    match stop {
        Ok(Stop::StepRefused(why)) => why.clone(),
        other => panic!(
            "the run stops StepRefused, not {:?}\n{text}",
            other.as_ref().map_err(ToString::to_string)
        ),
    }
}

/// Asserts `why` names `program` as of unknown origin, and only after the words that say so:
/// nowhere does it say `action` wrote or changed it.
fn assert_unknown_origin(why: &str, action: &str, program: &Path, text: &str) {
    let program = program.display().to_string();
    let Some((before, after)) = why.split_once(UNKNOWN) else {
        panic!(
            "story:an-unrelated-program-install-refuses-no-step: the refusal names no path of unknown origin ({UNKNOWN:?}): {why}\n{text}"
        );
    };
    assert!(
        after.contains(&program),
        "story:an-unrelated-program-install-refuses-no-step: the refusal names {program} as of unknown origin: {why}\n{text}"
    );
    assert!(
        !before.contains(&program),
        "story:an-unrelated-program-install-refuses-no-step: the refusal names {program} before it says its origin is unknown: {why}\n{text}"
    );
    for says in [
        format!("{action} changed what cargo"),
        format!("{action} wrote what its validator does not: {program}"),
    ] {
        assert!(
            !why.contains(&says),
            "story:an-unrelated-program-install-refuses-no-step: the refusal says the step wrote what it cannot attribute ({says:?}): {why}\n{text}"
        );
    }
}

/// A step that writes nothing while another writer installs a program into the binary's own
/// program directory: the run stops `StepRefused`, the reason names the program as changed
/// during the step with its origin unknown, and never says the step wrote it. The program is
/// left where the other writer put it.
#[test]
fn a_program_another_writer_installs_during_a_step_refuses_it_as_of_unknown_origin() {
    let _one = one_at_a_time();
    let scratch = scratch("other-writer", "#!/bin/sh\nexit 1\n");
    let (go, wait) = channel::<()>();
    let (finished, done) = channel::<()>();
    let program = scratch.program.clone();
    let writer = std::thread::spawn(move || {
        if wait.recv().is_ok() {
            install(&program);
            let _ = finished.send(());
        }
    });
    let harness = Idle {
        go,
        done,
        first: std::cell::Cell::new(true),
    };
    let (stop, text) = run(&scratch, &harness);
    drop(harness);
    writer.join().expect("the other writer ends");
    let why = refusal(&stop, &text);
    assert_unknown_origin(&why, "implementation.edit", &scratch.program, &text);
    assert!(
        scratch.program.is_file(),
        "takt never removes a program it did not write: {}\n{text}",
        scratch.program.display()
    );
}

/// A step that installs the program into that directory itself is still refused, named the
/// same way: takt cannot tell its write from another process's.
#[test]
fn a_program_the_step_installs_itself_still_refuses_it() {
    let _one = one_at_a_time();
    let scratch = scratch("the-step", "#!/bin/sh\nexit 1\n");
    let harness = Installs {
        program: scratch.program.clone(),
        first: std::cell::Cell::new(true),
    };
    let (stop, text) = run(&scratch, &harness);
    let why = refusal(&stop, &text);
    assert_unknown_origin(&why, "implementation.edit", &scratch.program, &text);
}

/// An observation whose test command writes a stray file in the tree and installs a program
/// into the binary's own program directory: the refusal says `tests.run` wrote the stray file,
/// which it is known to have, and names the program apart from it, as of unknown origin.
#[test]
fn an_observation_names_its_own_tree_write_apart_from_a_program_of_unknown_origin() {
    let _one = one_at_a_time();
    let scratch = scratch("observation", "");
    let check = format!(
        "#!/bin/sh\nprintf x > stray.txt\nprintf '#!/bin/sh\\nexit 0\\n' > '{0}'\nchmod 755 '{0}'\nexit 0\n",
        scratch.program.display()
    );
    fs::write(scratch.repo.join("check.sh"), check).expect("the check");
    git(&scratch.repo, &["add", "-A"]);
    git(&scratch.repo, &["commit", "-q", "-m", "the check"]);
    let harness = Installs {
        program: scratch.root.join("never"),
        first: std::cell::Cell::new(false),
    };
    let (stop, text) = run(&scratch, &harness);
    let why = refusal(&stop, &text);
    assert!(
        why.starts_with("tests.run wrote what its validator does not: stray.txt"),
        "story:an-unrelated-program-install-refuses-no-step: the refusal says first what tests.run is known to have written in the tree: {why}\n{text}"
    );
    assert_unknown_origin(&why, "tests.run", &scratch.program, &text);
}
