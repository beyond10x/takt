//! Adversary pass 1 on a grant bound to its action's subject: a grant must never authorise a
//! revision it was not given for. Each case builds a protocol shape the unit's own suite does
//! not, or drives a shipped protocol through a path it does not, and asserts what an approval
//! may authorise afterwards.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use takt::case::{Authority, grant};
use takt::evidence::Record;
use takt::{CaseDir, Decision, LoadedProtocol};

const SELF_IMPROVEMENT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../protocols/self-improvement/1.yaml"
);

const LOOP_IMPROVEMENT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../protocols/loop-improvement/1.yaml"
);

/// The producer of every record; declared independent where the protocol has the dimension.
const VERIFIER: &str = "operator-run";

/// `mixed.merge` needs the `left` artifact checked and a `review` approved, the review match
/// naming no subject: Canon reads a review about any artifact, so the precondition depends on
/// `right` as much as on `left`.
const MIXED: &str = "format: protocol/1

protocol:
  id: grant.mixed
  revision: 1
  description: a gated action whose precondition reads evidence about any artifact

artifacts:
  implementation:
    description: a revision of the code
  left:
    description: the artifact the claim is about
  right:
    description: the artifact the review is about

evidence_kinds:
  check:
    description: a check of an artifact; its result is pass or fail
  review:
    description: a review of an artifact; its result is approved or rejected

claims:
  left.ok:
    description: the left artifact passes its check
    true_when:
      evidence:
        kind: check
        result: pass
        subject: left

actions:
  mixed.merge:
    description: merge once the left artifact passes and a review is approved
    effect: write
    precondition:
      all:
        - claim: left.ok
        - evidence:
            kind: review
            result: approved
    requires:
      - capability: mixed.cap

outcomes:
  accepted:
    description: the left artifact passes
    requires:
      claim: left.ok
";

/// Two gated actions on two artifacts: `left.merge` needs `left.cap` and is about `left`,
/// `right.merge` needs `right.cap` and is about `right`.
const TWO_SUBJECTS: &str = "format: protocol/1

protocol:
  id: grant.two
  revision: 1
  description: two gated actions, each about its own artifact

artifacts:
  implementation:
    description: a revision of the code
  left:
    description: one artifact
  right:
    description: another artifact

evidence_kinds:
  check:
    description: a check of an artifact; its result is pass or fail

claims:
  left.ok:
    description: the left artifact passes its check
    true_when:
      evidence:
        kind: check
        result: pass
        subject: left
  right.ok:
    description: the right artifact passes its check
    true_when:
      evidence:
        kind: check
        result: pass
        subject: right

actions:
  left.merge:
    description: merge the left artifact
    effect: write
    precondition:
      claim: left.ok
    requires:
      - capability: left.cap
  right.merge:
    description: merge the right artifact
    effect: write
    precondition:
      claim: right.ok
    requires:
      - capability: right.cap

outcomes:
  accepted:
    description: both artifacts pass
    requires:
      all:
        - claim: left.ok
        - claim: right.ok
";

/// One capability required by two actions about two different artifacts.
const SHARED: &str = "format: protocol/1

protocol:
  id: grant.shared
  revision: 1
  description: one capability required by two actions about different artifacts

artifacts:
  implementation:
    description: a revision of the code
  left:
    description: one artifact
  right:
    description: another artifact

evidence_kinds:
  check:
    description: a check of an artifact; its result is pass or fail

claims:
  left.ok:
    description: the left artifact passes its check
    true_when:
      evidence:
        kind: check
        result: pass
        subject: left
  right.ok:
    description: the right artifact passes its check
    true_when:
      evidence:
        kind: check
        result: pass
        subject: right

actions:
  a.merge:
    description: merge the left artifact
    effect: write
    precondition:
      claim: left.ok
    requires:
      - capability: shared.cap
  b.merge:
    description: merge the right artifact
    effect: write
    precondition:
      claim: right.ok
    requires:
      - capability: shared.cap

outcomes:
  accepted:
    description: both artifacts pass
    requires:
      all:
        - claim: left.ok
        - claim: right.ok
";

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
            eprintln!("remove the scratch tree {}: {error}", self.0.display());
        }
    }
}

fn scratch(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("grant-subject-adv1-{}-{name}", std::process::id()));
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

/// Opens a case on `protocol` with `inputs`, every artifact at `<artifact>-1`, the verifier
/// independent on `different_agent_run` where the protocol has that dimension.
fn open(tree: &Path, protocol: LoadedProtocol, id: &str, inputs: &[(&str, &str)]) -> CaseDir {
    let inputs: Vec<(String, String)> = inputs
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect();
    let artifacts = protocol.artifacts.clone();
    let independent = protocol.yaml.contains("different_agent_run:");
    let mut case = CaseDir::open(tree, protocol, id, "one turn", &inputs).expect("the case opens");
    for artifact in &artifacts {
        case.set_revision(artifact, &format!("{artifact}-1"))
            .expect("the revision is set");
    }
    if !independent {
        return case;
    }
    let snapshot = case.dir.join("case.yaml");
    let mut doc: serde_yaml_ng::Value =
        serde_yaml_ng::from_str(&fs::read_to_string(&snapshot).expect("case.yaml")).expect("yaml");
    let mut independence = serde_yaml_ng::Mapping::new();
    independence.insert(VERIFIER.into(), vec!["different_agent_run"].into());
    doc["independence"] = independence.into();
    fs::write(&snapshot, serde_yaml_ng::to_string(&doc).expect("yaml")).expect("case.yaml");
    CaseDir::resume(tree, id).expect("the case resumes")
}

fn add(case: &mut CaseDir, n: usize, kind: &str, result: &str, subject: &str) {
    let revision = case.revision(subject).expect("the subject has a revision");
    case.add_evidence(&Record::new(
        format!("e{n}"),
        kind,
        Some(result.to_owned()),
        subject,
        revision,
        VERIFIER,
    ))
    .expect("the record is written");
}

fn action<'a>(decision: &'a Decision, id: &str) -> &'a str {
    decision
        .actions
        .get(id)
        .map(String::as_str)
        .unwrap_or("absent")
}

/// Lead 1: a precondition with an evidence match that names no subject reads records about any
/// artifact, so the action depends on `right` as well as `left`. The grant is either refused
/// (no single artifact) or bound to both; bound to `left` alone, it still authorises
/// `mixed.merge` after `right` moved to a revision whose review the approver never saw.
#[test]
fn a_grant_does_not_survive_a_move_of_an_artifact_its_precondition_reads() {
    let tree = scratch("mixed");
    let protocol = LoadedProtocol::from_yaml(MIXED).expect("the protocol loads");
    let mut case = open(&tree, protocol, "mixed", &[]);
    add(&mut case, 1, "check", "pass", "left");
    add(&mut case, 2, "review", "approved", "right");
    let d = case.evaluate(0).expect("evaluates");
    assert_eq!(action(&d, "mixed.merge"), "approval-required", "{}", d.json);

    let granted = grant(&tree, "mixed", "mixed.cap", "human:operator");

    // `right` moves; the new revision is reviewed and approved by someone, never by the
    // approver.
    case.set_revision("right", "right-2")
        .expect("the right artifact moves");
    add(&mut case, 3, "review", "approved", "right");
    let d = case.evaluate(1).expect("evaluates");
    assert_ne!(
        action(&d, "mixed.merge"),
        "admissible",
        "a grant given at right-1 authorises mixed.merge at right-2; the grant was {granted:?}, \
         the subject read from the protocol {:?}",
        LoadedProtocol::from_yaml(MIXED)
            .expect("loads")
            .grant_subject("mixed.cap")
    );
}

/// Lead 1: one capability required by two actions about different artifacts is refused,
/// naming both actions, and nothing is written.
#[test]
fn a_capability_two_actions_on_two_artifacts_require_is_refused_naming_both() {
    let tree = scratch("shared");
    let protocol = LoadedProtocol::from_yaml(SHARED).expect("the protocol loads");
    let mut case = open(&tree, protocol, "shared", &[]);
    add(&mut case, 1, "check", "pass", "left");
    add(&mut case, 2, "check", "pass", "right");
    let d = case.evaluate(0).expect("evaluates");
    assert_eq!(action(&d, "a.merge"), "approval-required", "{}", d.json);
    assert_eq!(action(&d, "b.merge"), "approval-required", "{}", d.json);

    let error = grant(&tree, "shared", "shared.cap", "human:operator")
        .expect_err("two artifacts, no single one to bind the grant to");
    let message = format!("{error:#}");
    assert!(
        message.contains("action a.merge") && message.contains("action b.merge"),
        "the refusal names both actions: {message}"
    );
    assert!(
        Authority::read(&case.dir).expect("reads").is_none(),
        "a refused grant writes nothing"
    );
}

/// Lead 4: two grants on two artifacts do not interfere. A grant of `left.cap` does not admit
/// `right.merge`; once both are granted, moving `left` takes back `left.merge` only.
#[test]
fn grants_on_two_artifacts_authorise_each_its_own_action_only() {
    let tree = scratch("two");
    let protocol = LoadedProtocol::from_yaml(TWO_SUBJECTS).expect("the protocol loads");
    let mut case = open(&tree, protocol, "two", &[]);
    add(&mut case, 1, "check", "pass", "left");
    add(&mut case, 2, "check", "pass", "right");
    let d = case.evaluate(0).expect("evaluates");
    assert_eq!(action(&d, "left.merge"), "approval-required", "{}", d.json);
    assert_eq!(action(&d, "right.merge"), "approval-required", "{}", d.json);

    let left = grant(&tree, "two", "left.cap", "human:operator").expect("left granted");
    assert_eq!(left.revision, "left-1");
    let d = case.evaluate(1).expect("evaluates");
    assert_eq!(action(&d, "left.merge"), "admissible", "{}", d.json);
    assert_eq!(
        action(&d, "right.merge"),
        "approval-required",
        "a grant of left.cap admits right.merge: {}",
        d.json
    );

    let right = grant(&tree, "two", "right.cap", "human:operator").expect("right granted");
    assert_eq!(right.revision, "right-1");
    case.set_revision("left", "left-2")
        .expect("the left artifact moves");
    add(&mut case, 3, "check", "pass", "left");
    let d = case.evaluate(2).expect("evaluates");
    assert_eq!(
        action(&d, "left.merge"),
        "approval-required",
        "the left grant survives the move of left: {}",
        d.json
    );
    assert_eq!(
        action(&d, "right.merge"),
        "admissible",
        "the move of left takes back the right grant: {}",
        d.json
    );
}

/// Lead 3: after the candidate moves, the improvement turn is granted again at the new
/// candidate, the old grant replaced, and the new one admits the merge.
#[test]
fn an_improvement_turn_is_granted_again_after_its_candidate_moves() {
    let tree = scratch("regrant");
    let protocol = LoadedProtocol::from_file(Path::new(SELF_IMPROVEMENT)).expect("loads");
    let mut case = open(&tree, protocol, "turn", &[("surface", "judge")]);
    add(&mut case, 1, "finding_record", "filed", "finding");
    add(&mut case, 2, "scenario_result", "reproduced", "baseline");
    add(
        &mut case,
        3,
        "scenario_result",
        "not_reproduced",
        "candidate",
    );
    let d = case.evaluate(0).expect("evaluates");
    assert_eq!(
        action(&d, "candidate.merge_judge_surface"),
        "approval-required",
        "{}",
        d.json
    );
    grant(
        &tree,
        "turn",
        "repository.merge.judge_surface",
        "agent:planner",
    )
    .expect("granted at candidate-1");

    case.set_revision("candidate", "candidate-2")
        .expect("the candidate moves");
    add(
        &mut case,
        4,
        "scenario_result",
        "not_reproduced",
        "candidate",
    );
    let d = case.evaluate(1).expect("evaluates");
    assert_eq!(
        action(&d, "candidate.merge_judge_surface"),
        "approval-required",
        "{}",
        d.json
    );

    let approval = grant(
        &tree,
        "turn",
        "repository.merge.judge_surface",
        "agent:planner",
    )
    .expect("granted again at candidate-2");
    assert_eq!(approval.revision, "candidate-2");
    let authority = Authority::read(&case.dir)
        .expect("reads")
        .expect("an authority");
    assert_eq!(
        authority.decisions.len(),
        1,
        "one decision per capability: {authority:?}"
    );
    let d = case.evaluate(2).expect("evaluates");
    assert_eq!(
        action(&d, "candidate.merge_judge_surface"),
        "admissible",
        "{}",
        d.json
    );
}

/// Lead 5: `loop.merge` binds to `loop_candidate` and the grant authorises nothing once the
/// loop candidate moves.
#[test]
fn a_loop_turn_is_granted_at_its_loop_candidate_and_loses_it_when_it_moves() {
    let tree = scratch("loop");
    let protocol = LoadedProtocol::from_file(Path::new(LOOP_IMPROVEMENT)).expect("loads");
    let mut case = open(&tree, protocol, "loopturn", &[]);
    add(
        &mut case,
        1,
        "definition_check",
        "unchanged",
        "loop_candidate",
    );
    let d = case.evaluate(0).expect("evaluates");
    assert_eq!(action(&d, "loop.merge"), "approval-required", "{}", d.json);

    let approval = grant(
        &tree,
        "loopturn",
        "repository.merge.judge_surface",
        "human:owner",
    )
    .expect("granted");
    assert_eq!(approval.revision, "loop_candidate-1");
    let d = case.evaluate(1).expect("evaluates");
    assert_eq!(action(&d, "loop.merge"), "admissible", "{}", d.json);

    // The baseline moving is no move of what the grant binds.
    case.set_revision("loop_baseline", "loop_baseline-2")
        .expect("the baseline moves");
    let d = case.evaluate(2).expect("evaluates");
    assert_eq!(action(&d, "loop.merge"), "admissible", "{}", d.json);

    case.set_revision("loop_candidate", "loop_candidate-2")
        .expect("the loop candidate moves");
    add(
        &mut case,
        2,
        "definition_check",
        "unchanged",
        "loop_candidate",
    );
    let d = case.evaluate(3).expect("evaluates");
    assert_eq!(action(&d, "loop.merge"), "approval-required", "{}", d.json);
}

/// Lead 5: the subject every shipped protocol gives each capability it gates through a
/// precondition, as the protocol documents read.
#[test]
fn the_shipped_protocols_bind_each_capability_to_the_artifact_it_merges() {
    let delivery = LoadedProtocol::builtin("story.delivery/1").expect("built-in");
    assert_eq!(
        delivery
            .grant_subject("repository.merge")
            .expect("one subject"),
        "implementation"
    );
    let change = LoadedProtocol::builtin("software.change/1").expect("built-in");
    assert_eq!(
        change
            .grant_subject("repository.merge")
            .expect("one subject"),
        "implementation"
    );
    let turn = LoadedProtocol::from_file(Path::new(SELF_IMPROVEMENT)).expect("loads");
    for capability in [
        "repository.merge",
        "repository.merge.judge_surface",
        "takt.promote",
    ] {
        assert_eq!(
            turn.grant_subject(capability).expect("one subject"),
            "candidate",
            "{capability}"
        );
    }
    let loop_turn = LoadedProtocol::from_file(Path::new(LOOP_IMPROVEMENT)).expect("loads");
    assert_eq!(
        loop_turn
            .grant_subject("repository.merge.judge_surface")
            .expect("one subject"),
        "loop_candidate"
    );
}
