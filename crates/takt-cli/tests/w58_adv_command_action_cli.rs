//! Wave 58's adversarial pass on story:a-command-action, through the binary. Each case drives
//! `takt run` with a run file's `command_actions` binding and asserts what the story, the design
//! (`docs/design/scheduled-runs-and-connector-actions.md` § Command action), the specification
//! (`takt.work.CommandAction`) or the `Validator` trait says of it.

#[path = "../../takt/tests/support/own_home.rs"]
mod own_home;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

fn git(tree: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(args)
        .current_dir(tree)
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?}");
}

/// A scratch directory under this crate's build directory, removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn tree(&self) -> PathBuf {
        self.0.join("tree")
    }

    fn state(&self) -> PathBuf {
        self.0.join("state")
    }

    fn protocol(&self) -> PathBuf {
        self.0.join("probe.yaml")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// A committed repository holding `script` as `act.sh`, the protocol beside it.
fn scratch(name: &str, protocol: &str, script: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "w58-adv-command-action-{}-{name}",
        std::process::id()
    ));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    let scratch = Scratch(dir);
    let tree = scratch.tree();
    fs::create_dir_all(&tree).expect("the tree");
    fs::create_dir_all(scratch.state()).expect("the state directory");
    fs::write(tree.join("act.sh"), script).expect("the script");
    git(&tree, &["init", "-q", "-b", "main"]);
    git(&tree, &["add", "-A"]);
    git(&tree, &["commit", "-q", "-m", "start"]);
    fs::write(scratch.protocol(), protocol).expect("the protocol");
    scratch
}

/// The built `takt run` on an intent, no harness, no forge, the run file `config`, `extra`
/// flags after the rest: exit code, stdout, stderr.
fn run(scratch: &Scratch, config: &Path, test_cmd: &str, extra: &[&str]) -> (i32, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args(["run", "--repo"])
        .arg(scratch.tree())
        .args([
            "--harness",
            "none",
            "--forge",
            "none",
            "--iterations",
            "3",
            "--test-cmd",
            test_cmd,
            "--intent",
            "render the status",
        ])
        .arg("--protocol")
        .arg(scratch.protocol())
        .arg("--config")
        .arg(config)
        .arg("--state-dir")
        .arg(scratch.state())
        .args(extra)
        .output()
        .expect("takt runs");
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

fn cases(tree: &Path) -> Vec<PathBuf> {
    fs::read_dir(tree.join(".takt/cases"))
        .into_iter()
        .flatten()
        .map(|entry| entry.expect("an entry").path())
        .filter(|path| path.is_dir())
        .collect()
}

fn records(case: &Path) -> Vec<serde_json::Value> {
    fs::read_dir(case.join("evidence"))
        .into_iter()
        .flatten()
        .map(|entry| entry.expect("an entry").path())
        .filter_map(|path| fs::read_to_string(path).ok())
        .filter_map(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .collect()
}

/// A protocol whose `status.render` may produce `status_rendered` alone, and whose `tests.run`,
/// the built-in, produces `test_result` once the render passed. The outcome reads the tests.
const TESTS_AFTER_RENDER: &str = "format: protocol/1

protocol:
  id: command.forge
  revision: 1
  description: renders a status, then runs the tests

artifacts:
  implementation:
    description: a revision of the repository

evidence_kinds:
  status_rendered:
    description: what the render command came to
  test_result:
    description: the result of the test command

claims:
  status.rendered:
    description: the render wrote a new status
    true_when:
      evidence:
        kind: status_rendered
        result: pass
        subject: implementation
  tests.pass:
    description: the tests pass
    true_when:
      evidence:
        kind: test_result
        result: pass
        subject: implementation

actions:
  status.render:
    description: render the status
    effect: read
    may_produce:
      - evidence: status_rendered
  tests.run:
    description: run the tests once the status is rendered
    effect: read
    precondition:
      claim: status.rendered
    may_produce:
      - evidence: test_result

outcomes:
  tested:
    description: the tests pass
    requires:
      claim: tests.pass
";

/// The protocol of the unit's own tests: `status.render` produces `status_rendered`, whose
/// `pass` completes the case.
const RENDER_ONLY: &str = "format: protocol/1

protocol:
  id: command.probe
  revision: 1
  description: renders a status by a command action

artifacts:
  implementation:
    description: a revision of the repository

evidence_kinds:
  status_rendered:
    description: what the render command came to

claims:
  status.rendered:
    description: the render wrote a new status
    true_when:
      evidence:
        kind: status_rendered
        result: pass
        subject: implementation

actions:
  status.render:
    description: render the status
    effect: read
    may_produce:
      - evidence: status_rendered

outcomes:
  rendered:
    description: a new status was rendered
    requires:
      claim: status.rendered
";

const WRITE_AND_PASS: &str =
    "mkdir -p \"$1/status\"\necho rendered > \"$1/status/out.txt\"\nexit 0\n";

/// The binding's `evidence` is a kind its action may not produce, and the kind the built-in
/// `tests.run` writes. The story: a record "of the bound kind"; the design's binding example:
/// "the kind the protocol's may_produce names"; `AGENTS.md` § Rules: a claim about the work
/// (`tests.pass`) is made true only by the validator that observes it. With the test command
/// `false`, the case must not complete on `tests.pass`: a command action may not write a
/// `test_result` its action does not declare.
#[test]
fn a_binding_writes_no_record_of_a_kind_its_action_may_not_produce() {
    let scratch = scratch("forged-kind", TESTS_AFTER_RENDER, WRITE_AND_PASS);
    let config = scratch.0.join("run.yaml");
    fs::write(
        &config,
        "command_actions:\n  status.render:\n    argv: [sh, act.sh, \"{artifacts}\"]\n    writes: status/out.txt\n    evidence: test_result\n",
    )
    .expect("the run file");
    let (code, out, err) = run(&scratch, &config, "false", &[]);
    let forged: Vec<serde_json::Value> = cases(&scratch.tree())
        .iter()
        .flat_map(|case| records(case))
        .filter(|record| record["kind"] == "test_result" && record["producer"] != "takt.tests-run")
        .filter(|record| {
            record["producer"]
                .as_str()
                .is_some_and(|producer| producer.contains("command-action"))
        })
        .collect();
    assert!(
        forged.is_empty(),
        "a command action wrote a `test_result` that status.render's may_produce does not name: {forged:?}\nexit {code}\nstdout {out}\nstderr {err}"
    );
    assert!(
        !out.contains("Completed (tested)"),
        "the case completed on tests.pass with the test command `false`\nstdout {out}\nstderr {err}"
    );
}

/// The child leaves its declared output as a link to a file outside the tree. The story: "its
/// one output file is the declared `writes`, kept under `artifacts/`"; what it left is no file
/// under `artifacts/`, and the next consumer of it (a write action's `input_from`) reads where it
/// points. takt refuses a link a step leaves elsewhere (`deployment_paths`, the cases' way).
#[test]
fn the_declared_output_left_as_a_link_out_of_artifacts_is_not_kept() {
    let script = "mkdir -p \"$1/status\"\nln -s \"$2\" \"$1/status/out.txt\"\nexit 0\n";
    let scratch = scratch("link-out", RENDER_ONLY, script);
    let outside = scratch.0.join("outside.txt");
    fs::write(&outside, "{\"operation\": \"chat.postMessage\"}\n").expect("the outside file");
    let config = scratch.0.join("run.yaml");
    fs::write(
        &config,
        format!(
            "command_actions:\n  status.render:\n    argv: [sh, act.sh, \"{{artifacts}}\", \"{}\"]\n    writes: status/out.txt\n    evidence: status_rendered\n",
            outside.display()
        ),
    )
    .expect("the run file");
    let (code, out, err) = run(&scratch, &config, "true", &[]);
    for case in cases(&scratch.tree()) {
        let declared = case.join("artifacts/status/out.txt");
        let link = fs::symlink_metadata(&declared)
            .map(|meta| meta.file_type().is_symlink())
            .unwrap_or(false);
        assert!(
            !link,
            "{} is kept as a link to {}\nexit {code}\nstdout {out}\nstderr {err}",
            declared.display(),
            fs::read_link(&declared).unwrap_or_default().display()
        );
    }
}

/// The `Validator` trait: `observe_within` holds "a child it runs" to the run's deadline, and
/// "by default `deadline` is not read: the validator runs no child of its own". A command action
/// runs a child of its own and keeps the default: under `--hours` its child outlives the run's
/// budget, up to its own five minutes.
#[test]
fn a_command_action_child_is_held_to_the_runs_hour_budget() {
    let script = "mkdir -p \"$1/status\"\necho started > \"$1/status/out.txt\"\nsleep 40\nexit 0\n";
    let scratch = scratch("deadline", RENDER_ONLY, script);
    let config = scratch.0.join("run.yaml");
    fs::write(
        &config,
        "command_actions:\n  status.render:\n    argv: [sh, act.sh, \"{artifacts}\"]\n    writes: status/out.txt\n    evidence: status_rendered\n",
    )
    .expect("the run file");
    let began = Instant::now();
    // 0.003 h is 10.8 s: the case opens well inside it and the child sleeps past it.
    let (code, out, err) = run(&scratch, &config, "true", &["--hours", "0.003"]);
    let took = began.elapsed();
    assert!(
        took < Duration::from_secs(30),
        "the run took {took:?} against an hour budget of 10.8 s: the command action's child was not held to it\nexit {code}\nstdout {out}\nstderr {err}"
    );
}
