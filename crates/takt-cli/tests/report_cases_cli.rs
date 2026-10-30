//! story:report-sums-cases through the `takt` binary: `takt report --cases <dir>...` over the
//! two fixture repositories (`crates/takt/tests/fixtures/report-cases`, counted by hand in
//! `crates/takt/tests/report_cases.rs`) exits 0 and prints a row per repository and day and a
//! total row; `--format json` beside `--cases` is refused as an argument (exit 2), and `--cases`
//! beside `--case` or `--trace` likewise.

#[path = "../../takt/tests/support/own_home.rs"]
mod own_home;

use std::path::{Path, PathBuf};
use std::process::Command;

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../takt/tests/fixtures/report-cases")
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

/// The table's rows: the cells of each line that opens with `|`, past the header and its rule.
fn rows(text: &str) -> Vec<Vec<String>> {
    text.lines()
        .filter(|line| line.starts_with('|'))
        .skip(2)
        .map(|line| {
            line.trim_matches('|')
                .split('|')
                .map(|cell| cell.trim().to_owned())
                .collect()
        })
        .collect()
}

#[test]
fn report_cases_prints_a_row_per_repository_and_day_and_their_total() {
    let root = fixtures();
    let a = root.join("a/.engineering/cases");
    let b = root.join("b/.takt/cases");
    let (code, stdout, stderr) = takt(&[
        "report",
        "--cases",
        a.to_str().expect("utf-8"),
        "--cases",
        b.to_str().expect("utf-8"),
    ]);
    assert_eq!(code, 0, "{stderr}");
    let rows = rows(&stdout);
    let figures: Vec<Vec<&str>> = rows
        .iter()
        .map(|row| row[1..].iter().map(String::as_str).collect())
        .collect();
    assert_eq!(
        figures,
        vec![
            vec!["2026-10-01", "1", "3000", "0"],
            vec!["2026-10-02", "1", "500", "1"],
            vec!["2026-10-02", "0", "unknown", "0"],
            vec!["", "2", "unknown", "1"],
        ],
        "{stdout}"
    );
    assert!(rows[0][0].ends_with("report-cases/a"), "{stdout}");
    assert!(rows[1][0].ends_with("report-cases/a"), "{stdout}");
    assert!(rows[2][0].ends_with("report-cases/b"), "{stdout}");
    assert_eq!(rows[3][0], "total", "{stdout}");
    assert!(
        !stdout.contains("4321"),
        "b's stop line's tokens are never printed: {stdout}"
    );
}

#[test]
fn report_cases_takes_several_directories_after_one_flag() {
    let root = fixtures();
    let a = root.join("a/.engineering/cases");
    let b = root.join("b/.takt/cases");
    let (code, stdout, stderr) = takt(&[
        "report",
        "--cases",
        a.to_str().expect("utf-8"),
        b.to_str().expect("utf-8"),
    ]);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(rows(&stdout).len(), 4, "{stdout}");
}

#[test]
fn report_cases_refuses_format_json_as_an_argument() {
    let a = fixtures().join("a/.engineering/cases");
    let (code, stdout, stderr) = takt(&[
        "report",
        "--cases",
        a.to_str().expect("utf-8"),
        "--format",
        "json",
    ]);
    assert_eq!(code, 2, "{stdout}{stderr}");
    assert!(stderr.contains("--format"), "{stderr}");
    assert!(
        stderr
            .lines()
            .any(|line| line.starts_with("Usage: takt report ")),
        "the usage is `takt report`'s, not the top level's: {stderr}"
    );
    assert!(stdout.is_empty(), "{stdout}");
    let (code, _, stderr) = takt(&[
        "report",
        "--cases",
        a.to_str().expect("utf-8"),
        "--format",
        "text",
    ]);
    assert_eq!(code, 0, "{stderr}");
}

#[test]
fn report_cases_beside_a_case_or_a_trace_is_refused_as_an_argument() {
    let a = fixtures().join("a/.engineering/cases");
    let one = a.join("a-one");
    for other in ["--case", "--trace"] {
        let (code, _, stderr) = takt(&[
            "report",
            "--cases",
            a.to_str().expect("utf-8"),
            other,
            one.to_str().expect("utf-8"),
        ]);
        assert_eq!(code, 2, "{other}: {stderr}");
        assert!(stderr.contains("cannot be used with"), "{other}: {stderr}");
    }
}

#[test]
fn report_cases_on_a_directory_that_is_not_one_is_refused() {
    let missing = fixtures().join("no-such/.takt/cases");
    let (code, _, stderr) = takt(&["report", "--cases", missing.to_str().expect("utf-8")]);
    assert_eq!(code, 1, "{stderr}");
    assert!(stderr.contains("no-such"), "{stderr}");
}

#[test]
fn report_cases_on_a_directory_holding_no_case_is_refused() {
    let repository = fixtures().join("a");
    let (code, stdout, stderr) = takt(&["report", "--cases", repository.to_str().expect("utf-8")]);
    assert_eq!(code, 1, "{stdout}{stderr}");
    assert!(stdout.is_empty(), "{stdout}");
    assert!(
        stderr.contains(&repository.display().to_string()),
        "{stderr}"
    );
}
