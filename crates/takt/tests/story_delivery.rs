//! The story-delivery protocol as data: `protocols/story-delivery/1.yaml` is loaded, validated
//! and compiled by Canon, a case is opened on it over a scratch git tree, and the five records
//! the delivery of a story produces are added one at a time, each evaluated: one more claim true
//! after each, `repository.merge` approval-required before the last, `accepted` legitimate only
//! after it. Every record is about `implementation` at its revision, as the kernel binds claims.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use takt::evidence::Record;
use takt::{CaseDir, Decision, LoadedProtocol};

/// The protocol under test, relative to this crate.
const PROTOCOL: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../protocols/story-delivery/1.yaml"
);

/// The records a delivery produces, in order, each with the claim it establishes. The merge is
/// its own kind: Canon leaves a match unknown when records of one kind disagree, so a `merged`
/// beside an `open` record of `pull_request` at one revision would decide neither claim.
const LADDER: [(&str, &str, &str); 5] = [
    ("test_result", "pass", "tests.pass"),
    ("gate_result", "pass", "gate.green"),
    ("pull_request", "open", "pr.opened"),
    ("ci_result", "pass", "ci.green"),
    ("merge_observation", "merged", "pr.merged"),
];

fn git(tree: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(args)
        .current_dir(tree)
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?}");
}

/// A scratch git tree with one commit, under this crate's build directory.
fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("story-delivery-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("scratch dir");
    fs::write(dir.join("README.md"), "a story\n").expect("a file");
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "start"]);
    dir
}

fn open(tree: &Path, id: &str) -> CaseDir {
    let protocol = LoadedProtocol::from_file(Path::new(PROTOCOL))
        .expect("the protocol loads, validates and compiles");
    CaseDir::open(
        tree,
        protocol,
        id,
        "deliver the story",
        &[("risk".to_owned(), "trivial".to_owned())],
    )
    .expect("the case opens")
}

fn record(n: usize, kind: &str, result: &str, revision: &str) -> Record {
    Record::new(
        format!("e{n}"),
        kind,
        Some(result.to_owned()),
        "implementation",
        revision,
        "story-delivery-test",
    )
}

/// The claims the decision holds, in id order.
fn held(decision: &Decision) -> Vec<&str> {
    decision
        .claims
        .iter()
        .filter(|(_, value)| value.as_str() == "true")
        .map(|(id, _)| id.as_str())
        .collect()
}

fn status<'a>(section: &'a std::collections::BTreeMap<String, String>, id: &str) -> &'a str {
    section.get(id).map(String::as_str).unwrap_or("absent")
}

#[test]
fn each_record_earns_one_claim_and_accepted_needs_the_merge() {
    let tree = scratch("ladder");
    let mut case = open(&tree, "ladder");
    assert_eq!(case.protocol.id, "story.delivery/1");
    let revision = case
        .revision("implementation")
        .expect("implementation has a revision");

    let decision = case.evaluate(0).expect("evaluates");
    assert!(held(&decision).is_empty(), "{}", decision.json);
    assert_eq!(status(&decision.actions, "pr.open"), "blocked");
    assert_eq!(status(&decision.actions, "repository.merge"), "blocked");
    assert_eq!(status(&decision.outcomes, "accepted"), "blocked");

    for (n, (kind, result, claim)) in LADDER.iter().enumerate() {
        let seq = n + 1;
        case.add_evidence(&record(seq, kind, result, &revision))
            .expect("the record is written");
        let decision = case.evaluate(seq).expect("evaluates");
        let mut expected: Vec<&str> = LADDER[..seq].iter().map(|(_, _, c)| *c).collect();
        expected.sort_unstable();
        assert_eq!(
            held(&decision),
            expected,
            "after {kind} {result}: {claim} should be one more claim true\n{}",
            decision.json
        );
        let pr_open = status(&decision.actions, "pr.open");
        let merge = status(&decision.actions, "repository.merge");
        let accepted = status(&decision.outcomes, "accepted");
        match seq {
            1 => {
                assert_eq!(pr_open, "blocked", "{}", decision.json);
                assert_eq!(merge, "blocked", "{}", decision.json);
                assert_eq!(accepted, "blocked", "{}", decision.json);
            }
            2 | 3 => {
                assert_eq!(pr_open, "admissible", "{}", decision.json);
                assert_eq!(merge, "blocked", "{}", decision.json);
                assert_eq!(accepted, "blocked", "{}", decision.json);
            }
            4 => {
                assert_eq!(merge, "approval-required", "{}", decision.json);
                assert_eq!(accepted, "blocked", "{}", decision.json);
            }
            _ => {
                assert_eq!(merge, "approval-required", "{}", decision.json);
                assert_eq!(accepted, "legitimate", "{}", decision.json);
            }
        }
    }
    let _ = fs::remove_dir_all(&tree);
}

#[test]
fn a_new_implementation_revision_returns_every_claim_to_unknown() {
    let tree = scratch("revision");
    let mut case = open(&tree, "revision");
    let revision = case
        .revision("implementation")
        .expect("implementation has a revision");
    for (n, (kind, result, _)) in LADDER.iter().enumerate() {
        case.add_evidence(&record(n + 1, kind, result, &revision))
            .expect("the record is written");
    }
    let decision = case.evaluate(1).expect("evaluates");
    assert_eq!(held(&decision).len(), LADDER.len(), "{}", decision.json);
    assert_eq!(status(&decision.outcomes, "accepted"), "legitimate");

    case.set_revision("implementation", "0000000000000000000000000000000000000000")
        .expect("the revision moves");
    let decision = case.evaluate(2).expect("evaluates");
    assert!(held(&decision).is_empty(), "{}", decision.json);
    assert!(
        decision.claims.values().all(|value| value == "unknown"),
        "{:?}",
        decision.claims
    );
    assert_eq!(status(&decision.actions, "repository.merge"), "blocked");
    assert_eq!(status(&decision.outcomes, "accepted"), "blocked");
    let _ = fs::remove_dir_all(&tree);
}
