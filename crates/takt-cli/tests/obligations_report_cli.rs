//! story:the-run-shows-each-repository-obligation through the `takt` binary: a run whose gate
//! fails prints the obligations line before its stop line, and `takt report --case` prints the
//! table, in text and in `--format json`, with the same row.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const GATE_PROTOCOL: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../takt/tests/fixtures/gate-only/1.yaml"
);

const GATES: &str = "format: engineering-gates/1\nlanguage: canon-expr/1\ngates:\n  - id: release-notes\n    \
                     text: A change records a release note in CHANGELOG.md.\n    assertions:\n      \
                     - 'git.changed(\"CHANGELOG.md\")'\n";

fn git(tree: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(args)
        .current_dir(tree)
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?}");
}

struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn takt(args: &[&str]) -> (i32, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args(args)
        .output()
        .expect("takt runs");
    (
        output.status.code().expect("an exit code"),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[test]
fn a_failed_obligation_is_in_the_run_line_and_in_the_report() {
    let repo = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("obligations-report-cli-{}", std::process::id()));
    let _ = fs::remove_dir_all(&repo);
    fs::create_dir_all(&repo).expect("a directory");
    let _scratch = Scratch(repo.clone());
    fs::write(repo.join("CHANGELOG.md"), "# Changelog\n").expect("a file");
    fs::write(repo.join("gates.yaml"), GATES).expect("a file");
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "start"]);
    let repo_arg = repo.to_str().expect("utf-8");

    let (code, out, err) = takt(&[
        "run",
        "--repo",
        repo_arg,
        "--intent",
        "record a note",
        "--case-id",
        "note",
        "--harness",
        "none",
        "--protocol",
        GATE_PROTOCOL,
        "--test-cmd",
        "true",
        "--gates",
        "gates.yaml",
    ]);
    assert_eq!(code, 3, "stdout {out} stderr {err}");
    let obligations = out
        .lines()
        .position(|line| line == "obligations: 1 obligation open: release-notes fail")
        .unwrap_or_else(|| panic!("the obligations line: {out}"));
    let stopped = out
        .lines()
        .position(|line| line.starts_with("stopped: "))
        .unwrap_or_else(|| panic!("the stop line: {out}"));
    assert!(obligations < stopped, "{out}");

    let case = repo.join(".takt/cases/note");
    let case_arg = case.to_str().expect("utf-8");
    let (code, out, err) = takt(&["report", "--case", case_arg]);
    assert_eq!(code, 0, "stdout {out} stderr {err}");
    assert!(
        out.lines()
            .any(|line| line.starts_with("| release-notes | fail |")),
        "{out}"
    );
    let (code, out, err) = takt(&["report", "--case", case_arg, "--format", "json"]);
    assert_eq!(code, 0, "stdout {out} stderr {err}");
    let json: serde_json::Value = serde_json::from_str(&out).expect("one JSON document");
    assert_eq!(json["runs"][0]["obligations"][0]["id"], "release-notes");
    assert_eq!(json["runs"][0]["obligations"][0]["status"], "fail");
}
