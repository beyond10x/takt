//! story:report-sums-cases: `takt report --cases` sums, per repository and UTC day, the units
//! accepted, the tokens the harness reported and the interventions by a person, over the case
//! directories under each cases directory given, and a total row; a figure no record states is
//! `unknown`, never 0.
//!
//! The fixtures (`tests/fixtures/report-cases`), counted by hand:
//!
//! - repository `a`, under `a/.engineering/cases`:
//!   - `a-one`: one run opened 2026-10-01, a harness step of 1000 tokens and validator steps
//!     with none, stopped `Completed (accepted)`; one grant by `agent:takt-delivery`, which is
//!     no intervention.
//!   - `a-two`: a run opened 2026-10-01, a harness step of 2000, stopped `ApprovalRequired`; a
//!     grant by `human:operator` on 2026-10-02; a run opened 2026-10-02, a harness step of 500,
//!     stopped `Completed (accepted)`.
//!   - so 2026-10-01: 1 accepted, 3000 tokens, 0 interventions; 2026-10-02: 1 accepted, 500
//!     tokens, 1 intervention.
//! - repository `b`, under `b/.takt/cases`: `b-budget`, one run opened 2026-10-02, a harness
//!   step of 4321 tokens and one with no `tokens`, stopped on its hour budget with the stop
//!   line's `tokens` 4321: 0 accepted, tokens unknown, 0 interventions.
//! - total: 2 accepted, tokens unknown, 1 intervention.

use std::fs;
use std::path::{Path, PathBuf};

use takt::report::{CaseSums, DayFigures, case_sums, print_case_sums};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/report-cases")
}

fn sums_of_both() -> CaseSums {
    let root = fixtures();
    case_sums(&[
        root.join("a/.engineering/cases"),
        root.join("b/.takt/cases"),
    ])
    .expect("the fixtures read")
}

fn figures(accepted: u64, tokens: Option<u64>, interventions: u64) -> DayFigures {
    DayFigures {
        accepted,
        tokens,
        interventions,
    }
}

#[test]
fn one_row_per_repository_and_day_equal_to_the_hand_count() {
    let root = fixtures();
    let sums = sums_of_both();
    let rows: Vec<(String, String, DayFigures)> = sums
        .rows
        .iter()
        .map(|row| (row.repository.clone(), row.day.clone(), row.figures))
        .collect();
    let a = root.join("a").display().to_string();
    let b = root.join("b").display().to_string();
    assert_eq!(
        rows,
        vec![
            (
                a.clone(),
                "2026-10-01".to_owned(),
                figures(1, Some(3000), 0)
            ),
            (a, "2026-10-02".to_owned(), figures(1, Some(500), 1)),
            (b, "2026-10-02".to_owned(), figures(0, None, 0)),
        ]
    );
    assert_eq!(sums.total, figures(2, None, 1));
}

#[test]
fn the_printed_table_says_unknown_for_b_and_the_total_never_the_stop_lines_number() {
    let root = fixtures();
    let mut out = Vec::new();
    print_case_sums(&sums_of_both(), &mut out).expect("printed");
    let text = String::from_utf8(out).expect("utf-8");
    let a = takt::trace::escape(&root.join("a").display().to_string());
    let b = takt::trace::escape(&root.join("b").display().to_string());
    assert!(
        text.contains(&format!("| {a} | 2026-10-01 | 1 | 3000 | 0 |")),
        "{text}"
    );
    assert!(
        text.contains(&format!("| {a} | 2026-10-02 | 1 | 500 | 1 |")),
        "{text}"
    );
    assert!(
        text.contains(&format!("| {b} | 2026-10-02 | 0 | unknown | 0 |")),
        "{text}"
    );
    assert!(text.contains("| total | | 2 | unknown | 1 |"), "{text}");
    assert!(
        !text.contains("4321"),
        "the stop line's tokens are never read: {text}"
    );
}

#[test]
fn a_grant_by_an_agent_role_is_no_intervention() {
    let root = fixtures();
    let sums = case_sums(&[root.join("a/.engineering/cases")]).expect("read");
    let one_day: Vec<u64> = sums
        .rows
        .iter()
        .filter(|row| row.day == "2026-10-01")
        .map(|row| row.figures.interventions)
        .collect();
    assert_eq!(one_day, vec![0], "a-one's grant by agent:takt-delivery");
    assert_eq!(sums.total, figures(2, Some(3500), 1));
}

/// A scratch directory under the target directory, removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("report-cases-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("scratch");
        Self(dir)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn a_scenario_cases_held_out_result_does_not_change_accepted() {
    let scratch = Scratch::new("held-out");
    let case = scratch.0.join("r/.takt/cases/scenario-s-trial-1");
    fs::create_dir_all(&case).expect("case");
    fs::write(
        case.join("events.jsonl"),
        concat!(
            r#"{"at":"2026-10-03T00:00:00Z","step":0,"event":"opened","case":"scenario-s-trial-1","protocol":"story.delivery/1"}"#,
            "\n",
            r#"{"at":"2026-10-03T00:01:00Z","step":1,"event":"step","by":"harness","name":"claude-code","action":"implementation.edit","tokens":10}"#,
            "\n",
            r#"{"at":"2026-10-03T00:01:01Z","step":1,"event":"stop","stop":"Completed (accepted)","steps":1,"tokens":10,"completed":true}"#,
            "\n",
        ),
    )
    .expect("record");
    fs::write(case.join("held-out.txt"), "failed\n").expect("held-out");
    let sums = case_sums(&[scratch.0.join("r/.takt/cases")]).expect("read");
    assert_eq!(sums.total, figures(1, Some(10), 0));
    assert_eq!(
        sums.rows[0].repository,
        scratch.0.join("r").display().to_string()
    );
}

#[test]
fn the_last_stop_decides_and_counts_on_its_runs_day() {
    let scratch = Scratch::new("last-stop");
    let case = scratch.0.join("cases-elsewhere/c");
    fs::create_dir_all(&case).expect("case");
    fs::write(
        case.join("events.jsonl"),
        concat!(
            r#"{"at":"2026-10-04T23:00:00Z","step":0,"event":"opened","case":"c","protocol":"story.delivery/1"}"#,
            "\n",
            r#"{"at":"2026-10-04T23:00:01Z","step":0,"event":"stop","stop":"Completed (accepted)","steps":0,"tokens":0,"completed":true}"#,
            "\n",
            r#"{"at":"2026-10-05T01:00:00Z","step":0,"event":"opened","case":"c","protocol":"story.delivery/1"}"#,
            "\n",
            r#"{"at":"2026-10-05T01:00:01Z","step":0,"event":"stop","stop":"NoProgress","steps":0,"tokens":0,"completed":false}"#,
            "\n",
            r#"{"at":"2026-10-06T01:00:00Z","step":0,"event":"opened","case":"c","protocol":"story.delivery/1"}"#,
            "\n",
        ),
    )
    .expect("record");
    fs::write(
        case.join("authority.yaml"),
        "- capability: repository.merge\n  decision: denied\n  approver: human:operator\n  granted_at: 2026-10-05T00:30:00Z\n",
    )
    .expect("authority");
    let given = scratch.0.join("cases-elsewhere");
    let sums = case_sums(std::slice::from_ref(&given)).expect("read");
    let rows: Vec<(String, String, DayFigures)> = sums
        .rows
        .iter()
        .map(|row| (row.repository.clone(), row.day.clone(), row.figures))
        .collect();
    let repository = given.display().to_string();
    assert_eq!(
        rows,
        vec![
            (
                repository.clone(),
                "2026-10-04".to_owned(),
                figures(0, Some(0), 0)
            ),
            (
                repository.clone(),
                "2026-10-05".to_owned(),
                figures(0, Some(0), 1)
            ),
            (repository, "2026-10-06".to_owned(), figures(0, Some(0), 0)),
        ],
        "the last stop line is NoProgress, so no unit is accepted; the denial by a person is an intervention on its own day; a cases directory under no known root names itself"
    );
}

#[test]
fn a_cases_directory_that_is_not_one_is_refused_naming_it() {
    let missing = fixtures().join("no-such/.takt/cases");
    let error = case_sums(std::slice::from_ref(&missing)).expect_err("refused");
    assert!(
        format!("{error:#}").contains(&missing.display().to_string()),
        "{error:#}"
    );
}
