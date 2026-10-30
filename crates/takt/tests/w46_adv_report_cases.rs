//! Adversarial pass, wave 46, story:report-sums-cases: `takt::report::case_sums` over built
//! cases directories. Each test names the expectation it holds the code to.

use std::fs;
use std::path::{Path, PathBuf};

use takt::report::{CaseSums, DayFigures, case_sums, print_case_sums};

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("w46-adv-{name}"));
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

fn case(root: &Path, id: &str, events: &str, authority: Option<&str>) {
    let dir = root.join(id);
    fs::create_dir_all(&dir).expect("case dir");
    fs::write(dir.join("events.jsonl"), events).expect("events");
    if let Some(authority) = authority {
        fs::write(dir.join("authority.yaml"), authority).expect("authority");
    }
}

const ACCEPTED: &str = r#"{"at":"2026-10-01T09:00:00Z","step":0,"event":"opened","case":"c"}
{"at":"2026-10-01T09:01:00Z","step":1,"event":"step","by":"harness","action":"implementation.edit","tokens":100}
{"at":"2026-10-01T09:02:00Z","step":1,"event":"stop","stop":"Completed (accepted)","steps":1,"tokens":100,"completed":true}
"#;

fn rows(sums: &CaseSums) -> Vec<(String, String, DayFigures)> {
    sums.rows
        .iter()
        .map(|row| (row.repository.clone(), row.day.clone(), row.figures))
        .collect()
}

/// The same cases directory given twice: one accepted unit, counted once.
#[test]
fn the_same_cases_directory_given_twice_counts_its_cases_once() {
    let scratch = Scratch::new("twice");
    let root = scratch.0.join("r/.takt/cases");
    case(&root, "c", ACCEPTED, None);
    let once = case_sums(std::slice::from_ref(&root)).expect("sums");
    let twice = case_sums(&[root.clone(), root.clone()]).expect("sums");
    assert_eq!(once.total.accepted, 1);
    assert_eq!(
        twice.total,
        once.total,
        "the same directory twice doubles the figures: {:?}",
        rows(&twice)
    );
}

/// One repository spelled two ways (`r` and `r/.`): one repository, one row per day.
#[test]
fn one_repository_spelled_two_ways_is_one_repository() {
    let scratch = Scratch::new("spelled");
    let root = scratch.0.join("r/.takt/cases");
    case(&root, "c", ACCEPTED, None);
    let other = scratch.0.join("r/./.takt/cases");
    let sums = case_sums(&[root, other]).expect("sums");
    let repositories: std::collections::BTreeSet<&str> = sums
        .rows
        .iter()
        .map(|row| row.repository.as_str())
        .collect();
    assert_eq!(
        repositories.len(),
        1,
        "one repository shown as several: {:?}",
        rows(&sums)
    );
}

/// A directory that holds no case (a repository root given in place of its cases root, or a
/// single case directory): not a quiet day of zeros.
#[test]
fn a_directory_that_holds_no_case_is_not_a_report_of_zeros() {
    let scratch = Scratch::new("nocase");
    let repo = scratch.0.join("r");
    let root = repo.join(".takt/cases");
    case(&root, "c", ACCEPTED, None);
    fs::create_dir_all(repo.join("src")).expect("src");
    // The repository root, not its cases root.
    let result = case_sums(std::slice::from_ref(&repo));
    let mut out = Vec::new();
    if let Ok(sums) = &result {
        print_case_sums(sums, &mut out).expect("print");
    }
    assert!(
        result.is_err(),
        "a directory holding no case reads as zero units, zero tokens:\n{}",
        String::from_utf8_lossy(&out)
    );
}

/// A run opened at 23:59:59 UTC and stopped accepted the next day counts on the day it opened
/// (the story's rule), and an empty-named `human:` approver is no person.
#[test]
fn midnight_and_the_empty_human() {
    let scratch = Scratch::new("midnight");
    let root = scratch.0.join("r/.takt/cases");
    case(
        &root,
        "c",
        r#"{"at":"2026-10-01T23:59:59.999999Z","step":0,"event":"opened","case":"c"}
{"at":"2026-10-02T00:00:01Z","step":1,"event":"step","by":"harness","action":"implementation.edit","tokens":7}
{"at":"2026-10-02T00:00:02Z","step":1,"event":"stop","stop":"Completed (accepted)","steps":1,"tokens":7,"completed":true}
"#,
        Some(
            "- capability: repository.merge\n  decision: granted\n  approver: 'human:'\n  granted_at: 2026-10-02T00:00:00Z\n  revision: 1111111111111111111111111111111111111111\n",
        ),
    );
    let sums = case_sums(&[root]).expect("sums");
    assert_eq!(sums.rows[0].day, "2026-10-01", "{:?}", rows(&sums));
    assert_eq!(sums.rows[0].figures.accepted, 1);
    assert_eq!(sums.rows[0].figures.tokens, Some(7));
    assert_eq!(
        sums.total.interventions,
        0,
        "`human:` with no name, which takt approve refuses (case::is_approver), counted: {:?}",
        rows(&sums)
    );
}

/// An unknown token figure on one day leaves another day of the same repository a number,
/// and turns the total unknown.
#[test]
fn unknown_tokens_stay_on_their_day() {
    let scratch = Scratch::new("unknown");
    let root = scratch.0.join("r/.takt/cases");
    case(
        &root,
        "c",
        r#"{"at":"2026-10-01T09:00:00Z","step":0,"event":"opened","case":"c"}
{"at":"2026-10-01T09:01:00Z","step":1,"event":"step","by":"harness","action":"tests.write","tokens":5}
{"at":"2026-10-01T09:02:00Z","step":1,"event":"stop","stop":"Iterations (1)","steps":1,"tokens":5,"completed":false}
{"at":"2026-10-02T09:00:00Z","step":1,"event":"opened","case":"c"}
{"at":"2026-10-02T09:01:00Z","step":2,"event":"step","by":"harness","action":"implementation.edit"}
{"at":"2026-10-02T09:02:00Z","step":2,"event":"stop","stop":"Completed (accepted)","steps":2,"tokens":0,"completed":true}
"#,
        None,
    );
    let sums = case_sums(&[root]).expect("sums");
    let got = rows(&sums);
    assert_eq!(got.len(), 2, "{got:?}");
    assert_eq!(got[0].2.tokens, Some(5), "{got:?}");
    assert_eq!(got[1].2.tokens, None, "{got:?}");
    assert_eq!(got[1].2.accepted, 1, "{got:?}");
    assert_eq!(sums.total.tokens, None);
}

/// A line in the middle that does not read, and was not cut short: refused, naming the file.
#[test]
fn a_malformed_middle_line_is_refused_naming_the_file() {
    let scratch = Scratch::new("malformed");
    let root = scratch.0.join("r/.takt/cases");
    case(
        &root,
        "c",
        "{\"at\":\"2026-10-01T09:00:00Z\",\"step\":0,\"event\":\"opened\"}\nnot json\n{\"at\":\"2026-10-01T09:00:01Z\",\"step\":0,\"event\":\"stop\",\"stop\":\"NoProgress\",\"steps\":0,\"tokens\":0}\n",
        None,
    );
    let error = case_sums(&[root]).expect_err("refused");
    assert!(format!("{error:#}").contains("events.jsonl"), "{error:#}");
}

/// A case directory that is a symbolic link to a case: the run record is never read through a
/// link (`Events::open`, O_NOFOLLOW), so the report must not read it silently as nothing
/// either; it is refused or counted, never dropped.
#[test]
fn a_linked_case_directory_is_not_dropped_silently() {
    let scratch = Scratch::new("linked");
    let root = scratch.0.join("r/.takt/cases");
    let elsewhere = scratch.0.join("elsewhere");
    case(&elsewhere, "c", ACCEPTED, None);
    fs::create_dir_all(&root).expect("root");
    std::os::unix::fs::symlink(elsewhere.join("c"), root.join("c")).expect("link");
    let result = case_sums(&[root]);
    match result {
        Err(_) => {}
        Ok(sums) => assert_eq!(
            sums.total.accepted,
            1,
            "a linked case dropped without a word: {:?}",
            rows(&sums)
        ),
    }
}

/// A case directory with no `events.jsonl` (its record on the case branch only, as the
/// committed cases of a repository are): no record states its tokens, so they are not 0.
#[test]
fn a_case_without_a_run_record_states_no_tokens() {
    let scratch = Scratch::new("norecord");
    let root = scratch.0.join("r/.takt/cases");
    let dir = root.join("c");
    fs::create_dir_all(&dir).expect("case");
    fs::write(dir.join("case.yaml"), "id: c\n").expect("case.yaml");
    let sums = case_sums(&[root]).expect("sums");
    let mut out = Vec::new();
    print_case_sums(&sums, &mut out).expect("print");
    assert_ne!(
        sums.total.tokens,
        Some(0),
        "a case with no events.jsonl prints tokens 0:\n{}",
        String::from_utf8_lossy(&out)
    );
}
