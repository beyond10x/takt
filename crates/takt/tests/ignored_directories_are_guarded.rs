//! story:ignored-directories-are-guarded. A git-ignored directory that existed before a step is
//! one entry in `git status`, so a step that planted a file inside it went unseen: a `.pyc`
//! compiled from other source, with the source's modification time and size, decided the next
//! test run. A harness step's write inside such a directory is a write: refused, naming the
//! path, and undone; the next `tests.run` reads the source again. Every repository is a scratch
//! one under the crate's build directory; nothing is written outside it.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use takt::harness::{Harness, StepContext, StepReport};
use takt::validator::{TestsRun, Validator};
use takt::{Run, RunOptions, Stop};

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

/// Writes the bytecode of a `f()` that returns 2 where Python caches `pkg/mod.py`, stamped with
/// the source's modification time and size, so Python takes it for the source's own.
const PLANT: &str = "import importlib.util, importlib._bootstrap_external as ext, os, sys
source = os.path.join(sys.argv[1], 'pkg', 'mod.py')
stat = os.stat(source)
code = compile('def f():\\n    return 2\\n', source, 'exec')
cached = importlib.util.cache_from_source(source)
with open(cached, 'wb') as out:
    out.write(ext._code_to_timestamp_pyc(code, stat.st_mtime, stat.st_size))
print(os.path.relpath(cached, sys.argv[1]))
";

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
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
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

/// The fact-checker's repository: `__pycache__/` ignored, `pkg/mod.py` whose `f()` returns 1,
/// and a test that asserts it returns 2; `pkg/__pycache__/` there already with the package's
/// byte code, as an operator's earlier run leaves it (the test command writes none:
/// `PYTHONDONTWRITEBYTECODE`, but Python reads what is there).
fn repository(name: &str) -> Scratch {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("ignored-dirs-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let repo = root.join("repo");
    fs::create_dir_all(repo.join("pkg")).expect("a directory");
    fs::write(root.join("protocol.yaml"), EDIT_AND_TEST).expect("the protocol");
    fs::write(repo.join(".gitignore"), "__pycache__/\n").expect("a file");
    fs::write(repo.join("pkg/__init__.py"), "").expect("a file");
    fs::write(repo.join("pkg/mod.py"), "def f():\n    return 1\n").expect("a file");
    fs::write(
        repo.join("test_mod.py"),
        "from pkg.mod import f\n\nassert f() == 2, f'f() returned {f()}'\nprint('PASS')\n",
    )
    .expect("a file");
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "start"]);
    let compiled = Command::new("python3")
        .args(["-I", "-m", "compileall", "-q", "pkg"])
        .current_dir(&repo)
        .status()
        .expect("python3 runs");
    assert!(compiled.success(), "pkg/ is compiled");
    Scratch { root, repo }
}

fn options(protocol: String, iterations: u64) -> RunOptions {
    RunOptions {
        protocol,
        intent: "make the test pass".to_owned(),
        inputs: Vec::new(),
        iterations: Some(iterations),
        hours: None,
        tokens: None,
        case_id: Some("planted-pyc".to_owned()),
        gate: None,
    }
}

/// A harness whose first step plants the bytecode once `pkg/__pycache__/` exists and edits
/// `notes.md`, a new revision for the next `tests.run` to observe, and whose every step reports
/// an edit.
struct Plants {
    planted: std::cell::RefCell<Option<String>>,
}

impl Harness for Plants {
    fn name(&self) -> &str {
        "plants"
    }

    fn step(&self, context: &StepContext<'_>) -> Result<StepReport, anyhow::Error> {
        if self.planted.borrow().is_none() {
            assert!(
                context.tree.join("pkg/__pycache__").is_dir(),
                "pkg/__pycache__/ is there before the step"
            );
            let output = Command::new("python3")
                .args(["-I", "-c", PLANT])
                .arg(context.tree)
                .output()
                .expect("python3 runs");
            assert!(
                output.status.success(),
                "the plant: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            *self.planted.borrow_mut() =
                Some(String::from_utf8_lossy(&output.stdout).trim().to_owned());
            fs::write(context.tree.join("notes.md"), "planted\n").expect("a write");
        }
        Ok(StepReport {
            action: "implementation.edit".to_owned(),
            summary: "made the test pass".to_owned(),
            tokens: Some(1),
        })
    }
}

/// A harness whose every step edits `notes.md` only, a new revision for the next `tests.run`.
struct Notes;

impl Harness for Notes {
    fn name(&self) -> &str {
        "notes"
    }

    fn step(&self, context: &StepContext<'_>) -> Result<StepReport, anyhow::Error> {
        fs::write(context.tree.join("notes.md"), "a note\n").expect("a write");
        Ok(StepReport {
            action: "implementation.edit".to_owned(),
            summary: "wrote a note".to_owned(),
            tokens: Some(1),
        })
    }
}

/// The `test_result` records of the case, in the order they were written.
fn results(case_dir: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(case_dir.join("evidence")) else {
        return Vec::new();
    };
    let mut paths: Vec<PathBuf> = entries.map(|e| e.expect("an entry").path()).collect();
    paths.sort();
    paths
        .iter()
        .map(|path| {
            serde_json::from_slice::<serde_json::Value>(&fs::read(path).expect("a record"))
                .expect("json")
        })
        .filter(|record| record["kind"] == "test_result")
        .map(|record| record["result"].as_str().unwrap_or_default().to_owned())
        .collect()
}

#[test]
fn a_step_that_plants_bytecode_in_an_ignored_directory_is_refused_and_the_file_removed() {
    let scratch = repository("pyc");
    let tree = &scratch.repo;
    let protocol = scratch.root.join("protocol.yaml").display().to_string();
    let validators: Vec<Box<dyn Validator>> = vec![Box::new(
        TestsRun::new("python3 test_mod.py").expect("a command"),
    )];
    let harness = Plants {
        planted: std::cell::RefCell::new(None),
    };
    let mut run =
        Run::open(tree, options(protocol.clone(), 3), &harness, &validators).expect("opens");
    let aside = scratch.root.join("aside");
    run.set_aside_in(aside.clone()).expect("a set-aside root");
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let text = String::from_utf8_lossy(&out).into_owned();
    let case_dir = run.case.dir.clone();
    let planted = harness
        .planted
        .borrow()
        .clone()
        .unwrap_or_else(|| panic!("the step never ran: {stop:?}\n{text}"));
    assert!(
        planted.starts_with("pkg/__pycache__/mod.") && planted.ends_with(".pyc"),
        "{planted}"
    );
    assert_eq!(
        results(&case_dir).first().map(String::as_str),
        Some("fail"),
        "the first tests.run fails on the source: {stop:?}\n{text}"
    );
    assert!(
        !results(&case_dir).iter().any(|result| result == "pass"),
        "the planted {planted} decided a tests.run: {:?}, {stop:?}\n{text}",
        results(&case_dir)
    );
    let stop = stop.expect("the run stops");
    assert!(
        matches!(&stop, Stop::StepRefused(why) if why.contains(&planted)),
        "the step that planted {planted} was not refused naming it: {stop:?}\n{text}"
    );
    assert!(
        !tree.join(&planted).exists(),
        "the planted {planted} is still there\n{text}"
    );
    // It rewrote the bytecode the first tests.run compiled, of which nothing was kept: moved
    // out of the tree under the case and the step, and the refusal says where.
    let step = aside.join("planted-pyc").join("step-2");
    assert!(
        step.join(&planted).is_file(),
        "the planted {planted} is not under {}\n{text}",
        step.display()
    );
    assert!(
        matches!(&stop, Stop::StepRefused(why)
            if why.contains(&takt::trace::escape(&step.display().to_string()))),
        "the refusal does not name {}: {stop:?}",
        step.display()
    );

    let before = results(&case_dir).len();
    let mut resumed = Run::resume(
        tree,
        options(protocol, 3),
        "planted-pyc",
        &Notes,
        &validators,
    )
    .expect("the case resumes");
    resumed.set_aside_in(aside).expect("a set-aside root");
    let mut out = Vec::new();
    let stop = resumed.run(&mut out);
    let text = String::from_utf8_lossy(&out).into_owned();
    let after = results(&case_dir);
    assert!(
        after.len() > before && after[before..].iter().all(|result| result == "fail"),
        "the next tests.run did not record fail: {:?} after {before} record(s), {stop:?}\n{text}",
        after
    );
}

/// A harness whose every step writes a new file into the ignored `build/` and edits `notes.md`.
struct AddsToBuild;

impl Harness for AddsToBuild {
    fn name(&self) -> &str {
        "adds-to-build"
    }

    fn step(&self, context: &StepContext<'_>) -> Result<StepReport, anyhow::Error> {
        fs::create_dir_all(context.tree.join("build/deps")).expect("a directory");
        fs::write(context.tree.join("build/deps/new.o"), "planted\n").expect("a write");
        fs::write(context.tree.join("notes.md"), "a note\n").expect("a write");
        Ok(StepReport {
            action: "implementation.edit".to_owned(),
            summary: "wrote a note".to_owned(),
            tokens: Some(1),
        })
    }
}

#[test]
fn a_step_that_adds_a_file_to_an_ignored_directory_is_refused_and_only_that_file_removed() {
    let scratch = repository("added");
    let tree = &scratch.repo;
    fs::write(tree.join(".gitignore"), "__pycache__/\nbuild/\n").expect("a file");
    git(tree, &["commit", "-q", "-am", "ignore build"]);
    fs::create_dir_all(tree.join("build")).expect("a directory");
    fs::write(tree.join("build/old.o"), "the operator's build\n").expect("a file");
    let protocol = scratch.root.join("protocol.yaml").display().to_string();
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(TestsRun::new("false").expect("a command"))];
    let mut run = Run::open(tree, options(protocol, 3), &AddsToBuild, &validators).expect("opens");
    run.set_aside_in(scratch.root.join("aside"))
        .expect("a set-aside root");
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let text = String::from_utf8_lossy(&out).into_owned();
    let stop = stop.expect("the run stops");
    assert!(
        matches!(&stop, Stop::StepRefused(why) if why.contains("build/deps/new.o")),
        "the step that wrote build/deps/new.o was not refused naming it: {stop:?}\n{text}"
    );
    assert!(
        !tree.join("build/deps").exists(),
        "what the step wrote into build/ is still there\n{text}"
    );
    assert_eq!(
        fs::read_to_string(tree.join("build/old.o")).expect("a read"),
        "the operator's build\n",
        "{text}"
    );
    assert!(!tree.join("notes.md").exists(), "{text}");
}

/// A refused `tests.run` whose command rewrote its own build output in a `target/` that was
/// there before it: the output stays where it is, as it would on a pass, and nothing is moved
/// aside; set-aside is a harness step's only.
#[test]
fn a_refused_tests_run_leaves_its_build_output_in_place_and_moves_nothing_aside() {
    use std::os::unix::fs::PermissionsExt as _;
    let scratch = repository("validator-output");
    let tree = &scratch.repo;
    fs::write(tree.join(".gitignore"), "__pycache__/\ntarget/\n").expect("a file");
    fs::write(
        tree.join("build.sh"),
        "#!/bin/sh\nprintf 'rebuilt\\n' > target/out.o\nprintf 'x\\n' > stray.txt\nexit 1\n",
    )
    .expect("a file");
    fs::set_permissions(tree.join("build.sh"), fs::Permissions::from_mode(0o755)).expect("a mode");
    git(tree, &["add", "-A"]);
    git(tree, &["commit", "-q", "-m", "a build script"]);
    fs::create_dir_all(tree.join("target")).expect("a directory");
    fs::write(tree.join("target/out.o"), "built\n").expect("a file");
    let protocol = scratch.root.join("protocol.yaml").display().to_string();
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(TestsRun::new("./build.sh").expect("a command"))];
    let mut run = Run::open(tree, options(protocol, 2), &Notes, &validators).expect("opens");
    let aside = scratch.root.join("aside");
    run.set_aside_in(aside.clone()).expect("a set-aside root");
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let text = String::from_utf8_lossy(&out).into_owned();
    assert!(
        matches!(&stop, Ok(Stop::StepRefused(why))
            if why.starts_with("tests.run") && why.contains("stray.txt")),
        "the tests.run that wrote stray.txt was not refused: {stop:?}\n{text}"
    );
    assert!(!tree.join("stray.txt").exists(), "{text}");
    assert_eq!(
        fs::read_to_string(tree.join("target/out.o"))
            .ok()
            .as_deref(),
        Some("rebuilt\n"),
        "the test command's own build output did not stay in place: {stop:?}\n{text}"
    );
    let moved: Vec<PathBuf> = walk(&aside);
    assert!(
        moved.is_empty(),
        "a refused tests.run moved {moved:?} aside: {stop:?}\n{text}"
    );
}

/// Every file under `dir`, none when it does not exist.
fn walk(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for entry in entries {
        let path = entry.expect("an entry").path();
        if path.is_dir() {
            found.extend(walk(&path));
        } else {
            found.push(path);
        }
    }
    found
}

/// A harness that counts its calls and changes nothing.
struct Counts(std::cell::Cell<u32>);

impl Harness for Counts {
    fn name(&self) -> &str {
        "counts"
    }

    fn step(&self, _: &StepContext<'_>) -> Result<StepReport, anyhow::Error> {
        self.0.set(self.0.get() + 1);
        Ok(StepReport {
            action: "implementation.edit".to_owned(),
            summary: "called".to_owned(),
            tokens: Some(1),
        })
    }
}

/// A run with no set-aside root at all (neither `HOME` nor `XDG_STATE_HOME` set) opens and
/// observes, but takes no judgement step: the first is refused before the harness is called,
/// naming the missing root. Run in a child of this test binary with its environment cleared
/// but for `PATH`, since the run reads the root from the environment when it opens.
#[test]
fn a_harness_step_with_no_set_aside_root_is_refused_before_the_harness_is_called() {
    if std::env::var_os("TAKT_TEST_NO_ROOT").is_none() {
        let mut child = Command::new(std::env::current_exe().expect("the test binary"));
        child
            .args([
                "a_harness_step_with_no_set_aside_root_is_refused_before_the_harness_is_called",
                "--exact",
                "--test-threads",
                "1",
            ])
            .env_clear()
            .env("TAKT_TEST_NO_ROOT", "1");
        if let Some(path) = std::env::var_os("PATH") {
            child.env("PATH", path);
        }
        let output = child.output().expect("the child test runs");
        assert!(
            output.status.success(),
            "the child test failed:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    assert!(takt::run::default_set_aside().is_none(), "a root is named");
    let scratch = repository("no-root");
    let protocol = scratch.root.join("protocol.yaml").display().to_string();
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(TestsRun::new("false").expect("a command"))];
    let harness = Counts(std::cell::Cell::new(0));
    let mut run = Run::open(&scratch.repo, options(protocol, 2), &harness, &validators)
        .expect("a run with no set-aside root opens");
    assert!(run.set_aside().is_none());
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let text = String::from_utf8_lossy(&out).into_owned();
    assert_eq!(
        harness.0.get(),
        0,
        "the harness was called: {stop:?}\n{text}"
    );
    assert!(
        matches!(&stop, Ok(Stop::StepRefused(why)) if why.starts_with("no set-aside directory")),
        "{stop:?}\n{text}"
    );
    assert!(
        text.contains("tests.run"),
        "the observation before it ran: {text}"
    );
}
