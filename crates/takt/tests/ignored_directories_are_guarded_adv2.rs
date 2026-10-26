//! Review pass 2 against story:ignored-directories-are-guarded: setting a harness step's
//! rewrites aside. Every way a harness step is discarded names where each moved file went; a
//! set-aside that fails still lets everything else be put back; a step directory is made new,
//! never shared with a run before under the same case id; a run does not open without a usable
//! set-aside root; a validator's own build output stays. Every repository is a scratch one under
//! the crate's build directory, and every run sets its own set-aside root beside it; nothing is
//! written outside it.

use std::fs;
use std::os::unix::ffi::OsStrExt as _;
use std::os::unix::fs::PermissionsExt as _;
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

/// A scratch root under this crate's build directory; dropping it makes it writable again and
/// removes it.
struct Scratch {
    root: PathBuf,
    repo: PathBuf,
}

impl Scratch {
    /// The set-aside root every run here is given.
    fn aside_root(&self) -> PathBuf {
        self.root.join("aside")
    }

    /// Where a refused step's files inside an ignored directory go for the case `rv` and the
    /// step `step`, when no run before took that directory.
    fn aside(&self, step: u64) -> PathBuf {
        self.aside_root().join("rv").join(format!("step-{step}"))
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

/// A committed repository with `ignore` as its `.gitignore` and one tracked file.
fn repository(name: &str, ignore: &str) -> Scratch {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("ignored-dirs-adv2-{}-{name}", std::process::id()));
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

/// A harness whose first step does `attack` (which may fail the step) and whose every step
/// edits `notes.md`.
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

fn options(protocol: String) -> RunOptions {
    RunOptions {
        protocol,
        intent: "x".to_owned(),
        inputs: Vec::new(),
        iterations: Some(2),
        hours: None,
        tokens: None,
        case_id: Some("rv".to_owned()),
        gate: None,
    }
}

/// A run of the case `rv`, its set-aside root the scratch one: a failing `tests.run` (step 1),
/// then the harness's step (step 2).
fn go(scratch: &Scratch, attack: Attack) -> (anyhow::Result<Stop>, String) {
    let protocol = scratch.root.join("protocol.yaml").display().to_string();
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(TestsRun::new("false").expect("a command"))];
    let harness = Harn {
        attack,
        done: std::cell::Cell::new(false),
    };
    let mut run =
        Run::open(&scratch.repo, options(protocol), &harness, &validators).expect("opens");
    run.set_aside_in(scratch.aside_root())
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

/// How the run names a directory: [`takt::git::shown`].
fn named(dir: &Path) -> String {
    takt::git::shown(dir.as_os_str())
}

/// R1: a harness step that writes an operator's file below an ignored directory with the same
/// bytes and then fails: the file is moved aside, and the error the run ends on names where.
#[test]
fn a_failed_harness_step_names_where_it_moved_the_operators_file() {
    let s = repository("failed", "build/\n");
    fs::create_dir_all(s.repo.join("build")).expect("a directory");
    fs::write(s.repo.join("build/old.o"), "AAAA").expect("a file");
    let (stop, text) = go(
        &s,
        Box::new(|tree: &Path| {
            fs::write(tree.join("build/old.o"), "AAAA").expect("a write");
            anyhow::bail!("the harness failed (max turns)")
        }),
    );
    let said = said(&stop);
    let aside = s.aside(2);
    assert_eq!(
        fs::read_to_string(aside.join("build/old.o"))
            .ok()
            .as_deref(),
        Some("AAAA"),
        "not moved aside: {said}"
    );
    assert!(
        said.contains(&named(&aside)) && said.contains("build/old.o"),
        "the operator's build/old.o went to {}, and the run's error does not say so: {said}\n{text}",
        named(&aside)
    );
}

/// R2: the set-aside root is made unusable during the step (a file put where it would be made):
/// what the step planted is still removed, the ignored file git names still put back, and the
/// refusal names the file that could not be moved.
#[test]
fn a_set_aside_that_fails_still_discards_the_plant() {
    let s = repository("unusable", ".env\nbuild/\n");
    fs::create_dir_all(s.repo.join("build")).expect("a directory");
    fs::write(s.repo.join("build/old.o"), "AAAA").expect("a file");
    fs::write(s.repo.join(".env"), "SECRET=1\n").expect("a file");
    let blocker = s.aside_root();
    let (stop, text) = go(
        &s,
        Box::new(move |tree: &Path| {
            fs::write(&blocker, "in the way").expect("a file");
            fs::write(tree.join("build/old.o"), "POISON").expect("a write");
            fs::write(tree.join("build/new.pyc"), "PLANT").expect("a write");
            fs::write(tree.join(".env"), "SECRET=evil\n").expect("a write");
            Ok(())
        }),
    );
    let said = said(&stop);
    assert!(
        !s.repo.join("build/new.pyc").exists(),
        "the planted build/new.pyc is still there: {said}\n{text}"
    );
    assert_eq!(
        fs::read_to_string(s.repo.join(".env")).ok().as_deref(),
        Some("SECRET=1\n"),
        "{said}\n{text}"
    );
    assert!(
        matches!(&stop, Ok(Stop::StepRefused(why))
            if why.contains("could not be moved out of the tree") && why.contains("build/old.o")),
        "the refusal does not name what could not be moved: {said}\n{text}"
    );
}

/// R3: the step writes, by absolute path, into the directory its own set-aside would take: that
/// directory is a run before's to the run, never shared; the rewrite goes to a new one, the
/// refusal names it, and what was there is left alone.
#[test]
fn a_set_aside_directory_that_exists_is_never_shared() {
    let s = repository("collide", "build/\n");
    fs::create_dir_all(s.repo.join("build")).expect("a directory");
    fs::write(s.repo.join("build/old.o"), "AAAA").expect("a file");
    let decoy = s.aside(2).join("build/old.o");
    let planted = decoy.clone();
    let (stop, text) = go(
        &s,
        Box::new(move |tree: &Path| {
            fs::create_dir_all(planted.parent().expect("a parent")).expect("a directory");
            fs::write(&planted, "decoy").expect("a write");
            fs::write(tree.join("build/old.o"), "POISON").expect("a write");
            fs::write(tree.join("build/new.pyc"), "PLANT").expect("a write");
            Ok(())
        }),
    );
    let said = said(&stop);
    let fresh = s.aside_root().join("rv/step-2-2");
    assert!(!s.repo.join("build/new.pyc").exists(), "{said}\n{text}");
    assert!(!s.repo.join("build/old.o").exists(), "{said}\n{text}");
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
        "{said}\n{text}"
    );
    assert!(
        matches!(&stop, Ok(Stop::StepRefused(why)) if why.contains(&named(&fresh))),
        "the refusal does not name {}: {said}",
        named(&fresh)
    );
}

/// R4: a run takes no set-aside root it could not use: a relative one, one inside the tree,
/// through a link or not, or one a file stands in the way of; it keeps the one it had. A run
/// given none opens with the default.
#[test]
fn a_run_takes_no_set_aside_root_it_could_not_use() {
    let s = repository("roots", "build/\n");
    let protocol = s.root.join("protocol.yaml").display().to_string();
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(TestsRun::new("false").expect("a command"))];
    let harness = Harn {
        attack: Box::new(|_| Ok(())),
        done: std::cell::Cell::new(false),
    };
    let mut run = Run::open(&s.repo, options(protocol), &harness, &validators).expect("opens");
    let resolved = |dir: &Path| takt::git::set_aside_root(dir, &s.repo).expect("usable");
    assert_eq!(
        run.set_aside().map(Path::to_path_buf),
        takt::run::default_set_aside().map(|dir| resolved(&dir)),
        "a run given no set-aside root opens with the default, resolved"
    );
    let default = run.set_aside().map(Path::to_path_buf);
    fs::write(s.root.join("a-file"), "x").expect("a file");
    std::os::unix::fs::symlink(&s.repo, s.root.join("into-repo")).expect("a link");
    for refused in [
        PathBuf::from("relative/aside"),
        PathBuf::from("./aside"),
        s.repo.join("build/aside"),
        s.root.join("into-repo/aside"),
        s.root.join("a-file/aside"),
    ] {
        assert!(
            run.set_aside_in(refused.clone()).is_err(),
            "{} was taken",
            refused.display()
        );
        assert_eq!(
            run.set_aside().map(Path::to_path_buf),
            default,
            "{}",
            refused.display()
        );
    }
    run.set_aside_in(s.aside_root()).expect("taken");
    assert_eq!(run.set_aside(), Some(resolved(&s.aside_root()).as_path()));
}

/// R5: a refused tests.run: what it created under target/ is removed, what it rewrote stays,
/// an operator's file it did not touch stays, nothing is moved aside.
#[test]
fn a_refused_tests_run_removes_what_it_created_below_target() {
    let s = repository("validator", "target/\n");
    fs::write(
        s.repo.join("build.sh"),
        "#!/bin/sh\nprintf 'rebuilt\\n' > target/out.o\nprintf 'new\\n' > target/new.o\nprintf 'x\\n' > stray.txt\nexit 1\n",
    )
    .expect("a file");
    fs::set_permissions(s.repo.join("build.sh"), fs::Permissions::from_mode(0o755))
        .expect("a mode");
    git(&s.repo, &["add", "-A"]);
    git(&s.repo, &["commit", "-q", "-m", "build"]);
    fs::create_dir_all(s.repo.join("target")).expect("a directory");
    fs::write(s.repo.join("target/out.o"), "built\n").expect("a file");
    fs::write(s.repo.join("target/keep.o"), "mine\n").expect("a file");
    let protocol = s.root.join("protocol.yaml").display().to_string();
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(TestsRun::new("./build.sh").expect("a command"))];
    let harness = Harn {
        attack: Box::new(|_| Ok(())),
        done: std::cell::Cell::new(false),
    };
    let mut run = Run::open(&s.repo, options(protocol), &harness, &validators).expect("opens");
    run.set_aside_in(s.aside_root()).expect("a set-aside root");
    let mut out = Vec::new();
    run.continue_past_red_base();
    let stop = run.run(&mut out);
    let said = said(&stop);
    assert!(
        matches!(&stop, Ok(Stop::StepRefused(why)) if why.starts_with("tests.run")),
        "{said}"
    );
    assert!(!s.repo.join("target/new.o").exists(), "{said}");
    assert_eq!(
        fs::read_to_string(s.repo.join("target/out.o"))
            .ok()
            .as_deref(),
        Some("rebuilt\n")
    );
    assert_eq!(
        fs::read_to_string(s.repo.join("target/keep.o"))
            .ok()
            .as_deref(),
        Some("mine\n")
    );
    assert!(
        !s.aside_root().exists(),
        "something was moved aside: {said}"
    );
}

/// R6: a name holding the text `\xff` and one holding the byte 0xff read apart.
#[test]
fn shown_tells_a_backslash_name_from_a_byte() {
    let text = takt::git::shown(std::ffi::OsStr::from_bytes(b"build/\\xffx.o"));
    let byte = takt::git::shown(std::ffi::OsStr::from_bytes(b"build/\xffx.o"));
    assert_ne!(text, byte, "both read {text}");
}
