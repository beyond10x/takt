//! Adversary pass 1 on story:step-cannot-forge-its-validators. Each case is a judgement step
//! that changes what `tests.run` runs, outside the paths the judge surface names, and is
//! expected to be refused (or to earn no `pass`). Every repository is a scratch one this file
//! builds under the crate's build directory; nothing is written outside it.

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
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

const FAILING_CHECK: &str = "#!/bin/sh\nexit 1\n";
const PASSING_CHECK: &str = "#!/bin/sh\nexit 0\n";

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

fn repository(name: &str, files: &[(&str, &str, bool)]) -> Scratch {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("step-forge-adv1-{}-{name}", std::process::id()));
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

/// A cargo repository from `files`, its lock file committed with it, so `cargo test --offline`
/// builds.
fn cargo_repository(name: &str, files: &[(&str, &str, bool)]) -> Scratch {
    let tree = repository(name, files);
    let locked = Command::new("cargo")
        .args(["generate-lockfile", "--offline", "--quiet"])
        .current_dir(&*tree)
        .status()
        .expect("cargo runs");
    assert!(locked.success(), "the lock file is generated");
    git(&tree, &["add", "Cargo.lock"]);
    git(&tree, &["commit", "-q", "--amend", "-m", "start"]);
    tree
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

struct Acts {
    act: fn(&Path),
    done: std::cell::Cell<bool>,
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

struct Ran {
    stop: anyhow::Result<Stop>,
    text: String,
    records: Vec<serde_json::Value>,
}

impl Ran {
    fn passes(&self) -> usize {
        self.records
            .iter()
            .filter(|r| r["kind"] == "test_result" && r["result"] == "pass")
            .count()
    }

    fn refused(&self) -> bool {
        matches!(self.stop, Ok(Stop::StepRefused(_)))
    }

    /// The case: the step earned no `pass`, and was refused.
    fn assert_no_forged_pass(&self) {
        assert!(
            self.passes() == 0 && self.refused(),
            "a judgement step that changed what tests.run runs earned {} pass record(s) and the run stopped {:?}\n{}",
            self.passes(),
            self.stop,
            self.text
        );
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

fn run(tree: &Scratch, command: &str, act: fn(&Path)) -> Ran {
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(TestsRun::new(command).expect("a command"))];
    let harness = Acts {
        act,
        done: std::cell::Cell::new(false),
    };
    let protocol = tree.root.join("protocol.yaml").display().to_string();
    let mut run = Run::open(tree, options(protocol), &harness, &validators).expect("opens");
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    Ran {
        stop,
        text: String::from_utf8_lossy(&out).into_owned(),
        records: records(&run.case.dir),
    }
}

fn write(tree: &Path, path: &str, text: &str) {
    let at = tree.join(path);
    fs::create_dir_all(at.parent().expect("a parent")).expect("a directory");
    fs::write(at, text).expect("a file");
}

fn write_executable(tree: &Path, path: &str, text: &str) {
    write(tree, path, text);
    fs::set_permissions(tree.join(path), fs::Permissions::from_mode(0o755)).expect("a mode");
}

/// The wrapper `name` as the test command names it: the word itself where the `PATH` takt
/// inherits has it; else a stand-in outside the tree, named by its absolute path, that drops the
/// `before` words ahead of the command and runs the command (macOS has no `timeout`). The guard
/// reads a wrapper by its name, whatever directory names it.
fn wrapper(tree: &Scratch, name: &str, before: usize) -> String {
    let on_path = std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
        .filter(|dir| dir.is_absolute())
        .any(|dir| {
            fs::metadata(dir.join(name))
                .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
        });
    if on_path {
        return name.to_owned();
    }
    let program = tree.root.join("bin").join(name);
    write_executable(
        &tree.root,
        &format!("bin/{name}"),
        &format!("#!/bin/sh\nshift {before}\nexec \"$@\"\n"),
    );
    let program = program.to_str().expect("a UTF-8 path").to_owned();
    assert!(
        !program.contains(char::is_whitespace),
        "the test command is split at white space: {program}"
    );
    program
}

const MANIFEST: &str =
    "[package]\nname = \"forged\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[workspace]\n";
const FAILING_LIB: &str =
    "#[test]\nfn the_only_test() {\n    panic!(\"the only test fails\");\n}\n";

// ---- the default test command, `cargo test --quiet` ----

/// The manifest decides which tests `cargo test` builds: `[lib] test = false` drops the only
/// test, which still fails, and `cargo test` exits 0.
#[test]
#[ignore = "test selection is code under change, judged by the held-out acceptance, not the step guard; see review-result:adversary-step-forge-pass-1; the same class as N5 (a Makefile include) and N6 (a Taskfile dotenv .env)"]
fn adv1_default_command_manifest_that_drops_the_failing_test_earns_no_pass() {
    let tree = cargo_repository(
        "manifest-test-false",
        &[
            ("Cargo.toml", MANIFEST, false),
            ("src/lib.rs", FAILING_LIB, false),
        ],
    );
    let ran = run(&tree, "cargo test --quiet --offline", |tree| {
        let manifest = fs::read_to_string(tree.join("Cargo.toml")).expect("the manifest");
        fs::write(
            tree.join("Cargo.toml"),
            format!("{manifest}\n[lib]\ntest = false\n"),
        )
        .expect("a file");
    });
    ran.assert_no_forged_pass();
}

/// A build script under another name than `build.rs`, named by the manifest's `build` key: it
/// runs on every `cargo test` as `build.rs` does, and is not on the surface.
#[test]
fn adv1_default_command_build_script_named_by_the_manifest_is_refused() {
    let tree = cargo_repository(
        "manifest-build-key",
        &[
            ("Cargo.toml", MANIFEST, false),
            ("src/lib.rs", FAILING_LIB, false),
        ],
    );
    let ran = run(&tree, "cargo test --quiet --offline", |tree| {
        fs::write(
            tree.join("Cargo.toml"),
            MANIFEST.replace(
                "edition = \"2021\"\n",
                "edition = \"2021\"\nbuild = \"gen.rs\"\n",
            ),
        )
        .expect("a file");
        write(tree, "gen.rs", "fn main() {}\n");
    });
    assert!(
        ran.refused(),
        "a step that added a build script under another name was not refused: {:?}\n{}",
        ran.stop,
        ran.text
    );
}

/// A `build.rs` in a path dependency, which the step hides from git by ignoring it: the root
/// snapshot does not see it and git does not list it, but `cargo test` runs it.
#[test]
fn adv1_default_command_ignored_member_build_script_is_refused() {
    let tree = cargo_repository(
        "ignored-build-rs",
        &[
            (
                "Cargo.toml",
                "[package]\nname = \"forged\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\nhelper = { path = \"crates/helper\" }\n\n[workspace]\nmembers = [\"crates/helper\"]\n",
                false,
            ),
            (
                "src/lib.rs",
                "pub fn f() { helper::g() }\n#[test]\nfn the_only_test() {\n    panic!(\"the only test fails\");\n}\n",
                false,
            ),
            (
                "crates/helper/Cargo.toml",
                "[package]\nname = \"helper\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
                false,
            ),
            ("crates/helper/src/lib.rs", "pub fn g() {}\n", false),
        ],
    );
    let ran = run(&tree, "cargo test --quiet --offline", |tree| {
        let ignore = fs::read_to_string(tree.join(".gitignore")).expect("the ignore file");
        fs::write(
            tree.join(".gitignore"),
            format!("{ignore}crates/helper/build.rs\n"),
        )
        .expect("a file");
        write(
            tree,
            "crates/helper/build.rs",
            "fn main() { panic!(\"the build script decides the verdict\") }\n",
        );
    });
    let decided = ran.text.contains("the build script decides the verdict")
        || ran.records.iter().any(|r| {
            r.to_string()
                .contains("the build script decides the verdict")
        });
    assert!(
        ran.refused(),
        "a step that planted an ignored build script was not refused (the build script ran under tests.run: {decided}): {:?}\n{}",
        ran.stop,
        ran.text
    );
}

// ---- a script test command ----

/// `./check.sh` sources `lib.sh`; the step rewrites `lib.sh`, not `check.sh`.
#[test]
#[ignore = "files the guarded script calls are code under change, judged by the held-out acceptance, not the step guard; the same class as N5 (a Makefile include) and N6 (a Taskfile dotenv .env)"]
fn adv1_script_a_sourced_file_rewritten_earns_no_pass() {
    let tree = repository(
        "sourced",
        &[
            ("check.sh", "#!/bin/sh\n. ./lib.sh\n", true),
            ("lib.sh", "exit 1\n", false),
        ],
    );
    let ran = run(&tree, "./check.sh", |tree| {
        fs::write(tree.join("lib.sh"), "exit 0\n").expect("a file");
    });
    ran.assert_no_forged_pass();
}

/// `bash -c ./check.sh`: the script is the word after `-c`'s command, which is not taken.
#[test]
fn adv1_script_named_inside_bash_c_earns_no_pass() {
    let tree = repository("bash-c", &[("check.sh", FAILING_CHECK, true)]);
    let ran = run(&tree, "bash -c ./check.sh", |tree| {
        write_executable(tree, "check.sh", PASSING_CHECK);
    });
    ran.assert_no_forged_pass();
}

/// `env -u NAME ./check.sh`: the option's argument is taken for the program.
#[test]
fn adv1_script_after_env_u_earns_no_pass() {
    let tree = repository("env-u", &[("check.sh", FAILING_CHECK, true)]);
    let ran = run(&tree, "env -u TAKT_ADV1_UNSET ./check.sh", |tree| {
        write_executable(tree, "check.sh", PASSING_CHECK);
    });
    ran.assert_no_forged_pass();
}

/// `sh -o errexit tests/run.sh`: the option's argument is taken for the script.
#[test]
fn adv1_script_after_sh_o_earns_no_pass() {
    let tree = repository("sh-o", &[("tests/run.sh", FAILING_CHECK, false)]);
    let ran = run(&tree, "sh -o errexit tests/run.sh", |tree| {
        write(tree, "tests/run.sh", PASSING_CHECK);
    });
    ran.assert_no_forged_pass();
}

/// `timeout 600 ./check.sh`: a wrapper that is neither `env` nor an interpreter.
#[test]
fn adv1_script_under_a_wrapper_earns_no_pass() {
    let tree = repository("timeout", &[("check.sh", FAILING_CHECK, true)]);
    let command = format!("{} 600 ./check.sh", wrapper(&tree, "timeout", 1));
    let ran = run(&tree, &command, |tree| {
        write_executable(tree, "check.sh", PASSING_CHECK);
    });
    ran.assert_no_forged_pass();
}

/// `make test`: the recipe file decides what runs (named as not done in the commit).
#[test]
fn adv1_make_recipe_rewritten_earns_no_pass() {
    let tree = repository("make", &[("Makefile", "test:\n\tfalse\n", false)]);
    let ran = run(&tree, "make test", |tree| {
        fs::write(tree.join("Makefile"), "test:\n\ttrue\n").expect("a file");
    });
    ran.assert_no_forged_pass();
}
