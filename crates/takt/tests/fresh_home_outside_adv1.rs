//! Adversary cases for story:fresh-home-first-test-run-is-not-refused: what a `tests.run` may
//! leave under a `$CARGO_HOME` (and `$RUSTUP_HOME`) that cargo or rustup reads or runs later,
//! and what takt does to `~/.cargo` when it refuses a program planted in a fresh home.

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

const CODE: &str = "def answer():\n    return 0\n";
const FIXED: &str = "def answer():\n    return 42\n";
const TEST: &str = "import unittest\n\nimport todo\n\n\nclass T(unittest.TestCase):\n    def test_answer(self):\n        self.assertEqual(todo.answer(), 42)\n";

/// The variable naming the variant [`adv_fresh_home_inner`] runs.
const VARIANT: &str = "TAKT_ADV1_FRESH_VARIANT";
/// The variable naming the home [`adv_fresh_home_inner`] runs under.
const HOME_VAR: &str = "TAKT_ADV1_FRESH_HOME";

/// The registry source directory cargo unpacks `serde 1.0.228` into from crates.io.
const SRC: &str = "registry/src/index.crates.io-1949cf8c6b5b557f/serde-1.0.228";

/// The test command's script: it plants a crate source cargo takes as already unpacked
/// (`.cargo-ok` holding `{"v":1}`) with a `build.rs`, beside the caches cargo makes, then runs
/// the tests. Code under test run by `tests.run` can do the same.
fn registry_check() -> String {
    format!(
        "#!/bin/sh\nset -eu\ncd \"$(dirname \"$0\")\"\nmkdir -p \"$CARGO_HOME/registry/cache\" \"$CARGO_HOME/{SRC}/src\"\nprintf '{{\"v\":1}}' > \"$CARGO_HOME/{SRC}/.cargo-ok\"\nprintf 'fn main() {{ std::process::Command::new(\"sh\").arg(\"-c\").arg(\"id\").status().ok(); }}\\n' > \"$CARGO_HOME/{SRC}/build.rs\"\nPYTHONPATH=. python3 -m unittest discover -s tests -v\n"
    )
}

/// The test command's script: the caches cargo makes and a `cargo-forge` program in
/// `$CARGO_HOME/bin`, as the unit's own `bin` variant writes them.
const PLANTING_CHECK: &str = "#!/bin/sh\nset -eu\ncd \"$(dirname \"$0\")\"\nmkdir -p \"$CARGO_HOME/registry/cache\" \"$CARGO_HOME/git/db\" \"$CARGO_HOME/bin\"\ntouch \"$CARGO_HOME/.package-cache\"\nprintf '#!/bin/sh\\nexit 0\\n' > \"$CARGO_HOME/bin/cargo-forge\"\nchmod 755 \"$CARGO_HOME/bin/cargo-forge\"\nPYTHONPATH=. python3 -m unittest discover -s tests -v\n";

/// The test command's script: a `cargo` that prints a passing result in the tree's ignored
/// `target/`, a toolchain `evil` in `$RUSTUP_HOME` linked to it, and `settings.toml` making it
/// rustup's default, then the tests.
const RUSTUP_CHECK: &str = "#!/bin/sh\nset -eu\ncd \"$(dirname \"$0\")\"\nmkdir -p target/evil/bin\nprintf '#!/bin/sh\\necho \"test result: ok. 1 passed\"\\n' > target/evil/bin/cargo\nchmod 755 target/evil/bin/cargo\nln -s \"$PWD/target/evil\" \"$RUSTUP_HOME/toolchains/evil\"\nprintf 'version = \"12\"\\ndefault_toolchain = \"evil\"\\n' > \"$RUSTUP_HOME/settings.toml\"\nPYTHONPATH=. python3 -m unittest discover -s tests -v\n";

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

/// A repository at `root/repo` holding `check.sh`, the code under test and its test, the
/// protocol at `root`.
fn repository_in(root: &Path, check: &str) -> PathBuf {
    let repo = root.join("repo");
    fs::create_dir_all(repo.join("tests")).expect("a directory");
    fs::write(root.join("protocol.yaml"), EDIT_AND_TEST).expect("the protocol");
    fs::write(repo.join(".gitignore"), "target/\n__pycache__/\n").expect("a file");
    fs::write(repo.join("check.sh"), check).expect("a file");
    fs::set_permissions(repo.join("check.sh"), fs::Permissions::from_mode(0o755)).expect("mode");
    fs::write(repo.join("todo.py"), CODE).expect("a file");
    fs::write(repo.join("tests/test_todo.py"), TEST).expect("a file");
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "start"]);
    repo
}

struct Writes {
    done: std::cell::Cell<bool>,
}

impl Harness for Writes {
    fn name(&self) -> &str {
        "writes"
    }

    fn step(&self, context: &StepContext<'_>) -> Result<StepReport, anyhow::Error> {
        if !context.offered.iter().any(|a| a == "implementation.edit") {
            anyhow::bail!("implementation.edit not offered");
        }
        if !self.done.replace(true) {
            fs::write(context.tree.join("todo.py"), FIXED).expect("a file");
        }
        Ok(StepReport {
            action: "implementation.edit".to_owned(),
            summary: "made the tests pass".to_owned(),
            tokens: Some(1),
        })
    }
}

/// The run's stop, what it printed, and how many passing test results it recorded.
fn run(root: &Path, repo: &Path) -> (anyhow::Result<Stop>, String, usize) {
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(TestsRun::new("./check.sh").expect("a command"))];
    let harness = Writes {
        done: std::cell::Cell::new(false),
    };
    let options = RunOptions {
        protocol: root.join("protocol.yaml").display().to_string(),
        intent: "make the tests pass".to_owned(),
        inputs: Vec::new(),
        iterations: Some(4),
        hours: None,
        tokens: None,
        case_id: None,
        gate: None,
    };
    let mut run = Run::open(repo, options, &harness, &validators).expect("the case opens");
    let mut out = Vec::new();
    run.continue_past_red_base();
    let stop = run.run(&mut out);
    let passes = fs::read_dir(run.case.dir.join("evidence"))
        .map(|entries| {
            entries
                .flatten()
                .filter(|entry| {
                    let value: serde_json::Value =
                        serde_json::from_slice(&fs::read(entry.path()).expect("a record"))
                            .expect("json");
                    value["kind"] == "test_result" && value["result"] == "pass"
                })
                .count()
        })
        .unwrap_or(0);
    (stop, String::from_utf8_lossy(&out).into_owned(), passes)
}

/// Runs [`adv_fresh_home_inner`] as `variant` in a child of this test binary whose `HOME` holds
/// `make` (relative directories), with neither `CARGO_HOME` nor `RUSTUP_HOME` set.
fn variant(variant: &str, make: &[&str]) -> Option<String> {
    let scratch = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "adv1-fresh-{}-{variant}-{}",
        std::process::id(),
        make.len()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let home = scratch.join("home");
    fs::create_dir_all(&home).expect("a home");
    for dir in make {
        fs::create_dir_all(home.join(dir)).expect("a directory");
    }
    let output = Command::new(std::env::current_exe().expect("this test binary"))
        .args([
            "--exact",
            "adv_fresh_home_inner",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("HOME", &home)
        .env_remove("CARGO_HOME")
        .env_remove("RUSTUP_HOME")
        .env(VARIANT, variant)
        .env(HOME_VAR, &home)
        .output()
        .expect("the test binary runs");
    let _ = fs::remove_dir_all(&scratch);
    let text = String::from_utf8_lossy(&output.stdout) + String::from_utf8_lossy(&output.stderr);
    if !output.status.success() {
        let why = text
            .lines()
            .find(|line| line.contains("ADV1"))
            .unwrap_or(&text);
        return Some(format!("{variant}: {why}"));
    }
    (!text.contains("1 passed")).then(|| format!("{variant}: the inner case did not run: {text}"))
}

/// A crate source planted under a fresh home's `~/.cargo/registry/src`, marked unpacked, is
/// compiled (its `build.rs` run) by the next `cargo build` that resolves that version, inside
/// takt and outside it. Today the run is accepted and the source left, as it is in an existing
/// `~/.cargo` (the next case): a known gap, story:cargo-home-sources-are-step-changes, which
/// turns this case into "the run stops naming it".
#[test]
fn adv_a_crate_source_planted_in_a_fresh_cargo_home_is_accepted_today() {
    assert_eq!(variant("registry", &[]), None);
}

/// The same planting under a `~/.cargo` that held cargo's cache when the run began, accepted
/// before this unit's change too (story:cargo-home-sources-are-step-changes).
#[test]
fn adv_a_crate_source_planted_in_an_existing_cargo_home_is_accepted_today() {
    assert_eq!(variant("registry", &[".cargo/registry/cache"]), None);
}

/// ESS `takt.yaml` (the clause on what cargo reads outside the tree): a change in a program
/// directory "is left exactly as it is, no file removed and no directory it made removed".
/// Today a program planted in a fresh home's `~/.cargo/bin` stops the run, but putting the
/// configuration path `~/.cargo` back removes it whole: the program, the directory made for it
/// and the cache beside it. A known gap, story:a-refused-step-leaves-the-cargo-home-it-made,
/// which turns this case into "all three are left".
#[test]
fn adv_a_program_planted_in_a_fresh_cargo_bin_is_removed_with_cargo_home_today() {
    assert_eq!(variant("bin-left", &[]), None);
}

/// A toolchain linked into the tree's ignored output and made rustup's default in
/// `$RUSTUP_HOME/settings.toml`: the next `cargo` a rustup proxy runs is the planted one.
/// Today the run is accepted: a known gap, story:a-toolchain-linked-into-the-tree-is-a-step-change,
/// which turns this case into "the run stops naming it".
#[test]
fn adv_a_rustup_default_toolchain_linked_into_the_tree_is_accepted_today() {
    assert_eq!(
        variant("rustup", &[".cargo/bin", ".rustup/toolchains/stable/bin"]),
        None
    );
}

/// The run one variant makes, under the home [`variant`] names; nothing without it.
#[test]
fn adv_fresh_home_inner() {
    let (Some(which), Some(home)) = (
        std::env::var(VARIANT).ok(),
        std::env::var_os(HOME_VAR).map(PathBuf::from),
    ) else {
        return;
    };
    let root = home.join("work");
    let check = match which.as_str() {
        "registry" => registry_check(),
        "bin-left" => PLANTING_CHECK.to_owned(),
        "rustup" => RUSTUP_CHECK.to_owned(),
        other => panic!("ADV1 no variant {other}"),
    };
    let repo = repository_in(&root, &check);
    let (stop, text, passes) = run(&root, &repo);
    match which.as_str() {
        "registry" => {
            let planted = home.join(".cargo").join(SRC).join("build.rs");
            assert!(
                matches!(&stop, Ok(Stop::Completed(outcome)) if outcome == "accepted")
                    && passes == 1
                    && planted.is_file(),
                "ADV1 the current behaviour changed (accepted, 1 pass, the planted source left in place: {}): stop {:?}, {passes} pass(es). If story:cargo-home-sources-are-step-changes landed, assert instead that the run stops StepRefused naming {} and records no pass\n{text}",
                planted.is_file(),
                stop.as_ref().map_err(ToString::to_string),
                planted.display()
            );
        }
        "bin-left" => {
            assert!(
                matches!(stop, Ok(Stop::StepRefused(_))),
                "ADV1 not refused: {:?}\n{text}",
                stop.as_ref().map_err(ToString::to_string)
            );
            let program = home.join(".cargo/bin/cargo-forge");
            assert!(
                !program.exists()
                    && !home.join(".cargo").exists()
                    && !home.join(".cargo/registry/cache").exists(),
                "ADV1 the current behaviour changed (putting ~/.cargo back removes it whole; now the program {} exists: {}, ~/.cargo: {}, its cache: {}). If story:a-refused-step-leaves-the-cargo-home-it-made landed, assert instead that the program, ~/.cargo/bin and the cache are all left\n{text}",
                program.display(),
                program.exists(),
                home.join(".cargo").exists(),
                home.join(".cargo/registry/cache").is_dir()
            );
        }
        "rustup" => {
            assert!(
                fs::symlink_metadata(home.join(".rustup/toolchains/evil")).is_ok(),
                "ADV1 the script did not plant\n{text}"
            );
            assert!(
                matches!(&stop, Ok(Stop::Completed(outcome)) if outcome == "accepted")
                    && passes == 1,
                "ADV1 the current behaviour changed (accepted, 1 pass): stop {:?}, {passes} pass(es). If story:a-toolchain-linked-into-the-tree-is-a-step-change landed, assert instead that the run stops StepRefused naming the toolchain or settings.toml and records no pass\n{text}",
                stop.as_ref().map_err(ToString::to_string)
            );
        }
        _ => unreachable!(),
    }
}
