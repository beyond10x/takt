//! story:a-command-action through the binary: a protocol action the run file's `command_actions`
//! binds to an argument list is performed by one generic validator, the argv without a shell, in
//! the tree, with no credential, `{artifacts}` the case's artifacts directory; its exit gives the
//! record's result, `pass`, `unchanged`, `done` or `fail`; a write it does not declare is refused
//! and put back; a binding of an action the protocol does not declare is refused at its line.
//!
//! The command is `sh render.sh {artifacts} <exit> <mode>`, a script committed in the tree that
//! prints its environment into `status/out.txt` under the artifacts directory and exits `<exit>`;
//! `<mode>` `case` also writes a second file under the case, `tree` a file in the tree.

#[path = "../../takt/tests/support/own_home.rs"]
mod own_home;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// A protocol whose one action, `status.render`, produces `status_rendered`, and whose three
/// outcomes each hold on one result of it.
const PROTOCOL: &str = "format: protocol/1

protocol:
  id: command.probe
  revision: 1
  description: renders a status by a command action

artifacts:
  implementation:
    description: a revision of the repository

evidence_kinds:
  status_rendered:
    description: what the render command came to; pass, unchanged, done or fail

claims:
  status.rendered:
    description: the render wrote a new status
    true_when:
      evidence:
        kind: status_rendered
        result: pass
        subject: implementation
  status.unchanged:
    description: the render found the status unchanged
    true_when:
      evidence:
        kind: status_rendered
        result: unchanged
        subject: implementation
  status.done:
    description: the render found nothing left to do
    true_when:
      evidence:
        kind: status_rendered
        result: done
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
  unchanged:
    description: the status is unchanged
    requires:
      claim: status.unchanged
  done:
    description: nothing is left to do
    requires:
      claim: status.done
";

/// The script the binding runs: its environment into its file, then the extra write `$3` names,
/// then exit `$2`.
const SCRIPT: &str = "mkdir -p \"$1/status\"
env > \"$1/status/out.txt\"
case \"$3\" in
  case) echo stray > \"$1/../stray.txt\" ;;
  tree) echo stray > tree-file.txt ;;
esac
exit \"$2\"
";

fn git(tree: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(args)
        .current_dir(tree)
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?}");
}

/// A scratch directory under this crate's build directory, removed on drop: `tree` the
/// repository, `state` the state directory, the protocol and the run file beside them.
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

    fn config(&self) -> PathBuf {
        self.0.join("run.yaml")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// A committed repository holding the script, the protocol and the run file `config`.
fn scratch(name: &str, config: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("command-action-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    let scratch = Scratch(dir);
    let tree = scratch.tree();
    fs::create_dir_all(&tree).expect("the tree");
    fs::create_dir_all(scratch.state()).expect("the state directory");
    fs::write(tree.join("render.sh"), SCRIPT).expect("the script");
    git(&tree, &["init", "-q", "-b", "main"]);
    git(&tree, &["add", "-A"]);
    git(&tree, &["commit", "-q", "-m", "start"]);
    fs::write(scratch.protocol(), PROTOCOL).expect("the protocol");
    fs::write(scratch.config(), config).expect("the run file");
    scratch
}

/// A run file binding `status.render` to the script, exiting `exit`, with `mode`.
fn binding(exit: u8, mode: &str) -> String {
    format!(
        "command_actions:\n  status.render:\n    argv: [sh, render.sh, \"{{artifacts}}\", \"{exit}\", {mode}]\n    writes: status/out.txt\n    evidence: status_rendered\n    unchanged_exit: 3\n    done_exit: 4\n"
    )
}

/// The built `takt run` on an intent in the scratch tree, with no harness and no forge, on the
/// probe protocol and the run file, `env` set in its environment: exit code, stdout, stderr.
fn run(scratch: &Scratch, env: &[(&str, &str)]) -> (i32, String, String) {
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
            "true",
            "--intent",
            "render the status",
        ])
        .arg("--protocol")
        .arg(scratch.protocol())
        .arg("--config")
        .arg(scratch.config())
        .arg("--state-dir")
        .arg(scratch.state())
        .envs(env.iter().copied())
        .output()
        .expect("takt runs");
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// The case directories under the tree's cases directory.
fn cases(tree: &Path) -> Vec<PathBuf> {
    fs::read_dir(tree.join(".takt/cases"))
        .into_iter()
        .flatten()
        .map(|entry| entry.expect("an entry").path())
        .filter(|path| path.is_dir())
        .collect()
}

/// The one case directory.
fn the_case(tree: &Path) -> PathBuf {
    let cases = cases(tree);
    assert_eq!(cases.len(), 1, "{cases:?}");
    cases[0].clone()
}

/// The `status_rendered` records under the case's `evidence/`.
fn rendered(case: &Path) -> Vec<serde_json::Value> {
    fs::read_dir(case.join("evidence"))
        .into_iter()
        .flatten()
        .map(|entry| entry.expect("an entry").path())
        .filter_map(|path| fs::read_to_string(path).ok())
        .filter_map(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .filter(|record| record["kind"] == "status_rendered")
        .collect()
}

#[test]
fn command_action_results() {
    for (exit, result, outcome) in [
        (0, "pass", Some("rendered")),
        (3, "unchanged", Some("unchanged")),
        (4, "done", Some("done")),
        (1, "fail", None),
    ] {
        let scratch = scratch(&format!("results-{exit}"), &binding(exit, "plain"));
        let (code, out, err) = run(&scratch, &[]);
        let case = the_case(&scratch.tree());
        let records = rendered(&case);
        assert_eq!(
            records.len(),
            1,
            "exit {exit}: one record\nstdout {out}\nstderr {err}"
        );
        let record = &records[0];
        assert_eq!(record["result"], result, "exit {exit}: {record}\n{out}");
        assert_eq!(record["format"], "canon-evidence/1", "{record}");
        assert_eq!(record["subject"], "implementation", "{record}");
        assert!(
            record["subject_revision"]
                .as_str()
                .is_some_and(|revision| !revision.is_empty()),
            "{record}"
        );
        assert!(
            case.join("artifacts/status/out.txt").is_file(),
            "exit {exit}: the declared file is kept under artifacts/\n{out}"
        );
        if let Some(outcome) = outcome {
            assert_eq!(code, 0, "exit {exit}: stdout {out} stderr {err}");
            assert!(
                out.contains(&format!("stopped: Completed ({outcome})")),
                "exit {exit}: {out}"
            );
        }
    }
}

#[test]
fn command_action_undeclared_write() {
    for (mode, stray) in [("case", None), ("tree", Some("tree-file.txt"))] {
        let scratch = scratch(&format!("undeclared-{mode}"), &binding(0, mode));
        let (_, out, err) = run(&scratch, &[]);
        assert!(
            out.contains("wrote what its validator does not"),
            "{mode}: the write is refused\nstdout {out}\nstderr {err}"
        );
        let case = the_case(&scratch.tree());
        assert!(
            !case.join("stray.txt").exists(),
            "{mode}: the second file under the case is put back\n{out}"
        );
        if let Some(stray) = stray {
            assert!(
                !scratch.tree().join(stray).exists(),
                "{mode}: the file in the tree is put back\n{out}"
            );
        }
        assert!(
            rendered(&case).is_empty(),
            "{mode}: nothing is recorded\n{out}"
        );
    }
}

#[test]
fn command_action_no_credential() {
    let scratch = scratch("no-credential", &binding(0, "plain"));
    let (code, out, err) = run(
        &scratch,
        &[
            ("GITHUB_TOKEN", "github-secret-value"),
            ("CONNECTORS_TOKEN", "connectors-secret-value"),
        ],
    );
    assert_eq!(code, 0, "stdout {out} stderr {err}");
    let case = the_case(&scratch.tree());
    let printed =
        fs::read_to_string(case.join("artifacts/status/out.txt")).expect("the child's file");
    assert!(
        printed.contains("PATH="),
        "the child printed its environment: {printed}"
    );
    for (name, value) in [
        ("GITHUB_TOKEN", "github-secret-value"),
        ("CONNECTORS_TOKEN", "connectors-secret-value"),
    ] {
        assert!(
            !printed.contains(name),
            "{name} reached the child: {printed}"
        );
        assert!(!printed.contains(value), "{name}'s value reached the child");
    }
}

#[test]
fn command_action_unknown_action() {
    let config = "command_actions:\n  status.publish:\n    argv: [sh, render.sh, \"{artifacts}\", \"0\", plain]\n    writes: status/out.txt\n    evidence: status_rendered\n";
    let scratch = scratch("unknown", config);
    let (code, out, err) = run(&scratch, &[]);
    assert_eq!(code, 2, "stdout {out} stderr {err}");
    assert!(
        err.contains(&format!(
            "the run configuration file {} is refused at line 2",
            scratch.config().display()
        )),
        "{err}"
    );
    assert!(err.contains("`command_actions`"), "{err}");
    assert!(
        cases(&scratch.tree())
            .iter()
            .all(|case| rendered(case).is_empty()),
        "nothing is recorded"
    );
}
