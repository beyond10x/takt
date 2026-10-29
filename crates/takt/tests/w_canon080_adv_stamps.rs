//! Adversary, canon 0.8.0 wave: the evidence port's restamp (`CaseEvidence::submit`,
//! `stamp_after`, `instant_micros`) promises "never before the case's latest record". These cases
//! drive it at the edge of the instants Canon reads (years 0000 to 9999): a latest record a caller
//! stamped in the last whole second Canon can write, or in its last nanoseconds.

#[path = "support/own_home.rs"]
mod own_home;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use takt::LoadedProtocol;
use takt::case::CaseDir;
use takt::evidence::{CaseEvidence, Record};
use takt::protocol::builtin_default;

const AT_A: &str = "aaaaaaaa11111111111111111111111111111111";
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
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "w-canon080-adv-stamps-{}-{name}",
        std::process::id()
    ));
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

/// Writes a record through the port; `given` set by the caller when present, else the clock's.
/// Answers the `observed_at` written.
fn submit(
    case: &mut CaseDir,
    n: usize,
    kind: &str,
    result: Option<&str>,
    given: Option<&str>,
) -> String {
    let mut record = Record::new(
        format!("e{n}"),
        kind,
        result.map(str::to_owned),
        "implementation",
        AT_A,
        PRODUCER,
    );
    if let Some(given) = given {
        record.observed_at = given.to_owned();
    }
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

/// Seconds since the epoch of an instant Canon writes, read by the same rule Canon reads it.
fn seconds(at: &str) -> i64 {
    b10x_canon::model::Instant::new(at)
        .seconds()
        .unwrap_or_else(|| panic!("{at} is an instant"))
}

/// The latest record a caller stamped in the last whole second Canon writes; the next record is of
/// a kind `story.delivery/2`'s `since` form reads with it, so the port moves it to the next tick.
/// That tick is in the year 10000, which `instant_micros` cannot format, and it falls back to the
/// epoch: the case's stamps go backwards by eight thousand years.
#[test]
fn an_ordered_stamp_after_the_last_second_canon_writes_is_never_before_it() {
    let tree = scratch("year-9999-ordered");
    let mut case = story_case(&tree.0, "year-9999-ordered");
    case.set_revision("implementation", AT_A)
        .expect("the revision is set");
    let latest = submit(
        &mut case,
        1,
        "tests_change",
        None,
        Some("9999-12-31T23:59:59Z"),
    );
    let next = submit(&mut case, 2, "test_result", Some("fail"), None);
    assert!(
        seconds(&next) >= seconds(&latest),
        "latest {latest}, then {next}: the stamp went backwards"
    );
}

/// The latest record a caller stamped in nanoseconds, in the last microsecond of year 9999: the
/// port rounds its instant up to the next microsecond (`LatestStamp::micros`), which is in the
/// year 10000, so even a record no order form compares with it is stamped at the epoch.
#[test]
fn an_unordered_stamp_after_the_last_nanoseconds_canon_writes_is_never_before_it() {
    let tree = scratch("year-9999-unordered");
    let mut case = story_case(&tree.0, "year-9999-unordered");
    case.set_revision("implementation", AT_A)
        .expect("the revision is set");
    let latest = submit(
        &mut case,
        1,
        "handover",
        None,
        Some("9999-12-31T23:59:59.999999001Z"),
    );
    let next = submit(&mut case, 2, "change_record", Some("ahead"), None);
    assert!(
        seconds(&next) >= seconds(&latest),
        "latest {latest}, then {next}: the stamp went backwards"
    );
}

/// Guard for an untested line (`CaseDir::latest_stamp` clamps the resolution to microseconds,
/// `.min(LatestStamp::MICRO_DIGITS)`): the unit's own `stamp_after` tests build `LatestStamp` by
/// hand and never run `latest_stamp` on a case whose records are all finer than a microsecond.
/// After a nanosecond record ahead of the clock, an ordered record is stamped at the start of the
/// next microsecond. Without the clamp the tick is read in nanoseconds as microseconds and the
/// stamp is not this one.
#[test]
fn an_ordered_stamp_after_a_nanosecond_stamp_is_the_next_microsecond() {
    let tree = scratch("nanos-ordered");
    let mut case = story_case(&tree.0, "nanos-ordered");
    case.set_revision("implementation", AT_A)
        .expect("the revision is set");
    submit(
        &mut case,
        1,
        "tests_change",
        None,
        Some("2999-01-01T00:00:00.123456789Z"),
    );
    let next = submit(&mut case, 2, "test_result", Some("fail"), None);
    assert_eq!(next, "2999-01-01T00:00:00.123457Z");
}
