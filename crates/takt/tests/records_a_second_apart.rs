//! The case's evidence port stamps a record whose stamp is the clock's again when it is submitted:
//! never before the case's latest record, and out of that record's second where an order form of
//! the protocol reads both kinds. Canon orders records by `observed_at` in whole seconds and reads
//! two records in one second as of unknown order; `story.delivery/2` of engineering-protocols
//! 0.13.0 admits the edit after a red gate only when a failing `test_result` is later than the
//! first `tests_change`. A stamp a caller set is kept, and the port does not wait on it.

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

/// The tests are written and fail within one second: the failing run's record is stamped in a
/// later second than the `tests_change`, after at most a second's wait.
#[test]
fn a_failing_run_in_the_second_the_tests_were_written_is_stamped_in_the_next() {
    let tree = scratch("next-second");
    let mut case = story_case(&tree.0, "next-second");
    early_in_a_second();
    let started = Instant::now();
    let written = submit(&mut case, 1, "tests_change", None, AT_A);
    let failed = submit(&mut case, 2, "test_result", Some("fail"), AT_A);
    let waited = started.elapsed();
    assert!(
        failed > written,
        "tests_change {written}, test_result {failed}"
    );
    assert!(waited < Duration::from_millis(1_500), "waited {waited:?}");
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
    let mut record = Record::new(
        format!("e{n}"),
        kind,
        None,
        "implementation",
        AT_A,
        PRODUCER,
    );
    record.observed_at = given.to_owned();
    CaseEvidence::new(case, vec![PRODUCER.to_owned()])
        .expect("the port opens")
        .submit(PRODUCER, &record, None)
        .expect("the record is written");
}

/// A case's stamps only grow: after a record stamped ahead of the clock, a record whose stamp is
/// the clock's is stamped no earlier. Its kind and the latest record's are read by one order form
/// (`after`, `tests_change` then `test_result`), so it is stamped the second after the latest
/// once the wait's bound is spent: the system clock cannot reach that second.
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
    assert_eq!(failed, "2999-01-01T00:00:01Z");
    assert!(waited < Duration::from_millis(1_500), "waited {waited:?}");
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
    assert_eq!(read, "2999-01-01T00:00:00Z");
    assert!(waited < Duration::from_millis(500), "waited {waited:?}");
}
