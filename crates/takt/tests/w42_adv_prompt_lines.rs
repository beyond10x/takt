//! Adversary, wave 42: what a red test run and a failed gate put into a judgement step's prompt.
//!
//! - U7/U10: the block naming each failing test's assertion ([`tests_path_block`]) is appended
//!   to the prompt after the scan ran on the block before it ([`red_tests_block`]); it rewrites
//!   the kept line as `- <test>: <location>`, a shape the scanned text did not hold.
//! - U4: a failing line located at a host, not a file, is never kept
//!   (story:failed-gate-keeps-other-linter-shapes, Outcome).
//! - U3: every test binary that runs steps under the guard reads program directories of its own,
//!   unit-test binaries of `crates/*/src` among them.

#[path = "support/own_home.rs"]
mod own_home;

use std::path::{Path, PathBuf};

use takt::harness::{red_tests_block, tests_path_block};
use takt::run::credential_shape;
use takt::validator::{FAILED_LINES_PER_CHECK, FailedCheck, TestsRun, failing_lines};

/// A libtest run whose one failing test is named `tests::refresh_token` and panics in
/// `crates/auth/src/oauth2.rs`: an ordinary test name and an ordinary source path.
const LIBTEST: &str = "running 1 test\n\
    test tests::refresh_token ... FAILED\n\n\
    failures:\n\n\
    ---- tests::refresh_token stdout ----\n\n\
    thread 'tests::refresh_token' panicked at crates/auth/src/oauth2.rs:12:9:\n\
    assertion failed: renewed\n\n\
    failures:\n    tests::refresh_token\n\n\
    test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out\n";

/// The text the prompt shows after the scanned block is text the scan saw: the block of
/// assertion locations holds no credential's shape that the scanned block did not hold. Here
/// the scanned block holds none, and the appended one joins the test's name and its location
/// as `refresh_token: crates/auth/src/oauth2.rs`, which `credential_shape` reads as a token
/// field. `ClaudeCode::prompt_with` appends it unscanned (`crates/takt/src/harness.rs`, the
/// `red == red_tests_block(&red_tests)` branch, and the same after the gates' block).
#[test]
fn the_assertion_block_shows_no_shape_the_scanned_block_did_not() {
    let (lines, omitted) = failing_lines(LIBTEST, FAILED_LINES_PER_CHECK);
    let failed = vec![FailedCheck {
        id: TestsRun::ACTION.to_owned(),
        lines,
        omitted,
    }];
    let offered = vec!["implementation.edit".to_owned()];
    let scanned = red_tests_block(&failed);
    assert!(
        scanned.contains("tests::refresh_token"),
        "the red block: {scanned}"
    );
    assert_eq!(
        credential_shape(&scanned),
        None,
        "the scanned block: {scanned}"
    );
    let appended = tests_path_block(&failed, &offered);
    assert!(
        appended.contains("crates/auth/src/oauth2.rs:12:9"),
        "the assertion block: {appended}"
    );
    assert_eq!(
        credential_shape(&appended),
        None,
        "the unscanned assertion block holds a credential's shape the scanned block did not: {appended}"
    );
}

/// A pytest summary, so the gate's output is read as a failed test run's.
const PYTEST: &str = "============ short test summary info ============\n\
    FAILED tests/t.py::test_x - assert 1 == 2\n";

/// The hosts of `lines` that `failing_lines` keeps beside a failed test run.
fn kept_hosts(lines: &[(&str, &str)]) -> Vec<String> {
    let mut kept_hosts = Vec::new();
    for (line, host) in lines {
        let (kept, _) = failing_lines(&format!("{line}\n{PYTEST}"), 4096);
        if kept.iter().any(|kept| kept.contains(host)) {
            kept_hosts.push((*line).to_owned());
        }
    }
    kept_hosts
}

/// U4's new shapes keep a host: `fatal error:` at a host under a domain not in
/// `HOST_DOMAINS`, and a capitalised host name read as an extensionless file like `Dockerfile`.
/// Before U4 neither line was kept.
#[test]
fn a_line_at_a_host_is_not_kept_in_the_shapes_u4_added() {
    let kept = kept_hosts(&[
        (
            "pg.example.de:5432: fatal error: password authentication failed for user deploy",
            "pg.example.de",
        ),
        ("Redis:6379: error: connection refused", "Redis:6379"),
    ]);
    assert!(
        kept.is_empty(),
        "lines at a host kept as failing lines: {kept:?}"
    );
}

/// The Outcome of story:failed-gate-keeps-other-linter-shapes: a line that names a host, not a
/// file, is not kept. A host under a country domain, or with a user before it, still is.
#[test]
fn a_line_at_a_host_is_not_kept() {
    let kept = kept_hosts(&[
        (
            "db.corp.example.de:5432: error: connection refused",
            "db.corp.example.de",
        ),
        (
            "deploy@db.example.com:5432: error: connection refused",
            "db.example.com",
        ),
    ]);
    assert!(
        kept.is_empty(),
        "lines at a host kept as failing lines: {kept:?}"
    );
}

/// What a unit-test module starts that runs steps under the step guard: a run opened in this
/// process, or the CLI's `run` driven in process.
const RUNS_STEPS: &[&str] = &[
    "Run::open",
    "Run::resume",
    "Run::take_next",
    "Run::dispatch",
    "Cli::parse_from",
];

fn rust_files(dir: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, found);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            found.push(path);
        }
    }
}

/// U3's commit: "every test binary that runs steps reads program directories of its own".
/// `tests_have_a_home_of_their_own` reads `crates/*/tests` only; a `#[cfg(test)]` module in
/// `crates/*/src` is a test binary too, and the `own_home` hook cannot be in it. The takt
/// binary's unit tests run `takt run --harness none --test-cmd true` in process
/// (`crates/takt-cli/src/main.rs`, `fn takt`), a `tests.run` step under the guard over the
/// operator's `PATH` and `~/.cargo/bin`.
#[test]
#[ignore = "story:unit-tests-run-steps-over-the-operators-path: unit-test modules in crates/*/src run steps over the operator's program directories"]
fn no_unit_test_module_runs_steps_over_the_operators_program_directories() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the crates directory");
    let mut files = Vec::new();
    for name in ["takt", "takt-cli"] {
        rust_files(&crates.join(name).join("src"), &mut files);
    }
    files.sort();
    assert!(!files.is_empty(), "no source file found");
    let mut exposed = Vec::new();
    for file in files {
        let text = std::fs::read_to_string(&file).expect("a source file");
        let Some(at) = text.find("#[cfg(test)]\nmod tests") else {
            continue;
        };
        let tests = &text[at..];
        if RUNS_STEPS.iter().any(|needle| tests.contains(needle))
            && tests.contains("\"--test-cmd\"")
        {
            exposed.push(
                file.strip_prefix(crates)
                    .unwrap_or(&file)
                    .display()
                    .to_string(),
            );
        }
    }
    assert!(
        exposed.is_empty(),
        "unit-test binaries that run steps over the operator's program directories: {exposed:?}"
    );
}
