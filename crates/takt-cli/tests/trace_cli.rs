//! story:run-trace-and-report through the binary: `takt trace --case <dir>` writes one
//! `takt-trace/1` file, `takt report` prints the attribution across the cases or traces given.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The run records the library crate keeps as fixtures.
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../takt/tests/fixtures/trace");

struct Scratch(PathBuf);

impl std::ops::Deref for Scratch {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.0
    }
}

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

fn scratch(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("trace-cli-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(&dir).expect("a scratch tree");
    Scratch(dir)
}

fn takt(args: &[&str]) -> (i32, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args(args)
        .output()
        .expect("takt runs");
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

fn fixture(name: &str) -> String {
    Path::new(FIXTURES).join(name).display().to_string()
}

/// `takt trace --case <dir> --steps <file>` prints the header line, then one line per
/// operation: trial 1's six steps and its stop.
#[test]
fn trace_prints_a_header_then_one_line_per_operation() {
    let case = fixture("trial-1");
    let steps = fixture("trial-1/steps.jsonl");
    let (code, stdout, stderr) = takt(&[
        "trace", "--case", &case, "--steps", &steps, "--run", "trial-1",
    ]);
    assert_eq!(code, 0, "{stderr}");
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 8, "{stdout}");
    assert!(
        lines[0].starts_with("{\"type\":\"header\","),
        "{}",
        lines[0]
    );
    for line in &lines[1..] {
        assert!(line.starts_with("{\"type\":\"op\","), "{line}");
        assert!(!line.contains("/home/"), "{line}");
    }
    let dir = scratch("printed");
    let file = dir.join("trial-1.jsonl");
    fs::write(&file, &stdout).expect("the printed trace is kept");
    let trace = takt::trace::Trace::load(&file).expect("the printed trace reads back");
    assert_eq!(trace.header.format, "takt-trace/1");
    assert_eq!(trace.header.run, "trial-1");
    assert_eq!(trace.header.case, "takt-20261018T205520Z");
    assert_eq!(trace.header.tokens.sum, Some(592_698));
    assert_eq!(trace.header.model.as_deref(), Some("claude-opus-5-5"));
    assert_eq!(trace.ops.len(), 7);
    let second = &trace.ops[1];
    assert_eq!(second.actor, takt::trace::Actor::Harness);
    assert_eq!(second.kind, takt::trace::Kind::Edit);
    assert_eq!(second.duration_ms, Some(41_885));
    assert_eq!(
        second.paths,
        vec!["crates/worktree-cli/tests/cache_discard.rs".to_owned()]
    );
}

/// `--out` writes the file, which `takt report --trace` reads back; `takt report --case`
/// over two cases prints one row each and a section each.
#[test]
fn report_reads_cases_and_traces() {
    let dir = scratch("report");
    let out = dir.join("t-24.jsonl");
    let (code, stdout, stderr) = takt(&[
        "trace",
        "--case",
        &fixture("t-24"),
        "--steps",
        &fixture("t-24/steps.jsonl"),
        "--out",
        &out.display().to_string(),
    ]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stdout.starts_with("written: "), "{stdout}");
    assert!(stdout.contains("21 operations"), "{stdout}");
    assert!(out.is_file());

    let (code, stdout, stderr) = takt(&[
        "report",
        "--case",
        &fixture("trial-1"),
        "--steps",
        &fixture("trial-1/steps.jsonl"),
        "--trace",
        &out.display().to_string(),
    ]);
    assert_eq!(code, 0, "{stderr}");
    assert!(
        stdout.starts_with("| run | protocol | story | outcome | wall s |"),
        "{stdout}"
    );
    assert!(
        stdout.contains("| takt-20261018T205520Z | software.change/1 | - | ApprovalRequired | 242 | 6 | 3 | 33 | 592698 | 0.849 | 107 | 0 | 0 | 0 | 130 | 128 | 5 |"),
        "{stdout}"
    );
    assert!(
        stdout.contains("| scenario-1-trial-46 | story.delivery/1 | story:remove-a-todo | ApprovalRequired | 123 | 18 | 3 | 14 | 122095 | 0.247 |"),
        "{stdout}"
    );
    assert!(stdout.contains("### takt-20261018T205520Z"), "{stdout}");
    assert!(stdout.contains("### scenario-1-trial-46"), "{stdout}");
    assert!(stdout.contains("- step 17: 2nd CI"), "{stdout}");
    assert!(
        stdout.contains("| 9 | idle | waiting on ci.read | 30.0 |"),
        "{stdout}"
    );
}

/// A case without a record is refused, exit 1; `--steps` more often than `--case` is
/// refused; no input at all is a usage error, exit 2.
#[test]
fn refusals_name_their_reason() {
    let dir = scratch("empty");
    let (code, _, stderr) = takt(&["trace", "--case", &dir.display().to_string()]);
    assert_eq!(code, 1, "{stderr}");
    assert!(stderr.contains("events.jsonl"), "{stderr}");

    let (code, _, stderr) = takt(&[
        "report",
        "--case",
        &fixture("trial-1"),
        "--steps",
        &fixture("trial-1/steps.jsonl"),
        "--steps",
        &fixture("t-24/steps.jsonl"),
    ]);
    assert_eq!(code, 1, "{stderr}");
    assert!(stderr.contains("--steps"), "{stderr}");

    let (code, _, stderr) = takt(&["report"]);
    assert_eq!(code, 2, "{stderr}");
}

/// The trace names its sources relative to the case directory whatever `HOME` is: with
/// `HOME` pointing nowhere and the case given by an absolute path, the header's `sources`
/// are `events.jsonl` and `steps.jsonl`, and no line holds the case's path.
#[test]
fn sources_are_relative_whatever_home_is() {
    let case = fixture("trial-1");
    let steps = fixture("trial-1/steps.jsonl");
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args(["trace", "--case", &case, "--steps", &steps])
        .env("HOME", "/nonexistent")
        .output()
        .expect("takt runs");
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert_eq!(output.status.code(), Some(0), "{stderr}");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let absolute = Path::new(FIXTURES)
        .canonicalize()
        .expect("the fixtures")
        .display()
        .to_string();
    for line in stdout.lines() {
        assert!(!line.contains("/home/"), "{line}");
        assert!(!line.contains(&absolute), "{line}");
        assert!(!line.contains(FIXTURES), "{line}");
    }
    let dir = scratch("home");
    let file = dir.join("trial-1.jsonl");
    fs::write(&file, &stdout).expect("the printed trace is kept");
    let trace = takt::trace::Trace::load(&file).expect("the printed trace reads back");
    assert_eq!(
        trace.header.sources,
        vec!["events.jsonl".to_owned(), "steps.jsonl".to_owned()]
    );
}

/// story:the-report-attributes-cost-and-failures-to-layers (ADR 0101) through the binary: the
/// report's first table ends in a layers column, `not recorded` for a record without
/// `layers`, and each run's section holds the cost and the failures by layer, each figure
/// naming the record line it came from.
#[test]
fn report_prints_the_layers_column_and_the_cost_by_layer() {
    let (code, stdout, stderr) = takt(&[
        "report",
        "--case",
        &fixture("trial-1"),
        "--steps",
        &fixture("trial-1/steps.jsonl"),
        "--case",
        &fixture("layered-1"),
    ]);
    assert_eq!(code, 0, "{stderr}");
    assert!(
        stdout.starts_with("| run | protocol | story | outcome | wall s |"),
        "{stdout}"
    );
    assert!(
        stdout
            .lines()
            .next()
            .is_some_and(|line| line.ends_with("| layers |")),
        "{stdout}"
    );
    assert!(
        stdout.contains("| takt-20261018T205520Z | software.change/1 | - | ApprovalRequired | 242 | 6 | 3 | 33 | 592698 | 0.849 | 107 | 0 | 0 | 0 | 130 | 128 | 5 | 0 | - | not recorded |"),
        "{stdout}"
    );
    assert!(
        stdout.contains("| takt 0123456789abcdef0123456789abcdef01234567; harness 2.1.304; model claude-opus-5-5 |"),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "| model | not recorded | 592698 | 0.849 | 128 | not reported | events.jsonl:6,12,18 |"
        ),
        "{stdout}"
    );
    assert!(
        stdout.contains("- takt: step 1: refusal by takt (events.jsonl:5)"),
        "{stdout}"
    );
    assert!(stdout.contains("## total by layer"), "{stdout}");
}
