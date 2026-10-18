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

/// Counts a test command prints are summed without trusting them to fit: a count near
/// `u64::MAX` saturates instead of panicking (debug) or wrapping a failure away (release).
#[test]
fn counts_are_summed_saturating() {
    let output = "test result: ok. 18446744073709551614 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n\ntest result: ok. 5 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s";
    let (_outer, repo) = repository("saturating", output, 101);
    let effect = effect_line(&repo);
    assert!(
        effect.contains("18446744073709551615 passed, 2 failed, 0 ignored over 2 result lines"),
        "the sum saturates and the failures stay counted: {effect}"
    );
}

/// A test binary killed by a signal after a passing block: the result lines show no failure,
/// the exit does, so the last line is kept beside the counts.
#[test]
fn a_failure_the_result_lines_do_not_show_keeps_the_last_line() {
    let output = "\nrunning 5 tests\n.....\ntest result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.20s\n\nerror: process didn't exit successfully: `target/debug/deps/cache-1a2b` (signal: 11, SIGSEGV: invalid memory reference)";
    let (_outer, repo) = repository("signal", output, 101);
    let effect = effect_line(&repo);
    assert!(
        effect.contains("5 passed, 0 failed, 0 ignored over 1 result line"),
        "{effect}"
    );
    assert!(
        effect.contains("(signal: 11, SIGSEGV: invalid memory reference)"),
        "the line the counts cannot show is kept: {effect}"
    );
}

/// A runner whose `test result:` line carries no counts (not cargo's), failing: the last line,
/// which holds its own totals, is kept.
#[test]
fn a_runner_with_a_countless_result_line_keeps_its_own_totals() {
    let output = "test result: see below\n............F\n1 failed, 12 passed in 0.53s";
    let (_outer, repo) = repository("pytest-like", output, 1);
    let effect = effect_line(&repo);
    assert!(
        effect.contains("1 failed, 12 passed in 0.53s"),
        "the runner's own totals are kept: {effect}"
    );
}

/// Under `cargo test --quiet` no `test <name> ... FAILED` lines are printed; the failed tests
/// are named from the `failures:` list.
#[test]
fn quiet_cargo_output_names_the_failed_tests_from_the_failures_list() {
    let output = "\nrunning 3 tests\n.FF\nfailures:\n\n---- cache::tmp_is_cache stdout ----\nassertion failed\n\n\nfailures:\n    cache::record_is_kept\n    cache::tmp_is_cache\n\ntest result: FAILED. 1 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s";
    let (_outer, repo) = repository("quiet", output, 101);
    let effect = effect_line(&repo);
    assert!(
        effect.contains("failed: cache::record_is_kept, cache::tmp_is_cache"),
        "{effect}"
    );
}

/// A summary longer than 300 characters is cut, and the cut is marked.
#[test]
fn a_long_summary_is_cut_and_the_cut_is_marked() {
    let mut output = String::from("\nrunning 40 tests\n");
    for n in 0..40 {
        output.push_str(&format!(
            "test module::a_rather_long_test_name_{n:02} ... FAILED\n"
        ));
    }
    output.push_str("\ntest result: FAILED. 0 passed; 40 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s");
    let (_outer, repo) = repository("long", &output, 101);
    let effect = effect_line(&repo);
    let summary = effect
        .split_once("; ")
        .map(|(_, rest)| rest)
        .unwrap_or(&effect);
    assert!(summary.ends_with('…'), "the cut is marked: {effect}");
    assert!(
        summary.chars().count() <= 300,
        "{} characters: {effect}",
        summary.chars().count()
    );
    assert!(
        effect.contains("0 passed, 40 failed, 0 ignored over 1 result line"),
        "{effect}"
    );
}
