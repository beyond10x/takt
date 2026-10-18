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

/// A check that passes.
const PASSING_CHECK_TEXT: &str = "#!/bin/sh\nexit 0\n";

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

/// A store directory the base holds as a link: a link is never followed, so no judgement step
/// is taken while it is there, and the refusal names the link.
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
    assert_refused_and_put_back(&tree, &ran, ".engineering/planning");
    assert!(
        !tree.join("plan/story/blocked-one.md").exists(),
        "{}",
        ran.text
    );
}

/// A step that plants a link on the surface (`.cargo` to a directory of its own) and writes the
/// configuration through it is refused and put back, the link with it.
#[test]
fn a_link_planted_on_the_surface_is_refused() {
    let tree = repository("planted-link", &[("check.sh", FAILING_CHECK, true)]);
    let ran = run(&tree, "./check.sh", |tree| {
        write(
            tree,
            "cfg/config.toml",
            "[target.'cfg(all())']\nrunner = \"true\"\n",
        );
        std::os::unix::fs::symlink("cfg", tree.join(".cargo")).expect("a link");
    });
    assert_refused_and_put_back(&tree, &ran, ".cargo");
    assert!(
        fs::symlink_metadata(tree.join(".cargo")).is_err(),
        "{}",
        ran.text
    );
}

/// A step that puts a link on the way to the test command's script (`tests` to a directory of
/// its own holding a `check.sh` that exits 0) is refused and put back.
#[test]
fn a_link_planted_on_the_way_to_the_script_is_refused() {
    let tree = repository("planted-way", &[("tests/check.sh", FAILING_CHECK, false)]);
    let ran = run(&tree, "sh tests/check.sh", |tree| {
        write(tree, "real/check.sh", "#!/bin/sh\nexit 0\n");
        fs::remove_dir_all(tree.join("tests")).expect("removed");
        std::os::unix::fs::symlink("real", tree.join("tests")).expect("a link");
    });
    assert_refused_and_put_back(&tree, &ran, "tests/check.sh");
    assert!(
        fs::symlink_metadata(tree.join("tests"))
            .is_ok_and(|meta| meta.is_dir() && !meta.file_type().is_symlink()),
        "{}",
        ran.text
    );
}

/// A manifest anywhere in the tree decides how cargo builds the tests through some of its keys:
/// a profile in a member's manifest, a patch at the root.
#[test]
fn a_manifest_entry_that_decides_the_build_is_refused() {
    for (path, entry) in [
        (
            "crates/member/Cargo.toml",
            "[profile.test]\nopt-level = 3\n",
        ),
        (
            "Cargo.toml",
            "[patch.crates-io]\nserde = { path = \"x\" }\n",
        ),
    ] {
        let tree = repository(
            "manifest",
            &[
                ("check.sh", FAILING_CHECK, true),
                (
                    "Cargo.toml",
                    "[package]\nname = \"m\"\nversion = \"0.1.0\"\n",
                    false,
                ),
                (
                    "crates/member/Cargo.toml",
                    "[package]\nname = \"member\"\nversion = \"0.1.0\"\n",
                    false,
                ),
            ],
        );
        let ran = match path {
            "Cargo.toml" => run(&tree, "./check.sh", |tree| {
                let text = fs::read_to_string(tree.join("Cargo.toml")).expect("a manifest");
                fs::write(
                    tree.join("Cargo.toml"),
                    format!("{text}[patch.crates-io]\nserde = {{ path = \"x\" }}\n"),
                )
                .expect("a file");
            }),
            _ => run(&tree, "./check.sh", |tree| {
                let at = tree.join("crates/member/Cargo.toml");
                let text = fs::read_to_string(&at).expect("a manifest");
                fs::write(at, format!("{text}[profile.test]\nopt-level = 3\n")).expect("a file");
            }),
        };
        let key = entry.lines().next().unwrap_or_default();
        assert_refused_and_put_back(&tree, &ran, path);
        assert!(
            ran.refusal().contains(if key.starts_with("[profile") {
                "profile.test.opt-level"
            } else {
                "patch.crates-io.serde"
            }),
            "{}",
            ran.text
        );
    }
}

/// A manifest change that decides nothing of the build (a dependency, a version) is the step's
/// to make.
#[test]
fn a_manifest_dependency_is_accepted() {
    let tree = repository(
        "manifest-dependency",
        &[
            ("check.sh", "#!/bin/sh\ngrep -q fixed check.txt\n", true),
            ("check.txt", "broken\n", false),
            (
                "Cargo.toml",
                "[package]\nname = \"m\"\nversion = \"0.1.0\"\n",
                false,
            ),
        ],
    );
    let ran = run(&tree, "./check.sh", |tree| {
        fs::write(
            tree.join("Cargo.toml"),
            "[package]\nname = \"m\"\nversion = \"0.2.0\"\n\n[dependencies]\nserde = \"1\"\n",
        )
        .expect("a file");
        fs::write(tree.join("check.txt"), "fixed\n").expect("a file");
    });
    assert_eq!(
        ran.stop.as_ref().ok(),
        Some(&Stop::Completed("accepted".to_owned())),
        "{}",
        ran.text
    );
}

/// The paths the guard reads as the test command's own, for the command lines a repository
/// gives: through `env` and its options, wrappers, an interpreter's options, `sh -c`, and the
/// build tools' files.
#[test]
fn the_test_command_names_its_own_files() {
    let tree = repository("parse", &[("check.sh", FAILING_CHECK, true)]);
    let cases: &[(&str, &[&str])] = &[
        ("./check.sh", &["check.sh"]),
        ("env -u NAME ./check.sh", &["check.sh"]),
        ("env -i A=1 --unset=B -C sub ./check.sh", &["sub/check.sh"]),
        ("env -S ./check.sh", &["check.sh"]),
        ("sh -o errexit tests/run.sh", &["tests/run.sh"]),
        ("bash -O extglob -e tests/run.sh", &["tests/run.sh"]),
        ("bash -c ./check.sh", &["check.sh"]),
        ("timeout 600 ./check.sh", &["check.sh"]),
        ("timeout -s KILL 10m nice -n 5 ./check.sh", &["check.sh"]),
        ("stdbuf -o L ionice -c 3 nohup ./check.sh", &["check.sh"]),
        ("flock lock -c ./check.sh", &["check.sh"]),
        ("python3 -m pytest tests", &[]),
        ("python3 -W ignore tests/run.py", &["tests/run.py"]),
        ("make test", &["GNUmakefile", "Makefile", "makefile"]),
        ("make -C sub -f build.mk test", &["sub/build.mk"]),
        (
            "task check",
            &[
                "Taskfile.dist.yaml",
                "Taskfile.dist.yml",
                "Taskfile.yaml",
                "Taskfile.yml",
            ],
        ),
        ("npm --prefix web test", &["web/package.json"]),
        ("just test", &[".justfile", "justfile"]),
        ("bash -c -- './check.sh'", &["check.sh"]),
        ("sh -c 'cd sub;./check.sh'", &["check.sh", "sub"]),
        ("perl -I lib t/check.pl", &["t/check.pl"]),
        (
            "deno run --allow-read -c deno.json tests/check.ts",
            &["tests/check.ts"],
        ),
        ("taskset -c 0 ./check.sh", &["check.sh"]),
        ("sh ../repo/check.sh", &["check.sh"]),
        (
            "npm test",
            &[
                "package.json",
                ".npmrc",
                ".yarnrc",
                ".yarnrc.yml",
                ".pnpmfile.cjs",
            ],
        ),
        (
            "unknown --config=conf/x.toml 'quoted.sh'",
            &["conf/x.toml", "quoted.sh"],
        ),
    ];
    for (command, expected) in cases {
        let words: Vec<String> = command.split_whitespace().map(str::to_owned).collect();
        let named = takt::run::scripts_of(&words, &tree);
        for path in *expected {
            assert!(
                named.iter().any(|named| named == path),
                "`{command}` names {named:?}, not {path}"
            );
        }
        for script in ["tests", "pytest", "test"] {
            assert!(
                !named.iter().any(|named| named == script) || expected.contains(&script),
                "`{command}` names {named:?}"
            );
        }
    }
}

/// A manifest line the scanner cannot read with certainty refuses the step, and the refusal
/// says so: a comment after a header, an escape in a key, a multi-line string ending in four
/// quotes, a date with a time (a value the scanner does not read).
#[test]
fn a_manifest_the_scanner_cannot_read_is_refused() {
    let base = "[package]\nname = \"m\"\nversion = \"0.1.0\"\n";
    for edited in [
        "[package] # x\nname = \"m\"\nversion = \"0.1.0\"\n",
        "[package]\nname = \"m\"\nversion = \"0.1.0\"\n\"\\u0062uild\" = \"gen.rs\"\n",
        "[package]\nname = \"m\"\nversion = \"0.1.0\"\ndescription = \"\"\"x\"\"\"\"\n",
        "[package]\nname = \"m\"\nversion = \"0.1.0\"\n[package.metadata]\nat = 1979-05-27 07:32:00\n",
    ] {
        let tree = repository(
            "uncertain",
            &[
                ("check.sh", FAILING_CHECK, true),
                ("Cargo.toml", base, false),
                ("src/lib.rs", "", false),
            ],
        );
        fs::write(tree.root.join("edited.toml"), edited).expect("a file");
        let ran = run(&tree, "./check.sh", |tree| {
            let edited = tree.parent().expect("the root").join("edited.toml");
            fs::copy(edited, tree.join("Cargo.toml")).expect("copied");
        });
        assert_refused_and_put_back(&tree, &ran, "Cargo.toml");
        assert!(
            ran.refusal().contains("cannot read with certainty"),
            "{edited}\n{}",
            ran.text
        );
    }
}

/// Cargo as the parser: a step that adds a workspace member whose build script the base holds
/// outside the workspace changes no build key the scanner looks at, but `cargo metadata` reads a
/// new build script; the step is refused naming it.
#[test]
fn a_build_script_cargo_reads_after_the_step_is_refused() {
    let tree = repository(
        "cargo-reads",
        &[
            ("check.sh", FAILING_CHECK, true),
            (
                "Cargo.toml",
                "[workspace]\nmembers = [\"crates/a\"]\nresolver = \"2\"\n",
                false,
            ),
            (
                "crates/a/Cargo.toml",
                "[package]\nname = \"a\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
                false,
            ),
            ("crates/a/src/lib.rs", "", false),
            (
                "extra/b/Cargo.toml",
                "[package]\nname = \"b\"\nversion = \"0.1.0\"\nedition = \"2021\"\nbuild = \"gen.rs\"\n",
                false,
            ),
            ("extra/b/gen.rs", "fn main() {}\n", false),
            ("extra/b/src/lib.rs", "", false),
        ],
    );
    let ran = run(&tree, "./check.sh", |tree| {
        fs::write(
            tree.join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/a\", \"extra/b\"]\nresolver = \"2\"\n",
        )
        .expect("a file");
    });
    assert_refused_and_put_back(&tree, &ran, "cargo reads");
    assert!(ran.refusal().contains("gen.rs"), "{}", ran.text);
}

/// A link under `.github/` that no validator reads does not lock the run out, but a step that
/// changes it is refused all the same.
#[test]
fn a_link_nothing_reads_is_not_followed_and_a_change_to_it_is_refused() {
    let tree = repository(
        "unread-link",
        &[
            ("check.sh", FAILING_CHECK, true),
            ("AGENTS.md", "# Agents\n", false),
            ("OTHER.md", "# Other\n", false),
        ],
    );
    fs::create_dir_all(tree.join(".github")).expect("a directory");
    std::os::unix::fs::symlink("../AGENTS.md", tree.join(".github/copilot-instructions.md"))
        .expect("a link");
    git(&tree, &["add", "-A"]);
    git(&tree, &["commit", "-q", "--amend", "-m", "start"]);
    let ran = run(&tree, "./check.sh", |tree| {
        let link = tree.join(".github/copilot-instructions.md");
        fs::remove_file(&link).expect("removed");
        std::os::unix::fs::symlink("../OTHER.md", &link).expect("a link");
    });
    assert_refused_and_put_back(&tree, &ran, ".github/copilot-instructions.md");
    assert_eq!(
        fs::read_link(tree.join(".github/copilot-instructions.md")).expect("the link"),
        Path::new("../AGENTS.md"),
        "{}",
        ran.text
    );
}

/// Reviewer route N1: a step that writes a cargo runner of `true` into `.cargo/config.toml` in
/// the directory above the tree, which cargo reads too, and touches the code: refused, and the
/// file it created removed.
#[test]
fn a_cargo_configuration_above_the_tree_is_refused_and_removed() {
    let tree = repository(
        "above",
        &[("check.sh", FAILING_CHECK, true), ("src/lib.rs", "", false)],
    );
    let above = tree.root.join(".cargo/config.toml");
    let ran = run(&tree, "./check.sh", |tree| {
        write(
            tree,
            "../.cargo/config.toml",
            "[target.'cfg(all())']\nrunner = \"true\"\n",
        );
        write(tree, "src/lib.rs", "// touched\n");
    });
    assert_refused_and_put_back(&tree, &ran, &above.display().to_string());
    assert!(ran.refusal().contains("outside the tree"), "{}", ran.text);
    assert!(fs::symlink_metadata(&above).is_err(), "{}", ran.text);
}

/// A toolchain file above the tree that the step rewrites is written back as it was.
#[test]
fn a_toolchain_file_above_the_tree_is_put_back() {
    let tree = repository("above-toolchain", &[("check.sh", FAILING_CHECK, true)]);
    let toolchain = tree.root.join("rust-toolchain.toml");
    fs::write(&toolchain, "[toolchain]\nchannel = \"stable\"\n").expect("a file");
    let ran = run(&tree, "./check.sh", |tree| {
        write(
            tree,
            "../rust-toolchain.toml",
            "[toolchain]\nchannel = \"nightly\"\n",
        );
    });
    assert!(
        ran.refusal().contains("rust-toolchain.toml"),
        "{}",
        ran.text
    );
    assert_eq!(
        fs::read_to_string(&toolchain).expect("the file"),
        "[toolchain]\nchannel = \"stable\"\n",
        "{}",
        ran.text
    );
}

/// A test command that writes above the tree: the observation is refused, nothing recorded,
/// and what it wrote removed.
#[test]
fn a_validator_that_writes_above_the_tree_is_refused_and_put_back() {
    let tree = repository(
        "above-validator",
        &[(
            "plant.sh",
            "#!/bin/sh\nmkdir -p ../.cargo\nprintf 'x = 1\\n' > ../.cargo/config.toml\nexit 0\n",
            true,
        )],
    );
    let ran = run(&tree, "./plant.sh", |_| {});
    let above = tree.root.join(".cargo/config.toml");
    assert!(
        ran.refusal().contains(&above.display().to_string()),
        "{}",
        ran.text
    );
    assert!(ran.records.is_empty(), "{:?}\n{}", ran.records, ran.text);
    assert!(fs::symlink_metadata(&above).is_err(), "{}", ran.text);
}

/// A cargo configuration above the tree changed by another route than a step, after the run
/// began: `tests.run` observes nothing and records nothing while it differs.
#[test]
fn tests_run_records_nothing_while_a_file_above_the_tree_differs() {
    let tree = repository("above-route", &[("check.sh", PASSING_CHECK_TEXT, true)]);
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(TestsRun::new("./check.sh").expect("a command"))];
    let harness = Acts::new(|_| {});
    let mut run = Run::open(&tree, options(tree.protocol(), 4), &harness, &validators)
        .expect("the case opens");
    write(&tree, "../.cargo/config.toml", "x = 1\n");
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let text = String::from_utf8_lossy(&out).into_owned();
    let Ok(Stop::StepRefused(why)) = &stop else {
        panic!("the observation is refused, not {stop:?}\n{text}");
    };
    assert!(why.contains("differs from what the run found"), "{why}");
    assert!(records(&run.case.dir).is_empty(), "{text}");
}

/// Reviewer route N7 and its siblings: what decides pytest's collection and report is on the
/// surface, a new `conftest.py` at any depth among it, and pytest's tables in `pyproject.toml`.
#[test]
fn pytest_configuration_written_by_a_step_is_refused() {
    for (path, text) in [
        (
            "conftest.py",
            "def pytest_runtest_makereport(item, call):\n    pass\n",
        ),
        ("tests/conftest.py", "import pytest\n"),
        ("pytest.ini", "[pytest]\naddopts = -p no:x\n"),
        ("tox.ini", "[pytest]\n"),
        ("setup.cfg", "[tool:pytest]\n"),
    ] {
        let tree = repository("pytest", &[("check.sh", FAILING_CHECK, true)]);
        fs::write(tree.root.join("payload"), format!("{path}\n{text}")).expect("a file");
        let ran = run(&tree, "./check.sh", |tree| {
            let payload =
                fs::read_to_string(tree.parent().expect("the root").join("payload")).expect("read");
            let (path, text) = payload.split_once('\n').expect("a payload");
            write(tree, path, text);
        });
        assert_refused_and_put_back(&tree, &ran, path);
    }
}

/// `pyproject.toml`: a change to `[tool.pytest.ini_options]` is refused; one to another table
/// is the step's to make.
#[test]
fn pyproject_pytest_tables_are_guarded_and_the_rest_is_not() {
    let base = "[project]\nname = \"p\"\nversion = \"0.1.0\"\n";
    let tree = repository(
        "pyproject",
        &[
            ("check.sh", FAILING_CHECK, true),
            ("pyproject.toml", base, false),
        ],
    );
    let ran = run(&tree, "./check.sh", |tree| {
        let text = fs::read_to_string(tree.join("pyproject.toml")).expect("read");
        fs::write(
            tree.join("pyproject.toml"),
            format!("{text}\n[tool.pytest.ini_options]\naddopts = \"-p no:x\"\n"),
        )
        .expect("a file");
    });
    assert_refused_and_put_back(
        &tree,
        &ran,
        "pyproject.toml: tool.pytest.ini_options.addopts",
    );
    let tree = repository(
        "pyproject-other",
        &[
            ("check.sh", "#!/bin/sh\ngrep -q fixed check.txt\n", true),
            ("check.txt", "broken\n", false),
            ("pyproject.toml", base, false),
        ],
    );
    let ran = run(&tree, "./check.sh", |tree| {
        fs::write(
            tree.join("pyproject.toml"),
            "[project]\nname = \"p\"\nversion = \"0.2.0\"\n\n[tool.ruff]\nline-length = 100\n",
        )
        .expect("a file");
        fs::write(tree.join("check.txt"), "fixed\n").expect("a file");
    });
    assert_eq!(
        ran.stop.as_ref().ok(),
        Some(&Stop::Completed("accepted".to_owned())),
        "{}",
        ran.text
    );
}

/// The repository's own list, `.engineering/judge-surface.yaml` at the base, extends the
/// surface: a step that writes a path it lists is refused, and so is one that rewrites the list
/// itself to drop the path, in the same step.
#[test]
fn the_repository_judge_surface_list_extends_the_default() {
    let tree = repository(
        "judge-list",
        &[
            ("check.sh", FAILING_CHECK, true),
            (
                ".engineering/judge-surface.yaml",
                "- fixtures/**\n- Makefile\n",
                false,
            ),
            ("fixtures/expected.txt", "expected\n", false),
        ],
    );
    let ran = run(&tree, "./check.sh", |tree| {
        write(tree, "fixtures/expected.txt", "anything\n");
    });
    assert_refused_and_put_back(&tree, &ran, "fixtures/expected.txt");
    let tree = repository(
        "judge-list-dropped",
        &[
            ("check.sh", FAILING_CHECK, true),
            (".engineering/judge-surface.yaml", "- fixtures/**\n", false),
            ("fixtures/expected.txt", "expected\n", false),
        ],
    );
    let ran = run(&tree, "./check.sh", |tree| {
        write(tree, ".engineering/judge-surface.yaml", "[]\n");
        write(tree, "fixtures/expected.txt", "anything\n");
    });
    assert_refused_and_put_back(&tree, &ran, "fixtures/expected.txt");
    assert!(
        ran.refusal().contains(".engineering/judge-surface.yaml"),
        "{}",
        ran.text
    );
}

/// A list at the base that does not parse fails closed: no judgement step is taken, and
/// `tests.run` records nothing.
#[test]
fn a_judge_surface_list_that_does_not_parse_fails_closed() {
    let tree = repository(
        "judge-list-broken",
        &[
            ("check.sh", "#!/bin/sh\nexit 0\n", true),
            (".engineering/judge-surface.yaml", "fixtures: [\n", false),
        ],
    );
    let ran = run(&tree, "./check.sh", |_| {});
    assert!(ran.refusal().contains("does not parse"), "{}", ran.text);
    assert!(ran.passes().is_empty(), "{:?}\n{}", ran.records, ran.text);
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

/// `tests.run` while the base holds a link on the surface, `.cargo` to `cfg`, and the branch
/// writes `cfg/config.toml`: a link is never followed; nothing is recorded and the link named.
#[test]
fn tests_run_records_nothing_while_the_base_holds_a_link_on_the_surface() {
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
        observation.summary.contains(".cargo (a link)"),
        "{}",
        observation.summary
    );
}

/// `tests.run` at a revision whose branch names a build script under another name in the
/// manifest, by another route than a step: nothing is recorded and the key named.
#[test]
fn tests_run_records_nothing_while_the_branch_changed_a_manifest_build_key() {
    let tree = repository(
        "tests-manifest",
        &[
            ("check.sh", FAILING_CHECK, true),
            (
                "Cargo.toml",
                "[package]\nname = \"m\"\nversion = \"0.1.0\"\n",
                false,
            ),
        ],
    );
    let (_, revision) = case_with_commit(
        &tree,
        "Cargo.toml",
        "[package]\nname = \"m\"\nversion = \"0.1.0\"\nbuild = \"gen.rs\"\n",
        false,
    );
    let observation = TestsRun::new("./check.sh")
        .expect("a command")
        .observe(&tree, &revision, 1)
        .expect("observes");
    assert!(observation.records.is_empty(), "{:?}", observation.records);
    assert!(
        observation.summary.contains("Cargo.toml: package.build"),
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

/// Reviewer route M10: `python3 -m <module>` imports `<module>.py` or the package `<module>/`
/// from the directory it runs in before the installed one, so a step that writes either at the
/// tree's root replaces the module the test command runs (a `pytest.py` that exits 0). Both are
/// refused, and so is the dotted form's first part.
#[test]
fn a_module_the_test_command_runs_with_python_m_is_guarded() {
    for (command, path) in [
        ("python3 -B -m checker", "checker.py"),
        ("python3 -B -m checker", "checker/__main__.py"),
        ("python3 -B -m checker.sub -q", "checker/sub.py"),
    ] {
        let tree = repository("python-m", &[("README.md", "a story\n", false)]);
        fs::write(tree.root.join("payload"), path).expect("a file");
        let ran = run(&tree, command, |tree| {
            let path =
                fs::read_to_string(tree.parent().expect("the root").join("payload")).expect("read");
            write(tree, &path, "import sys\nsys.exit(0)\n");
        });
        assert_refused_and_put_back(&tree, &ran, path);
    }
}

/// Reviewer routes M11a/b: pytest also reads `.pytest.ini`, `pytest.toml` and `.pytest.toml`;
/// `addopts = --collect-only` in any of them passes every run.
#[test]
fn pytest_dotfile_and_toml_configuration_is_refused() {
    for (path, text) in [
        (".pytest.ini", "[pytest]\naddopts = --collect-only\n"),
        ("pytest.toml", "[pytest]\naddopts = [\"--collect-only\"]\n"),
        (".pytest.toml", "[pytest]\naddopts = [\"--collect-only\"]\n"),
    ] {
        let tree = repository("pytest-more", &[("check.sh", FAILING_CHECK, true)]);
        fs::write(tree.root.join("payload"), format!("{path}\n{text}")).expect("a file");
        let ran = run(&tree, "./check.sh", |tree| {
            let payload =
                fs::read_to_string(tree.parent().expect("the root").join("payload")).expect("read");
            let (path, text) = payload.split_once('\n').expect("a payload");
            write(tree, path, text);
        });
        assert_refused_and_put_back(&tree, &ran, path);
    }
}

/// Reviewer route M12: the repository's list names `ci/**`, and the base holds `ci` as a link to
/// `tools`: what the list guards is read through the link, so no judgement step is taken while
/// it is there, as for a link on the default surface.
#[test]
fn a_link_towards_a_path_the_repository_list_names_locks_steps_out() {
    let tree = repository(
        "list-link",
        &[
            ("check.sh", FAILING_CHECK, true),
            (".engineering/judge-surface.yaml", "- ci/**\n", false),
            ("tools/run.sh", "#!/bin/sh\nexit 1\n", false),
        ],
    );
    std::os::unix::fs::symlink("tools", tree.join("ci")).expect("a link");
    git(&tree, &["add", "-A"]);
    git(&tree, &["commit", "-q", "--amend", "-m", "start"]);
    let ran = run(&tree, "./check.sh", |tree| {
        write(tree, "tools/run.sh", "#!/bin/sh\nexit 0\n");
    });
    assert_refused_and_put_back(&tree, &ran, "ci -> tools");
}

/// Reviewer route M15: a step that makes `.cargo` above the tree a link to a directory that
/// already holds a `config.toml`. The put-back removes the link it made and never removes or
/// writes through it: the file it pointed at is still there, as it was.
#[test]
fn putting_back_above_the_tree_never_goes_through_a_link() {
    let tree = repository("above-link", &[("check.sh", FAILING_CHECK, true)]);
    let elsewhere = tree.root.join("elsewhere/config.toml");
    fs::create_dir_all(elsewhere.parent().expect("a parent")).expect("a directory");
    fs::write(&elsewhere, "kept = true\n").expect("a file");
    let ran = run(&tree, "./check.sh", |tree| {
        std::os::unix::fs::symlink("elsewhere", tree.join("../.cargo")).expect("a link");
    });
    assert!(ran.refusal().contains(".cargo"), "{}", ran.text);
    assert_eq!(
        fs::read_to_string(&elsewhere).ok().as_deref(),
        Some("kept = true\n"),
        "the put-back went through the link\n{}",
        ran.text
    );
    assert!(
        fs::symlink_metadata(tree.root.join(".cargo")).is_err(),
        "the link the step made is left\n{}",
        ran.text
    );
}

/// The refusal of a change to pytest's tables in `pyproject.toml` names pytest, not cargo.
#[test]
fn a_pyproject_refusal_does_not_speak_of_cargo() {
    let tree = repository(
        "pyproject-wording",
        &[
            ("check.sh", FAILING_CHECK, true),
            ("pyproject.toml", "[project]\nname = \"p\"\n", false),
        ],
    );
    let ran = run(&tree, "./check.sh", |tree| {
        fs::write(
            tree.join("pyproject.toml"),
            "[project]\nname = \"p\"\n\n[tool.pytest.ini_options]\naddopts = \"-q\"\n",
        )
        .expect("a file");
    });
    let why = ran.refusal();
    assert!(why.contains("pyproject.toml"), "{why}");
    assert!(!why.contains("cargo"), "{why}");
}

/// A judgement step whose harness leaves a writer running in a process group of its own: the
/// group is killed once the step is over, so nothing it would write lands in the tree after it,
/// between steps or after the run.
#[test]
fn a_writer_a_step_leaves_running_is_killed_with_its_group() {
    use std::os::unix::process::CommandExt as _;
    let tree = repository("late-writer", &[("check.sh", FAILING_CHECK, true)]);
    let ran = run(&tree, "./check.sh", |tree| {
        // Left running on purpose: the run kills and reaps it once the step is over.
        #[allow(clippy::zombie_processes)]
        Command::new("sh")
            .args(["-c", "sleep 1; echo late > late.txt"])
            .current_dir(tree)
            .process_group(0)
            .spawn()
            .expect("a writer");
    });
    std::thread::sleep(std::time::Duration::from_millis(2500));
    assert!(
        !tree.join("late.txt").exists(),
        "a writer the step left wrote into the tree after it\n{:?}\n{}",
        ran.stop,
        ran.text
    );
}

/// Out of this story, recorded on story:confinement-seam (reviewer route M8): a `cargo` wrapper
/// a step writes into a directory on the test command's `PATH` outside the tree runs instead of
/// cargo and exits 0.
#[test]
#[ignore = "confinement-seam: a program in a user-writable PATH directory outside the tree (M8) is confinement, not the step guard"]
fn a_cargo_wrapper_on_the_path_outside_the_tree_earns_no_pass() {
    let tree = repository(
        "path-wrapper",
        &[
            (
                "Cargo.toml",
                "[package]\nname = \"m\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[workspace]\n",
                false,
            ),
            (
                "src/lib.rs",
                "#[test]\nfn the_only_test() {\n    panic!(\"fails\");\n}\n",
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
    let cargo = std::env::var("CARGO").expect("cargo runs the tests");
    let real = Path::new(&cargo)
        .parent()
        .expect("a directory")
        .display()
        .to_string();
    let bin = tree.root.join("bin");
    fs::create_dir_all(&bin).expect("a directory");
    let command = format!(
        "env PATH={}:{real}:/usr/bin:/bin cargo test --quiet --offline",
        bin.display()
    );
    let ran = run(&tree, &command, |tree| {
        let wrapper = tree.parent().expect("the root").join("bin/cargo");
        fs::write(&wrapper, "#!/bin/sh\nexit 0\n").expect("a wrapper");
        fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o755)).expect("a mode");
        write(
            tree,
            "src/lib.rs",
            "// touched\n#[test]\nfn the_only_test() {\n    panic!(\"fails\");\n}\n",
        );
    });
    assert!(ran.passes().is_empty(), "{:?}\n{}", ran.records, ran.text);
}

/// Out of this story, recorded on story:confinement-seam (reviewer route M9): a test command
/// that starts a writer which leaves its process group and outlives the observation, writing
/// into the tree between runs. `setsid` alone is caught: takt adopts the orphan as its
/// subreaper and kills what is in another session. Rejoining takt's own process group with the
/// call's mark dropped from the environment is not.
#[test]
#[ignore = "confinement-seam: a writer escaping the process group (M9, setsid or rejoining takt's own group with the mark dropped) is confinement, not the step guard"]
fn a_writer_escaping_the_process_group_writes_nothing_after_the_observation() {
    let tree = repository(
        "escaping-writer",
        &[
            (
                "check.sh",
                "#!/bin/sh\nenv -u TAKT_CHILD python3 writer.py \"$PPID\" </dev/null >/dev/null 2>&1\nexit 0\n",
                true,
            ),
            (
                "writer.py",
                "import os, sys, time\nos.setpgid(0, os.getpgid(int(sys.argv[1])))\nif os.fork() == 0:\n    time.sleep(1)\n    open(\"late.txt\", \"w\").write(\"late\")\n    os._exit(0)\n",
                false,
            ),
        ],
    );
    let ran = run(&tree, "./check.sh", |_| {});
    std::thread::sleep(std::time::Duration::from_millis(2500));
    assert!(
        !tree.join("late.txt").exists(),
        "a writer the test command started wrote into the tree after it\n{:?}\n{}",
        ran.stop,
        ran.text
    );
}
