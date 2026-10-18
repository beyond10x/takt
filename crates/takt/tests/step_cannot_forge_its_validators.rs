//! story:step-cannot-forge-its-validators: a judgement step cannot change what decides it. A
//! step whose change touches the judge surface (`.cargo/**`, `.github/**`, `.engineering/**`
//! but the cases, `rust-toolchain*`, `build.rs`, and the test command's own script) is refused
//! and put back, whatever the case of the path and whether it is reached through a link the
//! base holds; `tests.run` and `ci.read` record nothing while the case's branch carries a
//! judge-surface change that reached it by another route, and name the paths; and a validator's
//! observation that leaves anything on the judge surface (a FIFO, an ignored file) is refused
//! and put back like a step.

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

use takt::case::{Branch, current};
use takt::forge::{Checks, Head, Memory, PullRequest};
use takt::harness::{Harness, StepContext, StepReport};
use takt::validator::{CiRead, TestsRun, Validator};
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

/// The check every repository here starts with: it fails.
const FAILING_CHECK: &str = "#!/bin/sh\nexit 1\n";

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

/// A scratch directory under this crate's build directory: the protocol file at its root and
/// the repository in `repo/`. Dropping it removes both.
struct Scratch {
    root: PathBuf,
    repo: PathBuf,
}

impl std::ops::Deref for Scratch {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.repo
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

impl Scratch {
    fn protocol(&self) -> String {
        self.root.join("protocol.yaml").display().to_string()
    }
}

/// A repository on `main` holding `files` (path, text, executable) in its first commit, beside
/// a `.gitignore` that ignores `target/`.
fn repository(name: &str, files: &[(&str, &str, bool)]) -> Scratch {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("step-forge-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let repo = root.join("repo");
    fs::create_dir_all(&repo).expect("a directory");
    fs::write(root.join("protocol.yaml"), EDIT_AND_TEST).expect("the protocol");
    fs::write(repo.join(".gitignore"), "target/\n").expect("a file");
    for (path, text, executable) in files {
        let at = repo.join(path);
        fs::create_dir_all(at.parent().expect("a parent")).expect("a directory");
        fs::write(&at, text).expect("a file");
        if *executable {
            fs::set_permissions(&at, fs::Permissions::from_mode(0o755)).expect("a mode");
        }
    }
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "start"]);
    Scratch { root, repo }
}

fn options(protocol: String, iterations: u64) -> RunOptions {
    RunOptions {
        protocol,
        intent: "make the check pass".to_owned(),
        inputs: Vec::new(),
        iterations: Some(iterations),
        hours: None,
        tokens: None,
        case_id: None,
        gate: None,
    }
}

/// An `implementation.edit` step that does `act` to the tree the first time it is offered, and
/// changes nothing after.
struct Acts {
    act: fn(&Path),
    done: std::cell::Cell<bool>,
}

impl Acts {
    fn new(act: fn(&Path)) -> Self {
        Self {
            act,
            done: std::cell::Cell::new(false),
        }
    }
}

impl Harness for Acts {
    fn name(&self) -> &str {
        "acts"
    }

    fn step(&self, context: &StepContext<'_>) -> Result<StepReport, anyhow::Error> {
        if !context.offered.iter().any(|a| a == "implementation.edit") {
            anyhow::bail!("implementation.edit not offered");
        }
        if !self.done.replace(true) {
            (self.act)(context.tree);
        }
        Ok(StepReport {
            action: "implementation.edit".to_owned(),
            summary: "made the check pass".to_owned(),
            tokens: Some(1),
        })
    }
}

/// What one run left: its stop, what it printed, the case's evidence records.
struct Ran {
    stop: anyhow::Result<Stop>,
    text: String,
    records: Vec<serde_json::Value>,
}

impl Ran {
    /// The `test_result` records whose result is `pass`.
    fn passes(&self) -> Vec<&serde_json::Value> {
        self.records
            .iter()
            .filter(|r| r["kind"] == "test_result" && r["result"] == "pass")
            .collect()
    }

    /// The refusal the run stopped on; fails the case on any other stop.
    fn refusal(&self) -> &str {
        match &self.stop {
            Ok(Stop::StepRefused(why)) => why,
            other => panic!(
                "the run is refused, not {other:?}; records {:?}\n{}",
                self.records, self.text
            ),
        }
    }
}

fn records(case_dir: &Path) -> Vec<serde_json::Value> {
    let Ok(entries) = fs::read_dir(case_dir.join("evidence")) else {
        return Vec::new();
    };
    let mut paths: Vec<PathBuf> = entries.map(|e| e.expect("an entry").path()).collect();
    paths.sort();
    paths
        .iter()
        .map(|path| serde_json::from_slice(&fs::read(path).expect("a record")).expect("json"))
        .collect()
}

/// Runs the edit-and-test protocol on `tree` with `command` as the test command and a step that
/// does `act`.
fn run(tree: &Scratch, command: &str, act: fn(&Path)) -> Ran {
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(TestsRun::new(command).expect("a command"))];
    let harness = Acts::new(act);
    let mut run = Run::open(tree, options(tree.protocol(), 4), &harness, &validators)
        .expect("the case opens");
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    Ran {
        stop,
        text: String::from_utf8_lossy(&out).into_owned(),
        records: records(&run.case.dir),
    }
}

/// The first commit's revision.
fn first(tree: &Path) -> String {
    git(tree, &["rev-list", "--max-parents=0", "HEAD"])
}

/// Asserts the step that wrote `path` was refused and put back: the refusal names it, nothing
/// passed, the case's branch holds no commit beyond the base, and the path is as the base has
/// it; nothing else is left but the cases and a build's output under `target/`, which the test
/// command makes.
fn assert_refused_and_put_back(tree: &Scratch, ran: &Ran, path: &str) {
    let why = ran.refusal();
    assert!(
        why.contains(path),
        "the refusal names {path}: {why}\n{}",
        ran.text
    );
    assert!(ran.passes().is_empty(), "{:?}\n{}", ran.records, ran.text);
    assert_eq!(
        git(tree, &["rev-parse", "HEAD"]),
        first(tree),
        "{}",
        ran.text
    );
    let left = git(
        tree,
        &[
            "status",
            "--porcelain",
            "--ignored",
            "--",
            ".",
            ":!.engineering/cases",
            ":!target",
        ],
    );
    assert!(
        left.is_empty(),
        "the step's change is left: {left}\n{}",
        ran.text
    );
}

fn write(tree: &Path, path: &str, text: &str) {
    let at = tree.join(path);
    fs::create_dir_all(at.parent().expect("a parent")).expect("a directory");
    fs::write(at, text).expect("a file");
}

/// Expected before, 1: a step that sets a cargo runner of `true` makes `cargo test` exit 0 on a
/// crate whose only test fails.
#[test]
fn a_cargo_runner_written_by_a_step_earns_no_pass() {
    let tree = repository(
        "cargo-runner",
        &[
            (
                "Cargo.toml",
                "[package]\nname = \"forged\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[workspace]\n",
                false,
            ),
            (
                "src/lib.rs",
                "#[test]\nfn the_only_test() {\n    panic!(\"the only test fails\");\n}\n",
                false,
            ),
        ],
    );
    let locked = Command::new("cargo")
        .args(["generate-lockfile", "--offline", "--quiet"])
        .current_dir(&*tree)
        .status()
        .expect("cargo runs");
    assert!(locked.success(), "the lock file is generated");
    git(&tree, &["add", "Cargo.lock"]);
    git(&tree, &["commit", "-q", "--amend", "-m", "start"]);
    let ran = run(&tree, "cargo test --quiet", |tree| {
        write(
            tree,
            ".cargo/config.toml",
            "[target.'cfg(all())']\nrunner = \"true\"\n",
        );
    });
    assert_refused_and_put_back(&tree, &ran, ".cargo/config.toml");
    assert!(!tree.join(".cargo").exists(), "{}", ran.text);
}

/// Expected before, 2: a step that rewrites the test command's own script to `exit 0`.
#[test]
fn a_test_script_rewritten_to_exit_0_earns_no_pass() {
    let tree = repository("check-sh", &[("check.sh", FAILING_CHECK, true)]);
    let ran = run(&tree, "./check.sh", |tree| {
        fs::write(tree.join("check.sh"), "#!/bin/sh\nexit 0\n").expect("a file");
    });
    assert_refused_and_put_back(&tree, &ran, "check.sh");
}

/// The same with the script handed to an interpreter: `sh check.sh` names it too.
#[test]
fn a_script_an_interpreter_runs_is_guarded_too() {
    let tree = repository("sh-check-sh", &[("tests/check.sh", FAILING_CHECK, false)]);
    let ran = run(&tree, "sh -e tests/check.sh", |tree| {
        fs::write(tree.join("tests/check.sh"), "#!/bin/sh\nexit 0\n").expect("a file");
    });
    assert_refused_and_put_back(&tree, &ran, "tests/check.sh");
}

/// A step that writes a CI workflow is refused: it would decide what `ci.read` observes.
#[test]
fn a_workflow_written_by_a_step_is_refused() {
    let tree = repository("workflow", &[("check.sh", FAILING_CHECK, true)]);
    let ran = run(&tree, "./check.sh", |tree| {
        write(tree, ".github/workflows/ci.yml", "on: push\njobs: {}\n");
    });
    assert_refused_and_put_back(&tree, &ran, ".github/workflows/ci.yml");
    assert!(!tree.join(".github").exists(), "{}", ran.text);
}

/// A build script runs on every `cargo test`, at the root or in a member crate.
#[test]
fn a_build_script_written_by_a_step_is_refused() {
    for path in ["build.rs", "crates/member/build.rs"] {
        let tree = repository("build-rs", &[("check.sh", FAILING_CHECK, true)]);
        let ran = match path {
            "build.rs" => run(&tree, "./check.sh", |tree| {
                write(tree, "build.rs", "fn main() {}\n");
            }),
            _ => run(&tree, "./check.sh", |tree| {
                write(tree, "crates/member/build.rs", "fn main() {}\n");
            }),
        };
        assert_refused_and_put_back(&tree, &ran, path);
    }
}

/// The toolchain file chooses the compiler the tests are built with.
#[test]
fn a_toolchain_file_written_by_a_step_is_refused() {
    let tree = repository("toolchain", &[("check.sh", FAILING_CHECK, true)]);
    let ran = run(&tree, "./check.sh", |tree| {
        write(
            tree,
            "rust-toolchain.toml",
            "[toolchain]\nchannel = \"nightly\"\n",
        );
    });
    assert_refused_and_put_back(&tree, &ran, "rust-toolchain.toml");
}

/// The gates document and the assertions project file are the gate's inputs.
#[test]
fn the_gates_and_the_project_file_written_by_a_step_are_refused() {
    for path in [
        ".engineering/gates.yaml",
        ".engineering/assertions-project.yaml",
    ] {
        let tree = repository("gates", &[("check.sh", FAILING_CHECK, true)]);
        let ran = match path {
            ".engineering/gates.yaml" => run(&tree, "./check.sh", |tree| {
                write(
                    tree,
                    ".engineering/gates.yaml",
                    "format: engineering-gates/1\n",
                );
            }),
            _ => run(&tree, "./check.sh", |tree| {
                write(
                    tree,
                    ".engineering/assertions-project.yaml",
                    "format: engineering-assertions-project/1\n",
                );
            }),
        };
        assert_refused_and_put_back(&tree, &ran, path);
    }
}

/// The guarded prefixes match whatever the case of the path: on a file system that folds case,
/// `.Engineering/` is the store.
#[test]
fn a_judge_path_in_another_case_is_refused() {
    let tree = repository("case", &[("check.sh", FAILING_CHECK, true)]);
    let ran = run(&tree, "./check.sh", |tree| {
        write(
            tree,
            ".Engineering/planning/story/blocked-one.md",
            "forged\n",
        );
    });
    assert_refused_and_put_back(&tree, &ran, ".Engineering/planning/story/blocked-one.md");
}

/// A store directory the base holds as a link: what the step writes through it, at the link's
/// target, is the store's.
#[test]
fn a_judge_path_reached_through_a_link_at_the_base_is_refused() {
    let tree = repository(
        "link",
        &[
            ("check.sh", FAILING_CHECK, true),
            ("plan/story/ready-one.md", "ready\n", false),
        ],
    );
    fs::create_dir_all(tree.join(".engineering")).expect("a directory");
    std::os::unix::fs::symlink("../plan", tree.join(".engineering/planning")).expect("a link");
    git(&tree, &["add", "-A"]);
    git(&tree, &["commit", "-q", "--amend", "-m", "start"]);
    let ran = run(&tree, "./check.sh", |tree| {
        write(tree, "plan/story/blocked-one.md", "forged\n");
    });
    assert_refused_and_put_back(&tree, &ran, "plan/story/blocked-one.md");
}

/// Opens `fifo` for writing without waiting and closes it, so a reader blocked on it sees its
/// end.
fn release_reader(fifo: &Path) {
    use std::os::unix::fs::OpenOptionsExt as _;
    const O_NONBLOCK: i32 = 0o4000;
    let _ = fs::OpenOptions::new()
        .write(true)
        .custom_flags(O_NONBLOCK)
        .open(fifo);
}

/// A test command that plants a FIFO in the store and an ignored file under `.engineering/`:
/// the observation is refused, nothing recorded, and both are gone.
#[test]
fn a_validator_that_plants_in_the_store_is_refused_and_put_back() {
    let tree = repository(
        "plants",
        &[
            (
                "plant.sh",
                "#!/bin/sh\nmkdir -p .engineering/planning .engineering/local\nmkfifo .engineering/planning/pipe\necho x > .engineering/local/ignored\nexit 0\n",
                true,
            ),
            (".engineering/planning/README.md", "the store\n", false),
        ],
    );
    fs::write(tree.join(".gitignore"), "target/\n.engineering/local/\n").expect("a file");
    git(
        &tree,
        &["commit", "-q", "-am", "ignore the local store files"],
    );
    let ran = run(&tree, "./plant.sh", |_| {});
    release_reader(&tree.join(".engineering/planning/pipe"));
    let why = ran.refusal();
    for path in [".engineering/planning/pipe", ".engineering/local/ignored"] {
        assert!(
            why.contains(path),
            "the refusal names {path}: {why}\n{}",
            ran.text
        );
        assert!(
            fs::symlink_metadata(tree.join(path)).is_err(),
            "{path} is left\n{}",
            ran.text
        );
    }
    assert!(ran.records.is_empty(), "{:?}\n{}", ran.records, ran.text);
}

/// Opens a case on `tree` with no step taken, then commits `path` with `text` on the case's
/// branch by another route than a step: the implementation revision the validators observe.
fn case_with_commit(tree: &Scratch, path: &str, text: &str, executable: bool) -> (PathBuf, String) {
    let validators: Vec<Box<dyn Validator>> = Vec::new();
    let harness = Acts::new(|_| {});
    let run = Run::open(tree, options(tree.protocol(), 1), &harness, &validators)
        .expect("the case opens");
    let dir = run.case.dir.clone();
    drop(run);
    write(tree, path, text);
    if executable {
        fs::set_permissions(tree.join(path), fs::Permissions::from_mode(0o755)).expect("a mode");
    }
    git(tree, &["add", "--", path]);
    git(tree, &["commit", "-q", "-m", "by another route"]);
    (dir, git(tree, &["rev-parse", "HEAD"]))
}

/// `tests.run` at a revision whose branch changed the test command's script since the base:
/// nothing is run, nothing recorded, and the path is named.
#[test]
fn tests_run_records_nothing_while_the_branch_changed_its_script() {
    let tree = repository("tests-base", &[("check.sh", FAILING_CHECK, true)]);
    let (_, revision) = case_with_commit(&tree, "check.sh", "#!/bin/sh\nexit 0\n", true);
    let observation = TestsRun::new("./check.sh")
        .expect("a command")
        .observe(&tree, &revision, 1)
        .expect("observes");
    assert!(observation.records.is_empty(), "{:?}", observation.records);
    assert!(
        observation.summary.contains("check.sh"),
        "{}",
        observation.summary
    );
}

/// `tests.run` at a revision whose branch added a cargo configuration since the base.
#[test]
fn tests_run_records_nothing_while_the_branch_changed_the_cargo_configuration() {
    let tree = repository("tests-cargo", &[("check.sh", FAILING_CHECK, true)]);
    let (_, revision) = case_with_commit(
        &tree,
        ".cargo/config.toml",
        "[target.'cfg(all())']\nrunner = \"true\"\n",
        false,
    );
    let observation = TestsRun::new("./check.sh")
        .expect("a command")
        .observe(&tree, &revision, 1)
        .expect("observes");
    assert!(observation.records.is_empty(), "{:?}", observation.records);
    assert!(
        observation.summary.contains(".cargo/config.toml"),
        "{}",
        observation.summary
    );
}

/// `tests.run` at a revision whose branch changed what a link on the surface, held at the base,
/// leads to: `.cargo` is a link to `cfg`, and the branch writes `cfg/config.toml`.
#[test]
fn tests_run_follows_a_link_the_base_holds_on_the_surface() {
    let tree = repository(
        "tests-link",
        &[
            ("check.sh", FAILING_CHECK, true),
            ("cfg/README", "cargo\n", false),
        ],
    );
    std::os::unix::fs::symlink("cfg", tree.join(".cargo")).expect("a link");
    git(&tree, &["add", "-A"]);
    git(&tree, &["commit", "-q", "--amend", "-m", "start"]);
    let (_, revision) = case_with_commit(
        &tree,
        "cfg/config.toml",
        "[target.'cfg(all())']\nrunner = \"true\"\n",
        false,
    );
    let observation = TestsRun::new("./check.sh")
        .expect("a command")
        .observe(&tree, &revision, 1)
        .expect("observes");
    assert!(observation.records.is_empty(), "{:?}", observation.records);
    assert!(
        observation.summary.contains("cfg/config.toml"),
        "{}",
        observation.summary
    );
}

/// `ci.read` at a revision whose branch changed a workflow since the base: the forge's green
/// checks are not recorded, and the path is named.
#[test]
fn ci_read_records_nothing_while_the_branch_changed_a_workflow() {
    let tree = repository("ci-base", &[("check.sh", FAILING_CHECK, true)]);
    let (dir, revision) = case_with_commit(
        &tree,
        ".github/workflows/ci.yml",
        "on: push\njobs: {}\n",
        false,
    );
    assert_eq!(current(&tree).expect("the case"), dir);
    let mut branch = Branch::read(&dir).expect("the branch");
    branch.pull_request = Some(1);
    branch.pushed = Some(revision.clone());
    branch.write(&dir).expect("written");
    let memory = Rc::new(Memory {
        checks: Checks::Pass,
        head: Some(Head {
            branch: branch.name.clone(),
            sha: revision.clone(),
        }),
        ..Memory::default()
    });
    memory.opened.borrow_mut().push(PullRequest {
        branch: branch.name.clone(),
        base: "main".to_owned(),
        title: "t".to_owned(),
        body: "b".to_owned(),
    });
    let observation = CiRead::new(memory)
        .observe(&tree, &revision, 1)
        .expect("observes");
    assert!(observation.records.is_empty(), "{:?}", observation.records);
    assert!(
        observation.summary.contains(".github/workflows/ci.yml"),
        "{}",
        observation.summary
    );
}

/// What the run commits for an ordinary edit is unchanged: the step is accepted and the tests
/// pass on what it changed.
#[test]
fn an_edit_off_the_judge_surface_is_accepted() {
    let tree = repository(
        "ordinary",
        &[
            ("check.sh", "#!/bin/sh\ngrep -q fixed check.txt\n", true),
            ("check.txt", "broken\n", false),
        ],
    );
    let ran = run(&tree, "./check.sh", |tree| {
        fs::write(tree.join("check.txt"), "fixed\n").expect("a file");
    });
    assert_eq!(
        ran.stop.as_ref().ok(),
        Some(&Stop::Completed("accepted".to_owned())),
        "{}",
        ran.text
    );
    assert_eq!(ran.passes().len(), 1, "{:?}\n{}", ran.records, ran.text);
}
