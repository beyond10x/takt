//! The adversary's cases from its second pass over wave 44 unit U4 (loom 0.49.1, Canon 0.7.0,
//! engineering-protocols 0.13.0), kept unchanged: how the case's evidence port stamps a record
//! (`CaseEvidence::submit`, `now_after`). Canon reads two records in one second as of unknown
//! order, so a record is kept out of the second of the case's latest record of another kind where
//! an order form of the protocol reads both kinds, and is not held where none does.
//!
//! Retired from that pass: `the_second_record_of_one_observation_shares_no_second_with_another_kind`,
//! which asked for an order of `handover` and `paths_changed`, kinds no order form of
//! `story.delivery/2` reads, so their order decides nothing.
//!
//! Every record here goes through takt's own port, stamped by the clock (`Record::new`); none is
//! given a stamp by hand.

#[path = "support/own_home.rs"]
mod own_home;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use takt::LoadedProtocol;
use takt::case::CaseDir;
use takt::evidence::{CaseEvidence, Record};
use takt::protocol::builtin_default;

const AT_A: &str = "aaaaaaaa11111111111111111111111111111111";
const PRODUCER: &str = "takt.adversary";

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
        "w44-adv-record-stamps-{}-{name}",
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

/// A case on the built-in `id` with takt's built-in default for each input, its implementation
/// at [`AT_A`].
fn case_on(tree: &Path, id: &str, name: &str) -> CaseDir {
    let protocol = LoadedProtocol::builtin(id).expect("the built-in");
    let inputs: Vec<(String, String)> = protocol
        .inputs
        .iter()
        .map(|(input, _)| {
            let value = builtin_default(id, input).expect("a built-in default");
            (input.clone(), value.to_owned())
        })
        .collect();
    let mut case = CaseDir::open(tree, protocol, name, "a story", &inputs).expect("the case opens");
    case.set_revision("implementation", AT_A)
        .expect("the revision is set");
    case
}

/// Sleeps until the clock is early in a whole second.
fn early_in_a_second() {
    loop {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("the clock is past the epoch")
            .subsec_nanos();
        if nanos < 100_000_000 {
            return;
        }
        std::thread::sleep(Duration::from_nanos(u64::from(1_000_000_000 - nanos)));
    }
}

/// A record of `kind` about the implementation, stamped by the clock now, as a validator builds
/// one when its observation ends.
fn record(id: &str, kind: &str, result: Option<&str>) -> Record {
    Record::new(
        id.to_owned(),
        kind,
        result.map(str::to_owned),
        "implementation",
        AT_A,
        PRODUCER,
    )
}

/// Submits `records` in order through one evidence port, as the run loop submits a validator's
/// observation (`run.rs`, `for record in &observation.records`); each record's `observed_at` as
/// written.
fn submit_all(case: &mut CaseDir, records: &[Record]) -> Vec<String> {
    let port = CaseEvidence::new(case, vec![PRODUCER.to_owned()]).expect("the port opens");
    records
        .iter()
        .map(|record| {
            let path = port
                .submit(PRODUCER, record, None)
                .expect("the record is written");
            let written: serde_json::Value =
                serde_json::from_str(&fs::read_to_string(path).expect("the record")).expect("json");
            written["observed_at"]
                .as_str()
                .expect("observed_at")
                .to_owned()
        })
        .collect()
}

/// The run loop does the work after a validator's observation (the process sweep, the judge
/// surface's snapshot, git's status, what is outside the tree) between the record's stamp and
/// its submission. When that takes past the next second, the clock's own stamp is taken for a
/// given one and kept: the failing run shares its second with the `tests_change`, and
/// `story.delivery/2`'s `after` form reads their order as unknown again.
#[test]
fn a_clock_stamp_submitted_two_seconds_later_shares_no_second_with_another_kind() {
    let tree = scratch("late-submit");
    let mut case = case_on(&tree.0, "story.delivery/2", "late-submit");
    early_in_a_second();
    let written = submit_all(&mut case, &[record("e1", "tests_change", None)]);
    let failed = record("e2", "test_result", Some("fail"));
    std::thread::sleep(Duration::from_millis(2_000));
    let failed = submit_all(&mut case, &[failed]);
    assert_ne!(
        written[0], failed[0],
        "tests_change {} and the failing test_result {} share a second",
        written[0], failed[0]
    );
}

/// `story.delivery/1` has no order form: Canon never compares two of its records' instants, so
/// two records of different kinds in one second decide nothing differently. The port waits for
/// them all the same.
#[test]
fn a_protocol_with_no_order_form_is_not_held_a_second() {
    let tree = scratch("no-order");
    let mut case = case_on(&tree.0, "story.delivery/1", "no-order");
    early_in_a_second();
    let started = Instant::now();
    let written = submit_all(
        &mut case,
        &[
            record("e1", "test_result", Some("pass")),
            record("e2", "gate_result", Some("pass")),
        ],
    );
    let waited = started.elapsed();
    assert!(
        waited < Duration::from_millis(500),
        "two records under a protocol with no order form waited {waited:?} ({written:?})"
    );
}
