//! Conformance cases for the step guard around story:an-unrelated-program-install-refuses-no-step
//! (a review of wave 50's unit c). The guard refuses a step that changed what cargo or rustup
//! reads outside the tree, a program on `PATH`, or what Python imports from there, whoever wrote
//! it, and names such a change as of unknown origin. These cases check that the refusal is a
//! refusal to every reader of the case, that it repeats no content of what was written, that an
//! observation whose only write lies outside the tree is still refused, that a program changed
//! through the link a program directory holds is refused
//! (story:program-rewritten-through-a-link-refuses-the-step), and that a test's own home keeps a
//! toolchain install of another process away from its steps (acceptance 1).
//!
//! Every path written lies under this crate's `CARGO_TARGET_TMPDIR` or this binary's own program
//! directory (`support/own_home.rs`); nothing is written into a directory of the machine's home.

#[path = "support/own_home.rs"]
mod own_home;

use std::cell::Cell;
use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Mutex, MutexGuard};

use takt::harness::{Harness, StepContext, StepReport};
use takt::scorecard::Scorecard;
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

/// A marker written into a configuration file outside the tree; no refusal may repeat it.
const SENTINEL: &str = "w50-sec-sentinel-content";

/// Set in the child this binary starts for the rustup case: the directory it works in.
const CHILD: &str = "W50_SEC_RUSTUP_CHILD";

/// The cases share this binary's one program directory: they run one at a time.
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

/// This binary's own `$CARGO_HOME/bin`, at its real path, under the private directory
/// `own_home` made; fails the case otherwise.
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

/// A scratch directory under `base` holding `repo`, a git repository whose test command is
/// `check`, and the protocol file; removed on drop with every path in `extra`.
struct Scratch {
    root: PathBuf,
    repo: PathBuf,
    extra: Vec<PathBuf>,
}

impl Drop for Scratch {
    fn drop(&mut self) {
        for path in &self.extra {
            let _ = fs::remove_file(path);
        }
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn scratch_in(base: &Path, name: &str, check: &str) -> Scratch {
    let root = base.join(format!("w50-sec-{}-{name}", std::process::id()));
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
    Scratch {
        root,
        repo,
        extra: Vec::new(),
    }
}

fn scratch(name: &str, check: &str) -> Scratch {
    scratch_in(Path::new(env!("CARGO_TARGET_TMPDIR")), name, check)
}

/// An `implementation.edit` step that changes nothing in the tree; the first time it is offered
/// it lets another writer go and waits until that writer is done.
struct Idle {
    go: Sender<()>,
    done: Receiver<()>,
    first: Cell<bool>,
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

/// An `implementation.edit` step that, the first time, rewrites the program a lookup of `link`
/// finds by writing through the link, and changes nothing in the tree.
struct ThroughLink {
    link: PathBuf,
    first: Cell<bool>,
}

impl Harness for ThroughLink {
    fn name(&self) -> &str {
        "through-link"
    }

    fn step(&self, context: &StepContext<'_>) -> Result<StepReport, anyhow::Error> {
        if !context.offered.iter().any(|a| a == "implementation.edit") {
            anyhow::bail!("implementation.edit not offered");
        }
        if self.first.replace(false) {
            fs::write(
                &self.link,
                "#!/bin/sh\necho rewritten by the step\nexit 0\n",
            )
            .expect("a write through the link");
        }
        Ok(StepReport {
            action: "implementation.edit".to_owned(),
            summary: "changed nothing in the tree".to_owned(),
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

/// Runs `scratch` under an [`Idle`] step while `write` runs on another thread during it.
fn run_with_writer(
    scratch: &Scratch,
    write: impl FnOnce() + Send + 'static,
) -> (anyhow::Result<Stop>, String) {
    let (go, wait) = channel::<()>();
    let (finished, done) = channel::<()>();
    let writer = std::thread::spawn(move || {
        if wait.recv().is_ok() {
            write();
            let _ = finished.send(());
        }
    });
    let harness = Idle {
        go,
        done,
        first: Cell::new(true),
    };
    let ran = run(scratch, &harness);
    drop(harness);
    writer.join().expect("the other writer ends");
    ran
}

fn shown(stop: &anyhow::Result<Stop>) -> String {
    format!("{:?}", stop.as_ref().map_err(ToString::to_string))
}

/// The one case directory of `repo`'s run.
fn case_dir(repo: &Path) -> PathBuf {
    let cases = repo.join(".takt/cases");
    let mut dirs: Vec<PathBuf> = fs::read_dir(&cases)
        .unwrap_or_else(|error| panic!("read {}: {error}", cases.display()))
        .map(|entry| entry.expect("an entry").path())
        .filter(|path| path.is_dir())
        .collect();
    assert_eq!(
        dirs.len(),
        1,
        "one case under {}: {dirs:?}",
        cases.display()
    );
    dirs.remove(0)
}

/// The safety fact the unit rests on, run end to end: a step during which another writer
/// changes a configuration file cargo reads above the tree (with a marker in it) and installs a
/// program is refused, named of unknown origin, and every reader of the case takes it as a
/// refusal: the `refusal` line holds the stop's reason, the last `stop` line is `StepRefused`,
/// the scorecard ranks the run as a refused one, never as accepted. The configuration file is
/// put back and the program left; no reason, console line or record repeats the file's content.
#[test]
fn an_unattributed_change_is_a_refusal_to_every_reader_and_repeats_no_content() {
    let _one = one_at_a_time();
    let mut scratch = scratch("readers", "#!/bin/sh\nexit 1\n");
    let program = own_program_dir().join(format!("w50-sec-readers-{}", std::process::id()));
    let _ = fs::remove_file(&program);
    scratch.extra.push(program.clone());
    let config_dir = scratch.root.join(".cargo");
    let config = config_dir.join("config.toml");
    let (written_program, written_config) = (program.clone(), config.clone());
    let (stop, text) = run_with_writer(&scratch, move || {
        fs::create_dir_all(&config_dir).expect("the configuration directory");
        fs::write(
            &written_config,
            format!("# {SENTINEL}\n[build]\njobs = 1\n"),
        )
        .expect("a configuration file");
        install(&written_program);
    });
    let why = match &stop {
        Ok(Stop::StepRefused(why)) => why.clone(),
        _ => panic!("the run stops StepRefused, not {}\n{text}", shown(&stop)),
    };
    let (_, after) = why
        .split_once(UNKNOWN)
        .unwrap_or_else(|| panic!("the refusal names a change of unknown origin: {why}"));
    for path in [&config, &program] {
        assert!(
            after.contains(&path.display().to_string()),
            "the refusal names {} as of unknown origin: {why}",
            path.display()
        );
    }
    assert!(
        !config.exists(),
        "the configuration file cargo reads was put back: {}",
        config.display()
    );
    assert!(program.is_file(), "the program is left as it was written");
    let dir = case_dir(&scratch.repo);
    let record = fs::read_to_string(dir.join("events.jsonl")).expect("the case's record");
    for (what, said) in [
        ("the reason", why.as_str()),
        ("the console", &text),
        ("the record", &record),
    ] {
        assert!(
            !said.contains(SENTINEL),
            "{what} repeats the content of a file outside the tree: {said}"
        );
    }
    let lines: Vec<serde_json::Value> = record
        .lines()
        .map(|line| serde_json::from_str(line).expect("a record line"))
        .collect();
    assert!(
        lines
            .iter()
            .any(|line| line["event"] == "refusal" && line["reason"].as_str() == Some(why.as_str())),
        "a refusal line holds the stop's reason:\n{record}"
    );
    let last_stop = lines
        .iter()
        .rev()
        .find(|line| line["event"] == "stop")
        .and_then(|line| line["stop"].as_str())
        .unwrap_or_default();
    assert!(
        last_stop.starts_with("StepRefused"),
        "the record's stop is StepRefused, not {last_stop:?}:\n{record}"
    );
    let card = Scorecard::read(&scratch.repo).expect("the scorecard reads");
    assert_eq!(card.rows.len(), 1, "one run: {:?}", card.rows);
    let row = &card.rows[0];
    assert_eq!(row.stop, "StepRefused", "{row:?}");
    assert!(
        row.refusals >= 1,
        "the scorecard counts the refusal: {row:?}"
    );
    assert!(
        takt::scorecard::rank(&row.outcome) < takt::scorecard::rank("accepted"),
        "the scorecard ranks the run below an accepted one: {row:?}"
    );
}

/// An observation whose test command writes nothing in the tree and installs a program into
/// this binary's own program directory is refused, its reason led by the action and naming the
/// program as of unknown origin and as not put back, never as written by `tests.run`. With no
/// write in the tree, only the program's change makes the guard refuse: this measures the
/// condition the unit moved out of the list of what the observation wrote.
#[test]
fn an_observation_whose_only_write_is_a_program_is_refused_as_of_unknown_origin() {
    let _one = one_at_a_time();
    let mut scratch = scratch("observation-only", "");
    let program = own_program_dir().join(format!("w50-sec-observation-{}", std::process::id()));
    let _ = fs::remove_file(&program);
    scratch.extra.push(program.clone());
    let check = format!(
        "#!/bin/sh\nprintf '#!/bin/sh\\nexit 0\\n' > '{0}'\nchmod 755 '{0}'\nexit 0\n",
        program.display()
    );
    fs::write(scratch.repo.join("check.sh"), check).expect("the check");
    git(&scratch.repo, &["add", "-A"]);
    git(&scratch.repo, &["commit", "-q", "-m", "the check"]);
    let harness = ThroughLink {
        link: scratch.root.join("never"),
        first: Cell::new(false),
    };
    let (stop, text) = run(&scratch, &harness);
    let why = match &stop {
        Ok(Stop::StepRefused(why)) => why.clone(),
        _ => panic!("the run stops StepRefused, not {}\n{text}", shown(&stop)),
    };
    let program_shown = program.display().to_string();
    assert!(
        why.starts_with("tests.run: what cargo or rustup reads outside the tree"),
        "the reason leads with the action and what changed: {why}"
    );
    assert!(
        !why.contains("wrote what its validator does not"),
        "nothing in the tree was written, so nothing is said to be: {why}"
    );
    let (_, after) = why
        .split_once(UNKNOWN)
        .unwrap_or_else(|| panic!("the refusal names a change of unknown origin: {why}"));
    assert!(after.contains(&program_shown), "{why}");
    let (_, left) = why
        .split_once("not put back")
        .unwrap_or_else(|| panic!("the program is named as not put back: {why}"));
    assert!(left.contains(&program_shown), "{why}");
    assert!(
        program.is_file(),
        "a program directory's content is never removed"
    );
    assert!(
        text.contains("nothing recorded"),
        "the observation records nothing:\n{text}"
    );
}

/// A program directory on `PATH` that holds a link to a program elsewhere (as a dotfile or
/// tool manager installs one): a step that rewrites the program through that link changes what
/// a lookup by the name finds, as surely as a new file there would, and is refused
/// (story:program-rewritten-through-a-link-refuses-the-step). The guard reads such a link with
/// what it resolves to, so the rewrite behind it differs; the refusal names the program by the
/// link a lookup finds it at, as of unknown origin and as not put back, and the program is left
/// as the step wrote it.
#[test]
fn a_program_rewritten_through_its_link_on_path_during_a_step_is_refused() {
    let _one = one_at_a_time();
    let mut scratch = scratch("through-link", "#!/bin/sh\nexit 1\n");
    let target = scratch.root.join("tool").join("probe");
    fs::create_dir_all(target.parent().expect("a parent")).expect("the tool's directory");
    install(&target);
    let link = own_program_dir().join(format!("w50-sec-linked-{}", std::process::id()));
    let _ = fs::remove_file(&link);
    std::os::unix::fs::symlink(&target, &link).expect("a link on PATH");
    scratch.extra.push(link.clone());
    let harness = ThroughLink {
        link: link.clone(),
        first: Cell::new(true),
    };
    let (stop, text) = run(&scratch, &harness);
    let rewritten = fs::read_to_string(&target).unwrap_or_default();
    assert!(
        rewritten.contains("rewritten by the step"),
        "the step rewrote the program behind the link: {rewritten}"
    );
    let why = match &stop {
        Ok(Stop::StepRefused(why)) => why.clone(),
        _ => panic!(
            "a step that rewrote the program {} finds through {} on PATH stops the run \
             StepRefused, not {}\n{text}",
            target.display(),
            link.display(),
            shown(&stop)
        ),
    };
    let link_shown = link.display().to_string();
    let (_, after) = why
        .split_once(UNKNOWN)
        .unwrap_or_else(|| panic!("the refusal names a change of unknown origin: {why}"));
    assert!(
        after.contains(&link_shown),
        "the refusal names the program by its link {link_shown}: {why}"
    );
    let (_, left) = why
        .split_once("not put back")
        .unwrap_or_else(|| panic!("the program is named as not put back: {why}"));
    assert!(left.contains(&link_shown), "{why}");
    assert_eq!(
        fs::read_link(&link).ok().as_deref(),
        Some(target.as_path()),
        "the link is left as it was"
    );
}

/// Acceptance 1 of story:an-unrelated-program-install-refuses-no-step: a test that runs steps
/// under the guard has a home of its own, so a write into the home's program directories by
/// another process does not reach its steps. The guard reads `$RUSTUP_HOME/toolchains/*/bin`
/// among them (`program_dirs` in `crates/takt/src/run.rs`), and a toolchain or component install
/// (a `rust-toolchain.toml` another repository pins, `rustup component add`) writes there. This
/// binary starts itself again with `own_home` not yet in place and `RUSTUP_HOME` at a stand-in
/// for the home's rustup directory under `CARGO_TARGET_TMPDIR`; in the child, another writer
/// adds a program to a toolchain's `bin` while a step runs, and the run is not refused for it.
#[test]
fn a_toolchain_install_into_the_rustup_home_during_a_step_does_not_reach_a_test() {
    if let Some(base) = std::env::var_os(CHILD).map(PathBuf::from) {
        child_rustup_case(&base);
        return;
    }
    let _one = one_at_a_time();
    let base = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("w50-sec-{}-rustup", std::process::id()));
    let _ = fs::remove_dir_all(&base);
    let bin = base.join("rustup/toolchains/probe-toolchain/bin");
    fs::create_dir_all(&bin).expect("the stand-in toolchain");
    install(&bin.join("rustc"));
    let exe = std::env::current_exe().expect("this test binary");
    let output = Command::new(exe)
        .args([
            "a_toolchain_install_into_the_rustup_home_during_a_step_does_not_reach_a_test",
            "--exact",
            "--nocapture",
            "--test-threads=1",
        ])
        .env_remove("TAKT_TEST_OWN_HOME")
        .env("RUSTUP_HOME", base.join("rustup"))
        .env(CHILD, &base)
        .output()
        .expect("the child runs");
    let _ = fs::remove_dir_all(&base);
    assert!(
        output.status.success(),
        "the child case failed ({}):\n{}\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// The child's half of the rustup case: `own_home` has run in this process with `RUSTUP_HOME`
/// at the stand-in; another writer adds `cargo-clippy` to the stand-in's toolchain (the home's
/// rustup, not the copy `own_home` made of it) while a step runs.
fn child_rustup_case(base: &Path) {
    let rustup = base.join("rustup");
    assert!(
        std::env::var_os("RUSTUP_HOME").is_some_and(|own| own != rustup.as_os_str()),
        "own_home gives the child a rustup home of its own in place of the stand-in {}",
        rustup.display()
    );
    let probe = rustup.join("toolchains/probe-toolchain/bin/cargo-clippy");
    let scratch = scratch_in(base, "child", "#!/bin/sh\nexit 1\n");
    let written = probe.clone();
    let (stop, text) = run_with_writer(&scratch, move || install(&written));
    let probe_shown = probe.display().to_string();
    assert!(
        !matches!(&stop, Ok(Stop::StepRefused(why)) if why.contains(&probe_shown)),
        "story:an-unrelated-program-install-refuses-no-step acceptance 1: a program another \
         process installs into the rustup home ({}) while a test's step runs reaches the step; \
         own_home left RUSTUP_HOME at {}; the run stopped {}\n{text}",
        probe.display(),
        rustup.display(),
        shown(&stop)
    );
}
