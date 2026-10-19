//! The improvement loops as data: `protocols/self-improvement/1.yaml` (one turn improving takt)
//! and `protocols/loop-improvement/1.yaml` (one turn improving that loop) are loaded, validated
//! and compiled by Canon, and cases on them are driven record by record to the outcomes the
//! design names: a fix verified by another run, merged under the authority its surface needs,
//! with no regression and promoted is accepted; a fix that leaves the finding reproducing is
//! declined; a loop change that redefines a metric without the old figures cannot be merged.
//! The records here are written by hand so that they decide the protocol's reading alone. A
//! turn's carry the names of the turn validators that write them
//! (`crates/takt/src/improvement.rs`, driven in `tests/self_improvement.rs` and by `takt run` in
//! `crates/takt-cli/tests/self_improvement_cli.rs`), and the turn opens through the kernel's
//! opening method, which declares those validators independent; the merge and promotion
//! validators come with story:judge-guard, and their records here are the delivering run's. The
//! `loop.improvement/1` actions have theirs (`crates/takt/src/loop_scorecard.rs`, driven in
//! `tests/loop_scorecard.rs`).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use takt::case::TurnOpening;
use takt::evidence::Record;
use takt::improvement::{FindingFile, ScenarioReproduce, ScenarioVerify, ScorecardCompare};
use takt::{CaseDir, Decision, LoadedProtocol};

const SELF_IMPROVEMENT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../protocols/self-improvement/1.yaml"
);
const LOOP_IMPROVEMENT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../protocols/loop-improvement/1.yaml"
);

/// The producer that delivered the candidate, and the one that verifies a loop change.
const DELIVERER: &str = "developer-run";
const VERIFIER: &str = "operator-run";

/// In a turn, the validator of the turn that writes the record's kind ([`judge_of`]).
const JUDGE: &str = "judge";

fn git(tree: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(args)
        .current_dir(tree)
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?}");
}

/// A scratch git tree with one commit under this crate's build directory, removed on drop.
struct Scratch(PathBuf);

impl std::ops::Deref for Scratch {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            let message = format!("remove the scratch tree {}: {error}", self.0.display());
            if std::thread::panicking() {
                eprintln!("{message}");
            } else {
                panic!("{message}");
            }
        }
    }
}

fn scratch(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("improvement-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(&dir).expect("scratch dir");
    fs::write(dir.join("README.md"), "takt\n").expect("a file");
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "start"]);
    Scratch(dir)
}

/// Opens a case on `protocol` with `inputs`. A turn (`self.improvement/1`) opens through the
/// kernel's opening method, its baseline and candidate revisions of their own, which declares
/// every turn validator independent of the case's work on `different_agent_run`: nothing here
/// writes `independence`. A loop case gives every artifact a revision of its own and declares the
/// verifier independent by hand, as the loop protocol's opening does not yet (story:loop-scorecard).
fn open(tree: &Path, protocol: &str, id: &str, inputs: &[(&str, &str)]) -> CaseDir {
    let turn = protocol == SELF_IMPROVEMENT;
    let protocol = LoadedProtocol::from_file(Path::new(protocol))
        .expect("the protocol loads, validates and compiles");
    let inputs: Vec<(String, String)> = inputs
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect();
    let artifacts = protocol.artifacts.clone();
    let mut case =
        CaseDir::open(tree, protocol, id, "one improvement turn", &inputs).expect("the case opens");
    if turn {
        case.open_turn(&TurnOpening {
            finding: "story:a-finding".to_owned(),
            candidate: "candidate-1".to_owned(),
            baseline: "baseline-1".to_owned(),
        })
        .expect("the turn opens");
        return case;
    }
    for artifact in &artifacts {
        case.set_revision(artifact, &format!("{artifact}-1"))
            .expect("the revision is set");
    }
    let snapshot = case.dir.join("case.yaml");
    let mut doc: serde_yaml_ng::Value =
        serde_yaml_ng::from_str(&fs::read_to_string(&snapshot).expect("case.yaml")).expect("yaml");
    let mut independence = serde_yaml_ng::Mapping::new();
    independence.insert(VERIFIER.into(), vec!["different_agent_run"].into());
    doc["independence"] = independence.into();
    fs::write(&snapshot, serde_yaml_ng::to_string(&doc).expect("yaml")).expect("case.yaml");
    CaseDir::resume(tree, id).expect("the case resumes with its independence")
}

/// The turn validator that writes `kind` about `subject`: the producer [`JUDGE`] stands for.
fn judge_of(kind: &str, subject: &str) -> &'static str {
    match (kind, subject) {
        ("finding_record", _) => FindingFile::NAME,
        ("scenario_result", "baseline") => ScenarioReproduce::NAME,
        ("scenario_result", _) => ScenarioVerify::NAME,
        ("scorecard_result", _) => ScorecardCompare::NAME,
        _ => panic!("no turn validator writes {kind}"),
    }
}

fn add(case: &mut CaseDir, n: usize, kind: &str, result: &str, subject: &str, producer: &str) {
    let producer = if producer == JUDGE {
        judge_of(kind, subject)
    } else {
        producer
    };
    let revision = case.revision(subject).expect("the subject has a revision");
    case.add_evidence(&Record::new(
        format!("e{n}"),
        kind,
        Some(result.to_owned()),
        subject,
        revision,
        producer,
    ))
    .expect("the record is written");
}

fn claim<'a>(decision: &'a Decision, id: &str) -> &'a str {
    decision
        .claims
        .get(id)
        .map(String::as_str)
        .unwrap_or("absent")
}

fn status<'a>(section: &'a std::collections::BTreeMap<String, String>, id: &str) -> &'a str {
    section.get(id).map(String::as_str).unwrap_or("absent")
}

#[test]
fn both_loops_load_as_protocols() {
    let turn = LoadedProtocol::from_file(Path::new(SELF_IMPROVEMENT)).expect("loads");
    assert_eq!(turn.id, "self.improvement/1");
    assert_eq!(
        turn.artifacts,
        ["finding", "baseline", "candidate", "promotion"]
    );
    let meta = LoadedProtocol::from_file(Path::new(LOOP_IMPROVEMENT)).expect("loads");
    assert_eq!(meta.id, "loop.improvement/1");
    assert!(meta.actions.contains(&"loop.merge".to_owned()));
}

#[test]
fn a_verified_fix_without_regression_is_accepted_once_promoted() {
    let tree = scratch("accepted");
    let mut case = open(
        &tree,
        SELF_IMPROVEMENT,
        "accepted",
        &[("surface", "non_judge")],
    );
    let d = case.evaluate(0).expect("evaluates");
    assert_eq!(
        status(&d.actions, "finding.file"),
        "admissible",
        "{}",
        d.json
    );
    assert_eq!(
        status(&d.actions, "scenario.reproduce"),
        "blocked",
        "{}",
        d.json
    );

    add(&mut case, 1, "finding_record", "filed", "finding", JUDGE);
    add(
        &mut case,
        2,
        "scenario_result",
        "reproduced",
        "baseline",
        JUDGE,
    );
    let d = case.evaluate(1).expect("evaluates");
    assert_eq!(
        claim(&d, "scenario.reproduced_before"),
        "true",
        "{}",
        d.json
    );
    assert_eq!(
        status(&d.actions, "candidate.deliver"),
        "admissible",
        "{}",
        d.json
    );

    add(
        &mut case,
        3,
        "scenario_result",
        "not_reproduced",
        "candidate",
        JUDGE,
    );
    let d = case.evaluate(2).expect("evaluates");
    assert_eq!(claim(&d, "scenario.passes_after"), "true", "{}", d.json);
    assert_eq!(
        status(&d.actions, "candidate.merge"),
        "approval-required",
        "{}",
        d.json
    );
    assert_eq!(
        status(&d.actions, "candidate.merge_judge_surface"),
        "blocked",
        "{}",
        d.json
    );
    assert_eq!(status(&d.outcomes, "accepted"), "blocked", "{}", d.json);

    add(
        &mut case,
        4,
        "merge_observation",
        "merged",
        "candidate",
        DELIVERER,
    );
    add(
        &mut case,
        5,
        "scorecard_result",
        "no_regression",
        "candidate",
        JUDGE,
    );
    let d = case.evaluate(3).expect("evaluates");
    assert_eq!(
        status(&d.actions, "binary.promote"),
        "approval-required",
        "{}",
        d.json
    );
    assert_eq!(status(&d.outcomes, "accepted"), "blocked", "{}", d.json);

    add(
        &mut case,
        6,
        "promotion_record",
        "promoted",
        "promotion",
        DELIVERER,
    );
    let d = case.evaluate(4).expect("evaluates");
    assert_eq!(status(&d.outcomes, "accepted"), "legitimate", "{}", d.json);
    assert_eq!(status(&d.outcomes, "declined"), "blocked", "{}", d.json);
}

#[test]
fn the_delivering_run_cannot_verify_its_own_fix() {
    let tree = scratch("self-verified");
    let mut case = open(
        &tree,
        SELF_IMPROVEMENT,
        "self-verified",
        &[("surface", "non_judge")],
    );
    add(&mut case, 1, "finding_record", "filed", "finding", JUDGE);
    add(
        &mut case,
        2,
        "scenario_result",
        "reproduced",
        "baseline",
        JUDGE,
    );
    add(
        &mut case,
        3,
        "scenario_result",
        "not_reproduced",
        "candidate",
        DELIVERER,
    );
    let d = case.evaluate(1).expect("evaluates");
    assert_ne!(claim(&d, "scenario.passes_after"), "true", "{}", d.json);
    assert_eq!(
        status(&d.actions, "candidate.merge"),
        "blocked",
        "{}",
        d.json
    );
}

#[test]
fn a_judge_surface_change_needs_the_judge_surface_capability() {
    let tree = scratch("judge");
    let mut case = open(&tree, SELF_IMPROVEMENT, "judge", &[("surface", "judge")]);
    add(&mut case, 1, "finding_record", "filed", "finding", JUDGE);
    add(
        &mut case,
        2,
        "scenario_result",
        "reproduced",
        "baseline",
        JUDGE,
    );
    add(
        &mut case,
        3,
        "scenario_result",
        "not_reproduced",
        "candidate",
        JUDGE,
    );
    let d = case.evaluate(1).expect("evaluates");
    assert_eq!(
        status(&d.actions, "candidate.merge"),
        "blocked",
        "{}",
        d.json
    );
    assert_eq!(
        status(&d.actions, "candidate.merge_judge_surface"),
        "approval-required",
        "{}",
        d.json
    );
}

#[test]
fn a_fix_that_leaves_the_finding_reproducing_is_declined() {
    let tree = scratch("declined");
    let mut case = open(
        &tree,
        SELF_IMPROVEMENT,
        "declined",
        &[("surface", "non_judge")],
    );
    add(&mut case, 1, "finding_record", "filed", "finding", JUDGE);
    add(
        &mut case,
        2,
        "scenario_result",
        "reproduced",
        "baseline",
        JUDGE,
    );
    add(
        &mut case,
        3,
        "scenario_result",
        "reproduced",
        "candidate",
        JUDGE,
    );
    let d = case.evaluate(1).expect("evaluates");
    assert_eq!(status(&d.outcomes, "declined"), "legitimate", "{}", d.json);
    assert_eq!(status(&d.outcomes, "accepted"), "blocked", "{}", d.json);
}

#[test]
fn a_new_candidate_revision_keeps_the_baseline_reproduction() {
    let tree = scratch("revision");
    let mut case = open(
        &tree,
        SELF_IMPROVEMENT,
        "revision",
        &[("surface", "non_judge")],
    );
    add(&mut case, 1, "finding_record", "filed", "finding", JUDGE);
    add(
        &mut case,
        2,
        "scenario_result",
        "reproduced",
        "baseline",
        JUDGE,
    );
    add(
        &mut case,
        3,
        "scenario_result",
        "not_reproduced",
        "candidate",
        JUDGE,
    );
    case.set_revision("candidate", "candidate-2")
        .expect("the candidate moves");
    let d = case.evaluate(1).expect("evaluates");
    assert_eq!(
        claim(&d, "scenario.reproduced_before"),
        "true",
        "{}",
        d.json
    );
    assert_ne!(claim(&d, "scenario.passes_after"), "true", "{}", d.json);
}

#[test]
fn a_loop_change_that_redefines_a_metric_silently_cannot_be_merged() {
    let tree = scratch("loop-redefined");
    let mut case = open(&tree, LOOP_IMPROVEMENT, "loop-redefined", &[]);
    add(
        &mut case,
        1,
        "loop_measurement",
        "below_target",
        "loop_baseline",
        VERIFIER,
    );
    add(
        &mut case,
        2,
        "definition_check",
        "redefined",
        "loop_candidate",
        VERIFIER,
    );
    let d = case.evaluate(1).expect("evaluates");
    assert_eq!(claim(&d, "loop.metric_below_target"), "true", "{}", d.json);
    assert_ne!(claim(&d, "loop.definition_honest"), "true", "{}", d.json);
    assert_eq!(status(&d.actions, "loop.merge"), "blocked", "{}", d.json);
}

#[test]
fn an_honest_loop_change_whose_metric_improved_is_accepted() {
    let tree = scratch("loop-accepted");
    let mut case = open(&tree, LOOP_IMPROVEMENT, "loop-accepted", &[]);
    add(
        &mut case,
        1,
        "loop_measurement",
        "below_target",
        "loop_baseline",
        VERIFIER,
    );
    add(
        &mut case,
        2,
        "definition_check",
        "dual_reported",
        "loop_candidate",
        VERIFIER,
    );
    let d = case.evaluate(1).expect("evaluates");
    assert_eq!(
        status(&d.actions, "loop.merge"),
        "approval-required",
        "{}",
        d.json
    );

    add(
        &mut case,
        3,
        "merge_observation",
        "merged",
        "loop_candidate",
        DELIVERER,
    );
    add(
        &mut case,
        4,
        "loop_comparison",
        "improved",
        "loop_candidate",
        VERIFIER,
    );
    let d = case.evaluate(2).expect("evaluates");
    assert_eq!(status(&d.outcomes, "accepted"), "legitimate", "{}", d.json);
}

#[test]
fn a_loop_change_whose_metric_did_not_improve_is_declined() {
    let tree = scratch("loop-declined");
    let mut case = open(&tree, LOOP_IMPROVEMENT, "loop-declined", &[]);
    add(
        &mut case,
        1,
        "loop_measurement",
        "below_target",
        "loop_baseline",
        VERIFIER,
    );
    add(
        &mut case,
        2,
        "definition_check",
        "unchanged",
        "loop_candidate",
        VERIFIER,
    );
    add(
        &mut case,
        3,
        "merge_observation",
        "merged",
        "loop_candidate",
        DELIVERER,
    );
    add(
        &mut case,
        4,
        "loop_comparison",
        "not_improved",
        "loop_candidate",
        VERIFIER,
    );
    let d = case.evaluate(1).expect("evaluates");
    assert_eq!(status(&d.outcomes, "declined"), "legitimate", "{}", d.json);
    assert_eq!(status(&d.outcomes, "accepted"), "blocked", "{}", d.json);
}
