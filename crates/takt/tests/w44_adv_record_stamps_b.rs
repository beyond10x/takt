//! Adversary cases, pass 3, for wave 44 unit U4: the evidence port's re-stamping at submit
//! (`CaseEvidence::submit`, `second_for`), the protocol's order reads
//! (`LoadedProtocol::order_reads`), across a resume and for a protocol loaded from a file.
//!
//! Each case states what the round-3 decisions promise: a clock-stamped record of a kind an
//! order form reads with a kind recorded in the case's latest second is stamped out of it, and
//! stamps only grow.

#[path = "support/own_home.rs"]
mod own_home;

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

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
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("adversary-w44-u4c-{}-{name}", std::process::id()));
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

fn inputs_of(protocol: &LoadedProtocol, id: &str) -> Vec<(String, String)> {
    protocol
        .inputs
        .iter()
        .map(|(input, _)| {
            let value = builtin_default(id, input).expect("a built-in default");
            (input.clone(), value.to_owned())
        })
        .collect()
}

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

fn submit(case: &mut CaseDir, record: &Record) -> String {
    let path = CaseEvidence::new(case, vec![PRODUCER.to_owned()])
        .expect("the port opens")
        .submit(PRODUCER, record, None)
        .expect("the record is written");
    let written: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(path).expect("the record")).expect("json");
    written["observed_at"]
        .as_str()
        .expect("observed_at")
        .to_owned()
}

/// A protocol file whose only order form sits in an outcome's requirement, under `not` inside
/// `any` inside `all`, and a second one in a claim another claim reads: both are found.
#[test]
fn order_forms_nested_in_an_outcome_and_a_claim_are_read() {
    let two = LoadedProtocol::builtin("story.delivery/2").expect("the built-in");
    let yaml = two.yaml.replacen(
        "outcomes:\n",
        "outcomes:\n  probe_nested:\n    description: a probe\n    requires:\n      all:\n        - any:\n            - not:\n                before:\n                  first: handover\n                  then: gate_result\n            - claim: tests.pass\n",
        1,
    );
    assert_ne!(yaml, two.yaml, "the outcome was planted");
    let planted = LoadedProtocol::from_yaml(&yaml).expect("the planted protocol loads");
    let reads = planted.order_reads().expect("order reads");
    let pair: BTreeSet<String> = ["handover".to_owned(), "gate_result".to_owned()].into();
    assert!(reads.contains(&pair), "{reads:?}");
}

/// The case is resumed in another `CaseDir`, as a later run in another process reads it from
/// disk, and the failing run is recorded in the second of the first run's `tests_change`.
#[test]
fn a_resumed_case_keeps_an_ordered_pair_apart() {
    let tree = scratch("resume");
    let id = "story.delivery/2";
    let protocol = LoadedProtocol::builtin(id).expect("the built-in");
    let inputs = inputs_of(&protocol, id);
    early_in_a_second();
    let written = {
        let mut case =
            CaseDir::open(&tree.0, protocol, "resume", "a story", &inputs).expect("opens");
        case.set_revision("implementation", AT_A)
            .expect("the revision is set");
        submit(&mut case, &record("e1", "tests_change", None))
    };
    let mut again = CaseDir::resume(&tree.0, "resume").expect("the case resumes");
    let failed = submit(&mut again, &record("e2", "test_result", Some("fail")));
    assert!(
        failed > written,
        "tests_change {written}, test_result {failed}"
    );
}

/// The same pair under `story.delivery/2` loaded from a `protocol/1` file, as `--protocol
/// <path>` loads it.
#[test]
fn a_protocol_from_a_file_keeps_an_ordered_pair_apart() {
    let tree = scratch("from-file");
    let id = "story.delivery/2";
    let builtin = LoadedProtocol::builtin(id).expect("the built-in");
    let file = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("adversary-w44-u4c-{}-sd2.yaml", std::process::id()));
    fs::write(&file, &builtin.yaml).expect("the protocol file");
    let protocol = LoadedProtocol::resolve(file.to_str().expect("utf-8")).expect("the file");
    let _ = fs::remove_file(&file);
    let inputs = inputs_of(&protocol, id);
    let mut case =
        CaseDir::open(&tree.0, protocol, "from-file", "a story", &inputs).expect("opens");
    case.set_revision("implementation", AT_A)
        .expect("the revision is set");
    early_in_a_second();
    let written = submit(&mut case, &record("e1", "tests_change", None));
    let failed = submit(&mut case, &record("e2", "test_result", Some("fail")));
    assert!(
        failed > written,
        "tests_change {written}, test_result {failed}"
    );
}

/// A clock-stamped record held past a second before it is submitted (the run loop's checks
/// after an observation) is stamped at submit, never before the case's latest record.
#[test]
fn a_record_held_before_submit_is_stamped_at_submit_and_never_backwards() {
    let tree = scratch("held");
    let id = "story.delivery/2";
    let protocol = LoadedProtocol::builtin(id).expect("the built-in");
    let inputs = inputs_of(&protocol, id);
    let mut case = CaseDir::open(&tree.0, protocol, "held", "a story", &inputs).expect("opens");
    case.set_revision("implementation", AT_A)
        .expect("the revision is set");
    early_in_a_second();
    let held = record("e2", "test_result", Some("fail"));
    std::thread::sleep(Duration::from_millis(1_200));
    let written = submit(&mut case, &record("e1", "tests_change", None));
    let failed = submit(&mut case, &held);
    assert!(
        failed > written,
        "tests_change {written}, test_result {failed}"
    );
}
