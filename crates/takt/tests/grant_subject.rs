//! A grant binds to the revision of the artifact its action is about, read from the protocol:
//! the subject of the evidence the action's precondition names. A `self.improvement/1` case is
//! granted `repository.merge.judge_surface` at its `candidate` revision, and the grant
//! authorises nothing once the candidate moves; a `story.delivery/1` case is granted at its
//! `implementation` revision as before; an action whose precondition names no single artifact
//! is refused a grant, naming the action.

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

/// The producer that delivered the candidate, and the one that verifies it.
const VERIFIER: &str = "operator-run";

/// Two artifacts beside `implementation`, and two gated actions: `pair.merge`, whose
/// precondition names evidence about `left` and about `right`, and `free.merge`, which has no
/// precondition at all. Neither names one artifact a grant could bind to.
const UNBOUND: &str = "format: protocol/1

protocol:
  id: grant.unbound
  revision: 1
  description: two gated actions whose preconditions name no single artifact

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
  pair.merge:
    description: merge both artifacts
    effect: write
    precondition:
      all:
        - claim: left.ok
        - claim: right.ok
    requires:
      - capability: pair.merge
  free.merge:
    description: merge without a precondition
    effect: write
    requires:
      - capability: free.merge

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
        .join(format!("grant-subject-{}-{name}", std::process::id()));
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

/// Opens a case on `protocol` with `inputs`, gives every artifact a revision of its own,
/// `<artifact>-1`, and declares the verifier independent on `different_agent_run` where the
/// protocol has that dimension.
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

/// The improvement turn at its judge-surface merge gate: the finding filed, reproduced on the
/// baseline and gone on the candidate.
fn at_the_judge_surface_gate(tree: &Path, id: &str) -> CaseDir {
    let protocol = LoadedProtocol::from_file(Path::new(SELF_IMPROVEMENT)).expect("loads");
    let mut case = open(tree, protocol, id, &[("surface", "judge")]);
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
    case
}

#[test]
fn an_improvement_turn_is_granted_at_its_candidate_revision() {
    let tree = scratch("candidate");
    let mut case = at_the_judge_surface_gate(&tree, "turn");

    let approval = grant(
        &tree,
        "turn",
        "repository.merge.judge_surface",
        "agent:planner",
    )
    .expect("the grant is written");
    assert_eq!(approval.revision, "candidate-1");
    let authority = Authority::read(&case.dir)
        .expect("reads")
        .expect("an authority");
    assert!(authority.grants_at("repository.merge.judge_surface", "candidate-1"));

    let d = case.evaluate(1).expect("evaluates");
    assert_eq!(
        action(&d, "candidate.merge_judge_surface"),
        "admissible",
        "{}",
        d.json
    );

    // The candidate moves and is verified again: the grant was given at the old revision and
    // authorises nothing at the new one.
    case.set_revision("candidate", "candidate-2")
        .expect("the candidate moves");
    add(
        &mut case,
        4,
        "scenario_result",
        "not_reproduced",
        "candidate",
    );
    let d = case.evaluate(2).expect("evaluates");
    assert_eq!(
        action(&d, "candidate.merge_judge_surface"),
        "approval-required",
        "{}",
        d.json
    );
    // Read again after the move: the grant is still on file, at the old candidate only.
    let authority = Authority::read(&case.dir)
        .expect("reads")
        .expect("an authority");
    assert!(authority.grants_at("repository.merge.judge_surface", "candidate-1"));
    assert!(!authority.grants_at("repository.merge.judge_surface", "candidate-2"));
}

#[test]
fn a_software_change_rollback_is_refused_a_grant_naming_the_action() {
    let tree = scratch("rollback");
    let protocol = LoadedProtocol::builtin("software.change/1").expect("a built-in");
    let case = open(
        &tree,
        protocol,
        "rollback",
        &[
            ("risk", "standard"),
            ("change_kind", "bugfix"),
            ("affects_security_boundary", "false"),
        ],
    );
    let d = case.evaluate(0).expect("evaluates");
    assert_eq!(
        action(&d, "release.rollback"),
        "approval-required",
        "{}",
        d.json
    );

    let error = grant(&tree, "rollback", "release.rollback", "human:operator")
        .expect_err("release.rollback has no precondition, so no artifact to bind to");
    let message = format!("{error:#}");
    assert!(
        message.contains("action release.rollback"),
        "the refusal names the action: {message}"
    );
    assert!(
        Authority::read(&case.dir).expect("reads").is_none(),
        "a refused grant writes nothing"
    );
}

#[test]
fn a_story_delivery_case_is_granted_at_its_implementation_revision() {
    let tree = scratch("delivery");
    let protocol = LoadedProtocol::builtin("story.delivery/1").expect("a built-in");
    let mut case = open(&tree, protocol, "delivery", &[]);
    add(&mut case, 1, "test_result", "pass", "implementation");
    add(&mut case, 2, "gate_result", "pass", "implementation");
    add(&mut case, 3, "pull_request", "open", "implementation");
    add(&mut case, 4, "ci_result", "pass", "implementation");
    let d = case.evaluate(0).expect("evaluates");
    assert_eq!(
        action(&d, "repository.merge"),
        "approval-required",
        "{}",
        d.json
    );

    let approval = grant(&tree, "delivery", "repository.merge", "human:operator").expect("granted");
    assert_eq!(approval.revision, "implementation-1");
    let d = case.evaluate(1).expect("evaluates");
    assert_eq!(action(&d, "repository.merge"), "admissible", "{}", d.json);

    case.set_revision("implementation", "implementation-2")
        .expect("the implementation moves");
    for (n, kind, result) in [
        (5, "test_result", "pass"),
        (6, "gate_result", "pass"),
        (7, "pull_request", "open"),
        (8, "ci_result", "pass"),
    ] {
        add(&mut case, n, kind, result, "implementation");
    }
    let d = case.evaluate(2).expect("evaluates");
    assert_eq!(
        action(&d, "repository.merge"),
        "approval-required",
        "{}",
        d.json
    );
}

#[test]
fn an_action_about_no_single_artifact_is_refused_a_grant_naming_it() {
    let tree = scratch("unbound");
    let protocol = LoadedProtocol::from_yaml(UNBOUND).expect("the protocol loads");
    let mut case = open(&tree, protocol, "unbound", &[]);
    add(&mut case, 1, "check", "pass", "left");
    add(&mut case, 2, "check", "pass", "right");
    let d = case.evaluate(0).expect("evaluates");
    assert_eq!(action(&d, "pair.merge"), "approval-required", "{}", d.json);
    assert_eq!(action(&d, "free.merge"), "approval-required", "{}", d.json);

    for (capability, named) in [("pair.merge", "pair.merge"), ("free.merge", "free.merge")] {
        let error = grant(&tree, "unbound", capability, "human:operator")
            .expect_err("no single artifact to bind the grant to");
        let message = format!("{error:#}");
        assert!(
            message.contains(&format!("action {named}")),
            "the refusal names the action: {message}"
        );
    }
    assert!(
        Authority::read(&case.dir).expect("reads").is_none(),
        "a refused grant writes nothing"
    );
}
