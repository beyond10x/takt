//! What a failing `tests.run` summary names, as the run reads it back for `RedBase` and
//! `RepeatedTestFailure` (story:a-red-base-stops-at-open,
//! story:repeated-identical-test-failures-stop-the-run): the failed tests after the last
//! `; failed: `, none when that list was cut short, and the rerun hint; and the two stops' lines.

use takt::Stop;
use takt::validator::{Failures, failures_in};

/// A cargo failure: the counts, the rerun hint, then the tests in output order.
#[test]
fn a_cargo_failure_names_its_tests_and_its_rerun_hint() {
    let summary = "the test command exited 101: fail; 1 passed, 2 failed, 0 ignored over 1 result line; to rerun pass `-p b10x-takt --lib`; failed: a::two, a::three";
    assert_eq!(
        failures_in(summary),
        Failures {
            tests: vec!["a::two".to_owned(), "a::three".to_owned()],
            rerun: Some("to rerun pass `-p b10x-takt --lib`".to_owned()),
        }
    );
}

/// A list cut to its bound names no set: a part of one is never compared.
#[test]
fn a_cut_list_names_no_test() {
    let summary = "the test command exited 101: fail; 0 passed, 90 failed, 0 ignored over 1 result line; failed: a::one, a::tw…";
    assert!(failures_in(summary).tests.is_empty());
}

/// A pass, or a failure whose output named no test, names none.
#[test]
fn a_summary_without_the_list_names_no_test() {
    assert_eq!(
        failures_in("the test command exited 1: fail; the runner crashed"),
        Failures::default()
    );
    assert_eq!(
        failures_in(
            "the test command exited 0: pass; 3 passed, 0 failed, 0 ignored over 1 result line"
        ),
        Failures::default()
    );
}

/// The two stops' lines open with their kind and name what they stopped on.
#[test]
fn the_stop_lines_name_the_tests() {
    let red = Stop::RedBase {
        revision: "0123456789abcdef".to_owned(),
        tests: vec![
            "test_done_marks_one_item (test_todo.TodoTest.test_done_marks_one_item)".to_owned(),
        ],
        rerun: Some("to rerun pass `--lib`".to_owned()),
    };
    assert_eq!(
        red.to_string(),
        "RedBase (the tests fail at the case's base 01234567, before any step changed it: test_done_marks_one_item (test_todo.TodoTest.test_done_marks_one_item); to rerun pass `--lib`)"
    );
    let repeated = Stop::RepeatedTestFailure {
        tests: vec!["a::two".to_owned(), "a::three".to_owned()],
        count: 3,
    };
    assert_eq!(
        repeated.to_string(),
        "RepeatedTestFailure (3 tests.run results in a row, each at a new revision, failed the same tests: a::two, a::three)"
    );
}
