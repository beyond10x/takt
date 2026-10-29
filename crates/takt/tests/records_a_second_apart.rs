//! The case's evidence port stamps a record whose stamp is the clock's again when it is submitted,
//! to the microsecond (`YYYY-MM-DDTHH:MM:SS.ffffffZ`): never before the case's latest record, and
//! after it, at the resolution Canon compares the case's records in, where an order form of the
//! protocol reads both kinds. Canon 0.8.0 orders records by `observed_at` at the coarsest
//! precision written among them, so records all written to the microsecond are ordered within one
//! second; `story.delivery/2` of engineering-protocols 0.14.0 admits the edit only when a failing
//! `test_result` is later than the latest `tests_change`. The port never waits for the clock. A
//! stamp a caller set is kept.

#[path = "support/own_home.rs"]
mod own_home;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use takt::case::CaseDir;
use takt::evidence::{CaseEvidence, Record};
use takt::protocol::builtin_default;
use takt::{Decision, LoadedProtocol};

const AT_A: &str = "aaaaaaaa11111111111111111111111111111111";
const AT_B: &str = "bbbbbbbb22222222222222222222222222222222";
const PRODUCER: &str = "takt.tests-run";

fn git(tree: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(args)
        .current_dir(tree)
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn scratch(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("records-apart-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(&dir).expect("scratch dir");
    fs::write(dir.join("README.md"), "a story\n").expect("a file");
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "start"]);
    Scratch(dir)
}

/// A case on `story.delivery/2` with takt's built-in default for each input.
fn story_case(tree: &Path, name: &str) -> CaseDir {
    let id = "story.delivery/2";
    let protocol = LoadedProtocol::builtin(id).expect("the built-in");
    let inputs: Vec<(String, String)> = protocol
        .inputs
        .iter()
        .map(|(input, _)| {
            let value = builtin_default(id, input).expect("a built-in default");
            (input.clone(), value.to_owned())
        })
        .collect();
    CaseDir::open(tree, protocol, name, "a story", &inputs).expect("the case opens")
}

/// Sleeps until the clock is early in a whole second, so the next records are stamped in one
/// second unless something keeps them apart.
fn early_in_a_second() {
    loop {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("the clock is past the epoch")
            .subsec_nanos();
        if nanos < 100_000_000 {
            return;
        }
        std::thread::sleep(Duration::from_nanos(u64::from(1_000_000_000 - nanos)));
    }
}

/// Submits a record stamped by the clock now through the case's evidence port, about the
/// implementation at `at`, and answers the `observed_at` written.
fn submit(case: &mut CaseDir, n: usize, kind: &str, result: Option<&str>, at: &str) -> String {
    case.set_revision("implementation", at)
        .expect("the revision is set");
    let record = Record::new(
        format!("e{n}"),
        kind,
        result.map(str::to_owned),
        "implementation",
        at,
        PRODUCER,
    );
    let path = CaseEvidence::new(case, vec![PRODUCER.to_owned()])
        .expect("the port opens")
        .submit(PRODUCER, &record, None)
        .expect("the record is written");
    let written: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(path).expect("the record")).expect("json");
    written["observed_at"]
        .as_str()
        .expect("observed_at")
        .to_owned()
}

fn status(decision: &Decision, action: &str) -> String {
    decision
        .actions
        .get(action)
        .cloned()
        .unwrap_or_else(|| "absent".to_owned())
}

/// Whether `stamp` is an instant written to the microsecond: `YYYY-MM-DDTHH:MM:SS.ffffffZ`.
fn to_the_microsecond(stamp: &str) -> bool {
    stamp.len() == 27
        && stamp.as_bytes()[19] == b'.'
        && stamp.ends_with('Z')
        && stamp[20..26].bytes().all(|byte| byte.is_ascii_digit())
}

/// The tests are written and fail within one second: the failing run's record is stamped after
/// the `tests_change`, in the same second, both to the microsecond, and nothing waits.
#[test]
fn a_failing_run_in_the_second_the_tests_were_written_is_stamped_after_it_without_a_wait() {
    let tree = scratch("same-second");
    let mut case = story_case(&tree.0, "same-second");
    early_in_a_second();
    let started = Instant::now();
    let written = submit(&mut case, 1, "tests_change", None, AT_A);
    let failed = submit(&mut case, 2, "test_result", Some("fail"), AT_A);
    let waited = started.elapsed();
    assert!(
        failed > written,
        "tests_change {written}, test_result {failed}"
    );
    assert!(
        to_the_microsecond(&written) && to_the_microsecond(&failed),
        "tests_change {written}, test_result {failed}"
    );
    assert_eq!(written[..19], failed[..19], "one whole second");
    assert!(waited < Duration::from_millis(500), "waited {waited:?}");
}

/// The edit after a red gate: tests written, a red run half a second later in the same whole
/// second, then passing at a new revision and a red gate there. `story.delivery/2` admits
/// `implementation.edit` only when a failing run is later than the latest `tests_change` (its
/// `since` form), and it is, though the two share a second; the port did not hold the failing
/// run's record for the next second.
#[test]
fn a_red_run_half_a_second_after_the_tests_write_admits_the_edit_without_a_wait() {
    let tree = scratch("half-second");
    let mut case = story_case(&tree.0, "half-second");
    early_in_a_second();
    let written = submit(&mut case, 1, "tests_change", None, AT_A);
    std::thread::sleep(Duration::from_millis(500));
    let started = Instant::now();
    let failed = submit(&mut case, 2, "test_result", Some("fail"), AT_A);
    let waited = started.elapsed();
    assert_eq!(
        written[..19],
        failed[..19],
        "tests_change {written} and test_result {failed} share a whole second"
    );
    assert!(
        failed > written,
        "tests_change {written}, test_result {failed}"
    );
    assert!(waited < Duration::from_millis(300), "waited {waited:?}");
    submit(&mut case, 3, "test_result", Some("pass"), AT_B);
    submit(&mut case, 4, "gate_result", Some("fail"), AT_B);
    let decision = case.evaluate(0).expect("evaluates");
    assert_eq!(
        status(&decision, "implementation.edit"),
        "admissible",
        "{}",
        decision.json
    );
}

/// Writes a record of `kind` with `result` about the implementation at `revision`, stamped `at`
/// by the caller.
fn given_result(
    case: &mut CaseDir,
    n: usize,
    kind: &str,
    result: Option<&str>,
    revision: &str,
    at: &str,
) {
    case.set_revision("implementation", revision)
        .expect("the revision is set");
    let mut record = Record::new(
        format!("e{n}"),
        kind,
        result.map(str::to_owned),
        "implementation",
        revision,
        PRODUCER,
    );
    record.observed_at = at.to_owned();
    CaseEvidence::new(case, vec![PRODUCER.to_owned()])
        .expect("the port opens")
        .submit(PRODUCER, &record, None)
        .expect("the record is written");
}

/// The edit `story.delivery/2` gives after `tests_change` at [`AT_A`] stamped `stamps[0]`, a
/// failing `test_result` at [`AT_A`] stamped `stamps[1]`, and a passing `test_result` and a
/// failing `gate_result` at [`AT_B`] stamped `stamps[2]` and `stamps[3]`.
fn edit_after_a_red_gate(name: &str, stamps: [&str; 4]) -> String {
    let tree = scratch(name);
    let mut case = story_case(&tree.0, name);
    given_result(&mut case, 1, "tests_change", None, AT_A, stamps[0]);
    given_result(&mut case, 2, "test_result", Some("fail"), AT_A, stamps[1]);
    given_result(&mut case, 3, "test_result", Some("pass"), AT_B, stamps[2]);
    given_result(&mut case, 4, "gate_result", Some("fail"), AT_B, stamps[3]);
    let decision = case.evaluate(0).expect("evaluates");
    status(&decision, "implementation.edit")
}

/// At one precision Canon orders a red run within the second of the tests' write; in whole
/// seconds the same pair is of unknown order, and so it is when one record of the case is written
/// in whole seconds: Canon reads every instant of an evaluation at the coarsest precision written
/// among them.
#[test]
fn one_precision_orders_a_red_run_within_the_second_of_the_tests_write() {
    assert_eq!(
        edit_after_a_red_gate(
            "micro",
            [
                "2026-10-29T12:00:00.100000Z",
                "2026-10-29T12:00:00.600000Z",
                "2026-10-29T12:00:05.000000Z",
                "2026-10-29T12:00:06.000000Z",
            ]
        ),
        "admissible"
    );
    assert_eq!(
        edit_after_a_red_gate(
            "whole",
            [
                "2026-10-29T12:00:00Z",
                "2026-10-29T12:00:00Z",
                "2026-10-29T12:00:05Z",
                "2026-10-29T12:00:06Z",
            ]
        ),
        "blocked"
    );
    assert_eq!(
        edit_after_a_red_gate(
            "mixed",
            [
                "2026-10-29T12:00:00.100000Z",
                "2026-10-29T12:00:00.600000Z",
                "2026-10-29T12:00:05.000000Z",
                "2026-10-29T12:00:06Z",
            ]
        ),
        "blocked"
    );
}

/// The adversary's history, recorded through takt's own port with the clock's stamps: tests
/// written and failing within one second, made to pass, handed over, and a red gate. The edit
/// that fixes the gate is admitted under `story.delivery/2`.
#[test]
fn the_edit_after_a_red_gate_is_admitted_when_the_tests_failed_in_the_second_they_were_written() {
    let tree = scratch("red-gate");
    let mut case = story_case(&tree.0, "red-gate");
    early_in_a_second();
    submit(&mut case, 1, "tests_change", None, AT_A);
    submit(&mut case, 2, "test_result", Some("fail"), AT_A);
    submit(&mut case, 3, "test_result", Some("pass"), AT_B);
    submit(&mut case, 4, "handover", None, AT_B);
    submit(&mut case, 5, "gate_result", Some("fail"), AT_B);
    let decision = case.evaluate(0).expect("evaluates");
    assert_eq!(
        status(&decision, "implementation.edit"),
        "admissible",
        "{}",
        decision.json
    );
}

/// A record whose stamp was given, not taken from the clock a moment ago, keeps it, though it
/// shares its second with a record of another kind: a stamp in the past, and one ahead of the
/// clock, which the port neither moves nor waits for.
#[test]
fn a_given_stamp_in_the_same_second_is_kept() {
    for (name, given) in [
        ("given-past", "2020-01-01T00:00:00Z"),
        ("given-ahead", "2999-01-01T00:00:00Z"),
    ] {
        let started = Instant::now();
        assert_eq!(given_stamp_kept(name, given), given, "{name}");
        assert!(started.elapsed() < Duration::from_secs(5), "{name}: waited");
    }
}

/// Writes a `tests_change` and then a failing `test_result`, both stamped `given`, and answers
/// the failing run's `observed_at` as written.
fn given_stamp_kept(name: &str, given: &str) -> String {
    let tree = scratch(name);
    let mut case = story_case(&tree.0, name);
    case.set_revision("implementation", AT_A)
        .expect("the revision is set");
    let mut stamped = |n: usize, kind: &str, result: Option<&str>| {
        let mut record = Record::new(
            format!("e{n}"),
            kind,
            result.map(str::to_owned),
            "implementation",
            AT_A,
            PRODUCER,
        );
        record.observed_at = given.to_owned();
        let path = CaseEvidence::new(&mut case, vec![PRODUCER.to_owned()])
            .expect("the port opens")
            .submit(PRODUCER, &record, None)
            .expect("the record is written");
        let written: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(path).expect("the record")).expect("json");
        written["observed_at"]
            .as_str()
            .expect("observed_at")
            .to_owned()
    };
    stamped(1, "tests_change", None);
    stamped(2, "test_result", Some("fail"))
}

/// Writes a record of `kind` through the port with the stamp `given`, set by the caller.
fn given(case: &mut CaseDir, n: usize, kind: &str, given: &str) {
    given_result(case, n, kind, None, AT_A, given);
}

/// A case's stamps only grow: after a record stamped ahead of the clock, a record whose stamp is
/// the clock's is stamped no earlier. Its kind and the latest record's are read by one order form
/// (`since`, `tests_change` then `test_result`), and the case holds a record in whole seconds, so
/// Canon compares the case in whole seconds: the record is stamped at the start of the second
/// after the latest, at once.
#[test]
fn a_clock_stamp_after_one_ahead_of_the_clock_is_stamped_after_it() {
    let tree = scratch("ahead-ordered");
    let mut case = story_case(&tree.0, "ahead-ordered");
    case.set_revision("implementation", AT_A)
        .expect("the revision is set");
    given(&mut case, 1, "tests_change", "2999-01-01T00:00:00Z");
    let started = Instant::now();
    let failed = submit(&mut case, 2, "test_result", Some("fail"), AT_A);
    let waited = started.elapsed();
    assert_eq!(failed, "2999-01-01T00:00:01.000000Z");
    assert!(waited < Duration::from_millis(300), "waited {waited:?}");
}

/// After a record stamped to the microsecond ahead of the clock, an ordered record is stamped one
/// microsecond after it, at once.
#[test]
fn a_clock_stamp_after_a_microsecond_stamp_ahead_of_the_clock_is_one_microsecond_later() {
    let tree = scratch("ahead-micro");
    let mut case = story_case(&tree.0, "ahead-micro");
    case.set_revision("implementation", AT_A)
        .expect("the revision is set");
    given(&mut case, 1, "tests_change", "2999-01-01T00:00:00.250000Z");
    let started = Instant::now();
    let failed = submit(&mut case, 2, "test_result", Some("fail"), AT_A);
    let waited = started.elapsed();
    assert_eq!(failed, "2999-01-01T00:00:00.250001Z");
    assert!(waited < Duration::from_millis(300), "waited {waited:?}");
}

/// After a record stamped ahead of the clock, a record of a kind no order form reads with it is
/// stamped in that record's second, never earlier, and without a wait.
#[test]
fn a_clock_stamp_no_order_form_compares_is_not_held_but_never_earlier() {
    let tree = scratch("ahead-unordered");
    let mut case = story_case(&tree.0, "ahead-unordered");
    case.set_revision("implementation", AT_A)
        .expect("the revision is set");
    given(&mut case, 1, "handover", "2999-01-01T00:00:00Z");
    let started = Instant::now();
    let read = submit(&mut case, 2, "change_record", Some("ahead"), AT_A);
    let waited = started.elapsed();
    assert_eq!(read, "2999-01-01T00:00:00.000000Z");
    assert!(waited < Duration::from_millis(500), "waited {waited:?}");
}
