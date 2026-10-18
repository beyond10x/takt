//! story:test-summary-keeps-the-totals through the binary: the `tests.run` effect line keeps
//! what the test command said about its tests, not only the last five lines of its output. For
//! `cargo test`, whose output ends with the doc-test block of the last crate (`running 0 tests`,
//! `0 passed`), the summary sums every `test result:` line and names the tests that failed; for
//! any other command it keeps the first line beside the last ones, so a summary printed first is
//! not dropped.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

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

impl Drop for Scratch {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            let message = format!("remove the scratch tree {}: {error}", self.0.display());
            if std::thread::panicking() {
                eprintln!("{message}");
            } else {
                panic!("{message}");
            }
        }
    }
}

/// A committed repository at `<scratch>/repo` whose test command `./t.sh` prints `output` and
/// exits `code`.
fn repository(name: &str, output: &str, code: u8) -> (Scratch, PathBuf) {
    let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("test-summary-{}-{name}", std::process::id()));
    if outer.exists() {
        fs::remove_dir_all(&outer).expect("a stale scratch tree is removed");
    }
    let repo = outer.join("repo");
    fs::create_dir_all(&repo).expect("a directory");
    fs::write(repo.join("README.md"), "a story\n").expect("a file");
    let script = repo.join("t.sh");
    fs::write(
        &script,
        format!("#!/bin/sh\ncat <<'OUTPUT'\n{output}\nOUTPUT\nexit {code}\n"),
    )
    .expect("the test script");
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).expect("executable");
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "start"]);
    (Scratch(outer), repo)
}

/// One `takt run` with no harness and no forge over `repo`, one step: `tests.run`. Returns the
/// `effect:` line of that step.
fn effect_line(repo: &Path) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args([
            "run",
            "--repo",
            repo.to_str().expect("utf-8"),
            "--intent",
            "probe",
            "--harness",
            "none",
            "--forge",
            "none",
            "--iterations",
            "1",
            "--test-cmd",
            "./t.sh",
        ])
        .output()
        .expect("takt runs");
    let out = String::from_utf8_lossy(&output.stdout);
    let err = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(3),
        "one iteration then the budget stop: stdout {out} stderr {err}"
    );
    out.lines()
        .find(|line| {
            line.trim_start()
                .starts_with("effect: the test command exited")
        })
        .unwrap_or_else(|| panic!("an effect line for tests.run in:\n{out}\n{err}"))
        .to_owned()
}

/// The finding's predicate: a command whose first line is its summary and whose last six lines
/// are filler. The effect line keeps the summary.
#[test]
fn a_summary_printed_before_filler_is_kept() {
    let output = "SUITES-26-PASSED\ntail-1\ntail-2\ntail-3\ntail-4\ntail-5\ntail-6";
    let (_outer, repo) = repository("filler", output, 0);
    let effect = effect_line(&repo);
    assert!(
        effect.contains("exited 0: pass"),
        "the exit and the result stay: {effect}"
    );
    assert!(
        effect.contains("SUITES-26-PASSED"),
        "the first line is kept: {effect}"
    );
    assert!(effect.contains("tail-6"), "the last line is kept: {effect}");
}

/// `cargo test` output over three binaries, the last a doc-test block with no tests: the
/// summary sums the result lines instead of quoting the last block.
#[test]
fn cargo_result_lines_are_summed_over_every_block() {
    let output = "\nrunning 12 tests\n............\ntest result: ok. 12 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 0.40s\n\nrunning 8 tests\n........\ntest result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.20s\n\n   Doc-tests b10x_example\n\nrunning 0 tests\n\ntest result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s";
    let (_outer, repo) = repository("cargo-pass", output, 0);
    let effect = effect_line(&repo);
    assert!(
        effect.contains("20 passed, 0 failed, 2 ignored over 3 result lines"),
        "the totals over every block: {effect}"
    );
    assert!(
        !effect.contains("0 passed;"),
        "the last block alone is not the summary: {effect}"
    );
}

/// A failing `cargo test`: the totals count the failure and the summary names the tests that
/// failed, even though the output ends with a block that passed.
#[test]
fn a_failing_cargo_run_names_its_failed_tests() {
    let output = "\nrunning 3 tests\ntest cache::tmp_is_cache ... FAILED\ntest cache::profile_is_cache ... ok\ntest cache::record_is_kept ... FAILED\n\nfailures:\n\n---- cache::tmp_is_cache stdout ----\nassertion failed\n\nfailures:\n    cache::record_is_kept\n    cache::tmp_is_cache\n\ntest result: FAILED. 1 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s\n\nrunning 0 tests\n\ntest result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s";
    let (_outer, repo) = repository("cargo-fail", output, 101);
    let effect = effect_line(&repo);
    assert!(
        effect.contains("exited 101: fail"),
        "the exit and the result stay: {effect}"
    );
    assert!(
        effect.contains("1 passed, 2 failed, 0 ignored over 2 result lines"),
        "the totals over every block: {effect}"
    );
    assert!(
        effect.contains("failed: cache::tmp_is_cache, cache::record_is_kept"),
        "the failed tests by name, in output order: {effect}"
    );
}

/// Output of five lines or fewer with no result line is kept whole, in order.
#[test]
fn short_output_is_kept_whole() {
    let (_outer, repo) = repository("short", "one\ntwo\nthree", 0);
    let effect = effect_line(&repo);
    assert!(
        effect.ends_with("one | two | three"),
        "every line, in order: {effect}"
    );
}
