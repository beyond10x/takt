//! Adversary cases for story:an-unrelated-program-install-refuses-no-step, acceptance 1: a test
//! that runs steps under the guard reads program directories of its own, so a write to the real
//! home's program directories does not reach it.
//!
//! The step guard reads, beside `PATH` and `$CARGO_HOME/bin`, every `bin` of a rustup toolchain
//! (`$RUSTUP_HOME/toolchains/*/bin`, else `~/.rustup`; `program_dirs` in `crates/takt/src/run.rs`).
//! `support/own_home.rs` replaces `PATH` and `CARGO_HOME` and leaves `RUSTUP_HOME` and `HOME`
//! as they are, so a toolchain another process installs or updates while a test runs reaches its
//! steps. Every write here lands under this crate's build directory, never in the machine's home.

#[path = "support/own_home.rs"]
mod own_home;

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{Receiver, Sender, channel};

use takt::harness::{Harness, StepContext, StepReport};
use takt::validator::{TestsRun, Validator};
use takt::{Run, RunOptions, Stop};

const STORY: &str = "story:an-unrelated-program-install-refuses-no-step";

/// The variable that hands the child run its private home.
const CHILD_HOME: &str = "W50_ADV_C_HOME";

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

fn own_root() -> PathBuf {
    let root = std::env::var_os("TAKT_TEST_OWN_HOME")
        .map(PathBuf::from)
        .expect("TAKT_TEST_OWN_HOME: this test binary has no program directories of its own");
    fs::canonicalize(root).expect("the private directory exists")
}

fn writable(dir: &Path) -> bool {
    use std::os::unix::ffi::OsStrExt as _;
    let Ok(path) = std::ffi::CString::new(dir.as_os_str().as_bytes()) else {
        return false;
    };
    // SAFETY: `path` is a valid NUL-terminated string that outlives the call.
    unsafe { libc::access(path.as_ptr(), 2) == 0 }
}

/// The rustup toolchain `bin` directories the step guard reads in this process, as
/// `program_dirs` finds them: `$RUSTUP_HOME`, else `~/.rustup`, then `toolchains/*/bin`.
fn rustup_bins() -> Vec<PathBuf> {
    let rustup = std::env::var_os("RUSTUP_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".rustup")));
    let Some(Ok(toolchains)) = rustup.map(|rustup| fs::read_dir(rustup.join("toolchains"))) else {
        return Vec::new();
    };
    toolchains
        .flatten()
        .filter_map(|toolchain| fs::canonicalize(toolchain.path().join("bin")).ok())
        .filter(|bin| bin.is_dir() && writable(bin))
        .collect()
}

/// Acceptance 1, read off the guard's inputs: no rustup toolchain `bin` the guard reads in a
/// test binary is a directory another process may write.
#[test]
fn no_rustup_toolchain_bin_the_guard_reads_is_shared_with_another_process() {
    let root = own_root();
    let shared: Vec<String> = rustup_bins()
        .into_iter()
        .filter(|bin| !bin.starts_with(&root))
        .map(|bin| bin.display().to_string())
        .collect();
    assert!(
        shared.is_empty(),
        "{STORY}, acceptance 1: the step guard of every test reads rustup toolchain program directories another process can write (rustup update, a toolchain install): {shared:?}"
    );
}

/// Acceptance 1, end to end: a test binary started under a home whose rustup holds one
/// toolchain, both private and under this crate's build directory, as the real home's rustup
/// is to every test today. Another writer installs a program into that toolchain's `bin`
/// while an idle step runs; the step must not be refused for it.
#[test]
fn a_toolchain_program_another_process_installs_reaches_no_step_of_a_test() {
    let base = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .parent()
        .expect("the build directory")
        .join("scratch")
        .join(format!("w50-adv-c-{}", std::process::id()));
    let _ = fs::remove_dir_all(&base);
    let home = base.join("home");
    fs::create_dir_all(home.join(".rustup/toolchains/probe/bin")).expect("a private rustup");
    let output = Command::new(std::env::current_exe().expect("this test binary"))
        .args([
            "--exact",
            "the_rustup_home_run",
            "--nocapture",
            "--test-threads",
            "1",
        ])
        .env_remove("TAKT_TEST_OWN_HOME")
        .env_remove("CARGO_HOME")
        .env("HOME", &home)
        .env("RUSTUP_HOME", home.join(".rustup"))
        .env(CHILD_HOME, &home)
        .output()
        .expect("the test binary runs");
    let _ = fs::remove_dir_all(&base);
    let text = String::from_utf8_lossy(&output.stdout) + String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{text}");
    assert!(
        text.contains("1 passed"),
        "the inner run did not run: {text}"
    );
}

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

/// The run [`a_toolchain_program_another_process_installs_reaches_no_step_of_a_test`] starts;
/// nothing at all without its home.
#[test]
fn the_rustup_home_run() {
    let Some(home) = std::env::var_os(CHILD_HOME).map(PathBuf::from) else {
        return;
    };
    // The home's toolchain `bin`, which the guard read before `own_home` gave this process a
    // rustup home of its own in its place.
    let bin =
        fs::canonicalize(home.join(".rustup/toolchains/probe/bin")).expect("the private toolchain");
    assert!(
        bin.starts_with(fs::canonicalize(&home).expect("the home")),
        "the other writer writes into the home's rustup, {}",
        bin.display()
    );
    let root = home.parent().expect("the scratch").to_path_buf();
    let repo = root.join("repo");
    fs::create_dir_all(&repo).expect("a directory");
    fs::write(root.join("protocol.yaml"), EDIT_AND_TEST).expect("the protocol");
    fs::write(repo.join(".gitignore"), "target/\n").expect("a file");
    fs::write(repo.join("check.sh"), "#!/bin/sh\nexit 1\n").expect("the check");
    fs::set_permissions(repo.join("check.sh"), fs::Permissions::from_mode(0o755)).expect("a mode");
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "start"]);
    let program = bin.join("takt-w50-probe");
    let (go, wait) = channel::<()>();
    let (finished, done) = channel::<()>();
    let writer = {
        let program = program.clone();
        std::thread::spawn(move || {
            if wait.recv().is_ok() {
                fs::write(&program, "#!/bin/sh\nexit 0\n").expect("a program");
                fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).expect("a mode");
                let _ = finished.send(());
            }
        })
    };
    let harness = Idle {
        go,
        done,
        first: std::cell::Cell::new(true),
    };
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(TestsRun::new("./check.sh").expect("a command"))];
    let options = RunOptions {
        protocol: root.join("protocol.yaml").display().to_string(),
        intent: "make the check pass".to_owned(),
        inputs: Vec::new(),
        iterations: Some(4),
        hours: None,
        tokens: None,
        case_id: None,
        gate: None,
    };
    let mut run = Run::open(&repo, options, &harness, &validators).expect("the case opens");
    run.continue_past_red_base();
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    drop(run);
    drop(harness);
    writer.join().expect("the other writer ends");
    let text = String::from_utf8_lossy(&out);
    assert!(
        program.is_file(),
        "the other writer installed {}",
        program.display()
    );
    assert!(
        !matches!(&stop, Ok(Stop::StepRefused(why)) if why.contains(&program.display().to_string())),
        "{STORY}, acceptance 1: a program another process installed into the home's rustup toolchain bin refused a test's step: {:?}\n{text}",
        stop.as_ref().map_err(ToString::to_string)
    );
}
