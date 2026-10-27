//! The governor of Loom holds and decides takt's cases (`story:governor-holds-takts-cases`).
//!
//! `CaseFiles` is the governor's case store over `.takt/cases/<id>/`: the case's files are
//! where takt writes them, and `observations.json` beside them. Every `decisions/NNNN.json` is the
//! `canon-decision/1` document the governor rendered for that evaluation, which supplies no
//! authority. A grant reaches the run as the answer of `CaseAuthority`, the authority provider
//! over the case's `authority.yaml`: allowed only at the revision of its subject it was given at.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

use anyhow::anyhow;
use loom_sdk::commission::model::json as cjson;
use loom_sdk::commission::model::primitives::{Timestamp, Uuid};
use loom_sdk::commission::model::responsibility::{
    ActionRequestData, ActionRequestId, AgentRevisionId, AuthorityContext, AuthorityVerdict,
    CaseId, CommissionData, CommissionId, Observation, ObservationData, ObservationId, PrincipalId,
    ProposedActionArguments, RunId,
};
use loom_sdk::commission::ports::authority::AuthorityProvider;
use loom_sdk::commission::ports::evidence::ObservationPort;
use loom_sdk::commission::ports::governor::Governor;
use loom_sdk::governor::{CanonGovernor, FallibleCaseStore, MemoryCaseStore};
use takt::case::{
    AUTHORITY_FILE, Authority, AuthorityDecision, CaseAuthority, CaseDir, CaseFiles, Grant,
    OBSERVATIONS_FILE, grant, holds_grant,
};
use takt::evidence::Record;
use takt::forge::{Checks, Forge, Memory};
use takt::harness::{Harness, StepContext, StepReport};
use takt::validator::{CiRead, PrMerged, PrOpen, Validator};
use takt::{Decision, LoadedProtocol, Run, RunOptions, Stop};

/// The protocol under test, the engineering protocols' built-in.
const PROTOCOL: &str = "story.delivery/1";

fn git(tree: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(args)
        .current_dir(tree)
        .output()
        .expect("git runs");
    assert!(output.status.success(), "git {args:?}");
    String::from_utf8(output.stdout)
        .expect("utf-8")
        .trim()
        .to_owned()
}

/// A scratch git tree under this crate's build directory, removed when dropped.
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
        .join(format!("governor-{}-{name}", std::process::id()));
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

/// A harness the runs below never reach: every step they take is a validator's.
struct NoHarness;

impl Harness for NoHarness {
    fn name(&self) -> &str {
        "none"
    }

    fn step(&self, context: &StepContext<'_>) -> Result<StepReport, anyhow::Error> {
        Err(anyhow!(
            "no harness: a judgement step among {} is left",
            context.offered.join(", ")
        ))
    }
}

/// A harness that requests the merge gate as soon as it is offered.
struct Requester;

impl Harness for Requester {
    fn name(&self) -> &str {
        "requester"
    }

    fn step(&self, context: &StepContext<'_>) -> Result<StepReport, anyhow::Error> {
        if context.approvals.contains(&"repository.merge".to_owned()) {
            return Ok(StepReport {
                action: "repository.merge".to_owned(),
                summary: "the intent holds; merge it".to_owned(),
                tokens: None,
            });
        }
        Err(anyhow!(
            "the gate was not offered; a judgement step among {} is left",
            context.offered.join(", ")
        ))
    }
}

fn record(n: usize, kind: &str, result: Option<&str>, revision: &str, producer: &str) -> Record {
    Record::new(
        format!("e{n}"),
        kind,
        result.map(str::to_owned),
        "implementation",
        revision,
        producer,
    )
}

/// One commit on the case's branch, and the case's `implementation` moved to it.
fn implemented(tree: &Path, case: &mut CaseDir, name: &str) -> String {
    fs::write(tree.join(name), "done\n").expect("a file");
    git(tree, &["add", name]);
    git(tree, &["commit", "-q", "-m", "implemented by the test"]);
    let revision = git(tree, &["rev-parse", "HEAD"]);
    case.set_revision("implementation", &revision)
        .expect("the revision moves");
    revision
}

fn options(id: &str) -> RunOptions {
    RunOptions {
        protocol: PROTOCOL.to_owned(),
        intent: "deliver the story\n\nThe story's body.".to_owned(),
        inputs: Vec::new(),
        iterations: Some(8),
        hours: None,
        tokens: None,
        case_id: Some(id.to_owned()),
        gate: Some("repository.merge".to_owned()),
    }
}

fn validators(forge: Rc<dyn Forge>) -> Vec<Box<dyn Validator>> {
    vec![
        Box::new(PrOpen::new(forge.clone())),
        Box::new(CiRead::new(forge.clone())),
        Box::new(PrMerged::new(forge)),
    ]
}

fn action<'a>(decision: &'a Decision, id: &str) -> &'a str {
    decision
        .actions
        .get(id)
        .map(String::as_str)
        .unwrap_or("absent")
}

/// The first run of a case on the delivery protocol over `tree`, stopped at the merge gate: the
/// revision, the case directory.
fn first_run(tree: &Path, id: &str, memory: &Rc<Memory>) -> (String, PathBuf) {
    let forge: Rc<dyn Forge> = memory.clone();
    let validators = validators(forge);
    let mut run = Run::open(tree, options(id), &Requester, &validators).expect("the case opens");
    let revision = implemented(tree, &mut run.case, "implemented.txt");
    let records = [
        record(1, "test_result", Some("pass"), &revision, "merge-test"),
        record(2, "gate_result", Some("pass"), &revision, "merge-test"),
        record(3, "handover", None, &revision, takt::run::CHOICE_PRODUCER),
        record(
            4,
            "change_record",
            Some("ahead"),
            &revision,
            "takt.change-read",
        ),
    ];
    for record in &records {
        run.case
            .add_evidence(record)
            .expect("the record is written");
    }
    let mut out = Vec::new();
    let stop = run.run(&mut out).expect("the run stops without an error");
    let text = String::from_utf8(out).expect("utf-8");
    assert_eq!(
        stop,
        Stop::ApprovalRequired(vec!["repository.merge".to_owned()]),
        "{text}"
    );
    (revision, run.case.dir.clone())
}

fn memory() -> Rc<Memory> {
    Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    })
}

/// The decision files of the case, in sequence order.
fn decision_files(case_dir: &Path) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = fs::read_dir(case_dir.join("decisions"))
        .expect("a decisions directory")
        .map(|entry| entry.expect("an entry").path())
        .collect();
    paths.sort();
    paths
}

fn json_file(path: &Path) -> serde_json::Value {
    serde_json::from_str(&fs::read_to_string(path).expect("a file")).expect("json")
}

/// A governor over the case's files, its protocol registered from the case's `protocol.yaml`,
/// evaluating at `at`.
fn governor_at(
    case_dir: &Path,
    at: &str,
) -> CanonGovernor<
    CaseFiles,
    impl Fn(
        &CaseId,
    ) -> Result<Option<String>, loom_sdk::commission::model::responsibility::GovernorError>,
> {
    let files = CaseFiles::at(case_dir).expect("the case's files");
    let name = files.protocol().to_owned();
    let yaml = fs::read_to_string(case_dir.join("protocol.yaml")).expect("protocol.yaml");
    let at = at.to_owned();
    CanonGovernor::new(files)
        .with_protocol_yaml(&name, &yaml)
        .expect("the governor takes the case's protocol")
        .with_evaluation_time(move |_: &CaseId| Ok(Some(at.clone())))
}

/// The observations of a case a run took to its merge gate are what Commission's runtime of
/// loom-sdk recorded of its validator steps (`story:validators-perform-as-effects`): as many
/// proposals of the executor (`executor`) as outcomes of the effect (`effect`), and one
/// observation under the validator's name of each record the run's effects submitted, naming a
/// record in `evidence/` of that producer. The records written before the run have none.
fn assert_observed_by_the_runtime(case_dir: &Path, memory: &Rc<Memory>) {
    let observations = json_file(&case_dir.join(OBSERVATIONS_FILE));
    let observations = observations.as_array().expect("an array");
    let names: Vec<String> = validators(memory.clone())
        .iter()
        .map(|validator| validator.name().to_owned())
        .collect();
    let records: Vec<serde_json::Value> = fs::read_dir(case_dir.join("evidence"))
        .expect("evidence")
        .map(|entry| json_file(&entry.expect("an entry").path()))
        .collect();
    let of = |source: &str| {
        observations
            .iter()
            .filter(|observation| observation["source"] == source)
            .count()
    };
    assert!(of("executor") > 0, "{observations:?}");
    assert_eq!(of("executor"), of("effect"), "{observations:?}");
    let mut observed = Vec::new();
    for observation in observations {
        let source = observation["source"].as_str().expect("a source");
        if source == "executor" || source == "effect" {
            continue;
        }
        assert!(names.iter().any(|name| name == source), "{observation}");
        let id = observation["payload"]["record"].as_str().expect("a record");
        assert!(
            records
                .iter()
                .any(|record| record["id"] == id && record["producer"] == source),
            "{observation}"
        );
        observed.push(id.to_owned());
    }
    let mut by_the_run: Vec<String> = records
        .iter()
        .filter(|record| {
            record["producer"]
                .as_str()
                .is_some_and(|producer| names.iter().any(|name| name == producer))
        })
        .map(|record| record["id"].as_str().expect("an id").to_owned())
        .collect();
    observed.sort();
    by_the_run.sort();
    assert!(!by_the_run.is_empty());
    assert_eq!(observed, by_the_run, "{observations:?}");
}

/// Acceptance 1: a case run to its merge gate has `case.yaml`, its evidence, `decisions/` and
/// `observations.json` where takt writes them, and `authority.yaml` once granted. The latest
/// decision file is the document the governor renders for the case as it is held, at the instant
/// the file names: a governor over the same files gives it back.
#[test]
fn a_case_at_its_merge_gate_holds_its_files_and_the_governors_decision_documents() {
    let tree = scratch("files");
    let memory = memory();
    let (_, case_dir) = first_run(&tree, "files", &memory);
    assert!(case_dir.join("case.yaml").is_file());
    assert_eq!(
        fs::read_dir(case_dir.join("evidence"))
            .expect("evidence")
            .count(),
        6
    );
    let decisions = decision_files(&case_dir);
    assert!(!decisions.is_empty());
    for path in &decisions {
        let name = path.file_name().expect("a name").to_string_lossy();
        assert!(
            name.len() == "0001.json".len() && name.ends_with(".json"),
            "{name}"
        );
    }
    assert_observed_by_the_runtime(&case_dir, &memory);
    assert!(!case_dir.join(AUTHORITY_FILE).exists());

    let latest = json_file(decisions.last().expect("a decision"));
    assert_eq!(latest["format"], "canon-decision/1", "{latest}");
    let from = &latest["explanation"]["computed_from"];
    assert!(
        from["case"]["revision"]
            .as_str()
            .is_some_and(|revision| revision.starts_with('r')),
        "the governor's snapshot carries the case revision: {latest}"
    );
    assert!(
        from.get("authority").is_none(),
        "the governor supplies no authority: {latest}"
    );
    assert_eq!(
        latest["actions"]["repository.merge"]["status"],
        "approval-required"
    );
    let at = from["at"].as_str().expect("the instant decided at");
    let governor = governor_at(&case_dir, at);
    let case = CaseId("files".to_owned());
    let frontier = governor.frontier(&case).expect("a frontier");
    let document = governor
        .decision_document(&case, &frontier.data().frontier_id)
        .expect("the case is held")
        .expect("the document behind the frontier");
    let mut rendered = String::new();
    loom_sdk::governor::model::json::push_value(&mut rendered, &document);
    let rendered: serde_json::Value = serde_json::from_str(&rendered).expect("json");
    assert_eq!(rendered, latest);

    grant(&tree, "files", "repository.merge", "human:operator").expect("granted");
    assert!(case_dir.join(AUTHORITY_FILE).is_file());
}

/// The store reads the case from its files and writes back nothing it does not hold: an existing
/// case is not inserted again, a change to anything but its artifacts is refused with
/// `case.yaml` left as it was, and an observation is kept in `observations.json`, read back by
/// another store over the same files.
#[test]
fn the_case_store_reads_the_case_files_and_keeps_observations() {
    let tree = scratch("store");
    let memory = memory();
    let (revision, case_dir) = first_run(&tree, "store", &memory);
    let files = CaseFiles::at(&case_dir).expect("the case's files");
    let case = CaseId("store".to_owned());
    let state = files.get(&case).expect("reads").expect("held");
    assert_eq!(state.protocol, "story.delivery/1");
    assert_eq!(
        state.artifacts.get("implementation").map(String::as_str),
        Some(revision.as_str())
    );
    assert_eq!(state.evidence.len(), 6);
    assert!(state.evidence.iter().all(|held| held.applies));
    assert!(state.revision >= 1);
    assert!(
        files
            .get(&CaseId("another".to_owned()))
            .expect("reads")
            .is_none()
    );
    assert!(!files.insert(state.clone()).expect("no error"));

    let before = fs::read_to_string(case_dir.join("case.yaml")).expect("case.yaml");
    assert!(files.update(&case, |state| state.evidence.clear()).is_err());
    assert!(
        files
            .update(&case, |state| state.protocol = "other/1".to_owned())
            .is_err()
    );
    assert_eq!(
        fs::read_to_string(case_dir.join("case.yaml")).expect("case.yaml"),
        before
    );
    assert_eq!(files.update(&case, |_| 7).expect("no error"), Some(7));
    assert_eq!(
        files.get(&case).expect("reads").expect("held"),
        state,
        "an update that changes nothing leaves the case as it was"
    );

    let governor = governor_at(&case_dir, "2026-10-21T00:00:00Z");
    let held = governor.try_observations().expect("reads");
    let observation = ObservationData {
        observation_id: ObservationId(Uuid("00000000-0000-4000-8000-000000000001".to_owned())),
        source: "forge".to_owned(),
        subject: "pull_request".to_owned(),
        observed_at: Timestamp("2026-10-21T00:00:00Z".to_owned()),
        payload: cjson::parse(r#"{"number":1}"#).expect("json"),
    };
    governor
        .observe(Observation::new(observation.clone()))
        .expect("observed");
    let mut kept = held.clone();
    kept.push(observation.clone());
    assert_eq!(governor.try_observations().expect("reads"), kept);
    let again = CaseFiles::at(&case_dir).expect("the case's files");
    assert_eq!(again.observations().expect("reads"), kept);
    let written = json_file(&case_dir.join(OBSERVATIONS_FILE));
    assert_eq!(written[held.len()]["source"], "forge", "{written}");
}

fn commission(case: &str) -> CommissionData {
    CommissionData {
        commission_id: CommissionId(Uuid("00000000-0000-4000-8000-000000000002".to_owned())),
        agent_revision_id: AgentRevisionId(Uuid("00000000-0000-4000-8000-000000000003".to_owned())),
        case_id: CaseId(case.to_owned()),
        principal: PrincipalId("agent:developer".to_owned()),
        authority_context: AuthorityContext(cjson::Value::Object(Vec::new())),
    }
}

fn request(case: &str, action: &str, revision: i64) -> ActionRequestData {
    ActionRequestData {
        action_request_id: ActionRequestId(Uuid("00000000-0000-4000-8000-000000000004".to_owned())),
        run_id: RunId(Uuid("00000000-0000-4000-8000-000000000005".to_owned())),
        case_id: CaseId(case.to_owned()),
        expected_case_revision: revision,
        action: action.to_owned(),
        arguments: ProposedActionArguments(cjson::Value::Object(Vec::new())),
    }
}

fn verdict(case_dir: &Path, id: &str, revision: i64) -> AuthorityVerdict {
    CaseAuthority::new(case_dir, id)
        .decide(
            &commission(id),
            &request(id, "repository.merge", revision),
            "repository.merge",
        )
        .expect("the provider answers")
}

fn case_revision(case_dir: &Path, id: &str) -> i64 {
    CaseFiles::at(case_dir)
        .expect("the case's files")
        .get(&CaseId(id.to_owned()))
        .expect("reads")
        .expect("held")
        .revision
}

/// Acceptance 3: the provider allows `repository.merge` only at the revision of its subject the
/// grant was given at; the evaluation answers the merge admissible there while the governor's
/// document, which supplies no authority, holds it approval-required. Once `implementation`
/// moves, the grant authorises nothing, at the case revision the request expected and at the new
/// one, and the merge validator's gate (`holds_grant`) refuses it too.
#[test]
fn the_provider_allows_a_grant_only_at_the_revision_of_its_subject_it_was_given_at() {
    let tree = scratch("provider");
    let memory = memory();
    let (_, case_dir) = first_run(&tree, "provider", &memory);
    let at_gate = case_revision(&case_dir, "provider");
    assert!(matches!(
        verdict(&case_dir, "provider", at_gate),
        AuthorityVerdict::ApprovalRequired(_)
    ));
    grant(&tree, "provider", "repository.merge", "human:operator").expect("granted");
    assert_eq!(
        verdict(&case_dir, "provider", at_gate),
        AuthorityVerdict::Allow(loom_sdk::commission::model::responsibility::Unit(true))
    );
    // A request about another case is no request to this case's provider.
    assert!(
        CaseAuthority::new(&case_dir, "provider")
            .decide(
                &commission("provider"),
                &request("other", "repository.merge", at_gate),
                "repository.merge",
            )
            .is_err()
    );
    // An action that does not require the capability asked about is denied it.
    assert!(matches!(
        CaseAuthority::new(&case_dir, "provider")
            .decide(
                &commission("provider"),
                &request("provider", "pr.open", at_gate),
                "repository.merge",
            )
            .expect("answers"),
        AuthorityVerdict::Deny(_)
    ));
    let mut case = CaseDir::resume(&tree, "provider").expect("the case resumes");
    let seq = case.evaluations().expect("counts") + 1;
    let decision = case.evaluate(seq).expect("evaluates");
    assert_eq!(action(&decision, "repository.merge"), "admissible");
    let written = json_file(&case_dir.join("decisions").join(format!("{seq:04}.json")));
    assert_eq!(
        written["actions"]["repository.merge"]["status"], "approval-required",
        "{written}"
    );
    assert!(holds_grant(&case_dir, "repository.merge").expect("reads"));

    implemented(&tree, &mut case, "after-the-grant.txt");
    let moved = case_revision(&case_dir, "provider");
    assert!(moved > at_gate, "{moved} after {at_gate}");
    for revision in [at_gate, moved] {
        assert!(
            matches!(
                verdict(&case_dir, "provider", revision),
                AuthorityVerdict::ApprovalRequired(_)
            ),
            "at case revision {revision}"
        );
    }
    let decision = case
        .evaluate(case.evaluations().expect("counts") + 1)
        .expect("evaluates");
    assert_ne!(action(&decision, "repository.merge"), "admissible");
    assert!(!holds_grant(&case_dir, "repository.merge").expect("reads"));
}

/// A denial on file is the provider's denial at every revision, and the evaluation answers the
/// merge blocked.
#[test]
fn a_denial_on_file_is_denied_and_blocks_the_merge() {
    let tree = scratch("denied");
    let memory = memory();
    let (revision, case_dir) = first_run(&tree, "denied", &memory);
    Authority {
        decisions: vec![AuthorityDecision {
            capability: "repository.merge".to_owned(),
            decision: Grant::Denied,
            approver: "human:operator".to_owned(),
            granted_at: "2026-10-21T00:00:00Z".to_owned(),
            revision: Some(revision),
            mainline_tip: None,
        }],
    }
    .write(&case_dir)
    .expect("written");
    let at = case_revision(&case_dir, "denied");
    assert!(matches!(
        verdict(&case_dir, "denied", at),
        AuthorityVerdict::Deny(_)
    ));
    let case = CaseDir::resume(&tree, "denied").expect("the case resumes");
    let decision = case
        .evaluate(case.evaluations().expect("counts") + 1)
        .expect("evaluates");
    assert_eq!(action(&decision, "repository.merge"), "blocked");
}

/// A capability `authority.yaml` decides more than once is denied wherever one of its
/// decisions denies it, in whatever order they are on file: a denial appended after a grant at
/// the same revision denies the merge to the provider and the evaluation, the merge validator's
/// gate (`holds_grant`, the case's `grants`, `grants_at`) holds no grant, the merge validator
/// merges nothing, and `takt approve` grants nothing over it.
#[test]
fn a_denial_after_a_grant_still_denies() {
    let tree = scratch("dupdeny");
    let memory = memory();
    let (revision, case_dir) = first_run(&tree, "dupdeny", &memory);
    grant(&tree, "dupdeny", "repository.merge", "human:operator").expect("granted");
    let mut authority = Authority::read(&case_dir).expect("reads").expect("held");
    authority.decisions.push(AuthorityDecision {
        capability: "repository.merge".to_owned(),
        decision: Grant::Denied,
        approver: "human:operator".to_owned(),
        granted_at: "2026-10-21T00:00:00Z".to_owned(),
        revision: Some(revision.clone()),
        mainline_tip: None,
    });
    authority.write(&case_dir).expect("written");
    let at = case_revision(&case_dir, "dupdeny");
    let answer = verdict(&case_dir, "dupdeny", at);
    assert!(
        matches!(answer, AuthorityVerdict::Deny(_)),
        "verdict {answer:?}"
    );
    assert!(
        !authority.grants_at("repository.merge", &revision),
        "grants_at"
    );
    assert!(
        authority
            .decision("repository.merge")
            .is_some_and(|decision| decision.decision == Grant::Denied),
        "the decision on the capability"
    );
    let case = CaseDir::resume(&tree, "dupdeny").expect("the case resumes");
    let decision = case
        .evaluate(case.evaluations().expect("counts") + 1)
        .expect("evaluates");
    assert_eq!(action(&decision, "repository.merge"), "blocked");
    assert!(!case.grants("repository.merge").expect("reads"), "grants");
    drop(case);
    assert!(
        !holds_grant(&case_dir, "repository.merge").expect("reads"),
        "holds_grant"
    );
    let observation = PrMerged::new(memory.clone())
        .observe(&tree, &revision, 99)
        .expect("observes");
    assert!(observation.records.is_empty(), "{}", observation.summary);
    assert!(
        memory.merged.borrow().is_empty(),
        "merged: {}",
        observation.summary
    );
    let refused = grant(&tree, "dupdeny", "repository.merge", "human:operator")
        .expect_err("a grant over a denial is refused");
    assert!(
        format!("{refused:#}").contains("already decides repository.merge"),
        "{refused:#}"
    );
}

/// Acceptance 4: a case the last release left at its merge grant (no case revision in
/// `case.yaml`, no `observations.json`, each decision rendered by Canon directly) resumes after
/// the grant `takt approve` writes and merges.
#[test]
fn a_case_left_at_its_merge_grant_by_the_last_release_resumes_after_approve_and_merges() {
    let tree = scratch("released");
    let memory = memory();
    let (_, case_dir) = first_run(&tree, "released", &memory);
    // What the last release left: `case.yaml` without a case revision, no observations, and each
    // decision as Canon rendered it with no authority supplied.
    let snapshot = case_dir.join("case.yaml");
    let mut doc: serde_yaml_ng::Value =
        serde_yaml_ng::from_str(&fs::read_to_string(&snapshot).expect("case.yaml")).expect("yaml");
    doc.as_mapping_mut().expect("a mapping").remove("revision");
    let case_yaml = serde_yaml_ng::to_string(&doc).expect("yaml");
    fs::write(&snapshot, &case_yaml).expect("case.yaml");
    fs::remove_file(case_dir.join(OBSERVATIONS_FILE)).expect("removed");
    let protocol = LoadedProtocol::from_file(&case_dir.join("protocol.yaml")).expect("loads");
    let ir = b10x_canon::eval::read_ir(&protocol.ir_json).expect("ir");
    let case = b10x_canon::eval::read_case(&case_yaml).expect("case");
    let mut evidence: Vec<PathBuf> = fs::read_dir(case_dir.join("evidence"))
        .expect("evidence")
        .map(|entry| entry.expect("an entry").path())
        .collect();
    evidence.sort();
    let records: Vec<_> = evidence
        .iter()
        .map(|path| {
            b10x_canon::eval::read_evidence(&fs::read_to_string(path).expect("a record"))
                .expect("a record")
        })
        .collect();
    for path in decision_files(&case_dir) {
        let at = json_file(&path)["explanation"]["computed_from"]["at"]
            .as_str()
            .expect("an instant")
            .to_owned();
        let decision = b10x_canon::eval::evaluate_with(
            &ir,
            &case,
            &records,
            b10x_canon::eval::Supplied {
                authority: None,
                at: Some(&at),
                decisions: None,
            },
        )
        .expect("decides");
        fs::write(&path, b10x_canon::eval::render(&decision)).expect("written");
    }

    grant(&tree, "released", "repository.merge", "human:operator").expect("granted");
    let forge: Rc<dyn Forge> = memory.clone();
    let validators = validators(forge);
    let mut run = Run::resume(
        &tree,
        options("released"),
        "released",
        &NoHarness,
        &validators,
    )
    .expect("the case resumes");
    let mut out = Vec::new();
    let stop = run.run(&mut out).expect("the run stops without an error");
    let text = String::from_utf8(out).expect("utf-8");
    assert_eq!(stop, Stop::Completed("accepted".to_owned()), "{text}");
    assert_eq!(memory.merged.borrow().len(), 1, "{text}");
}

/// The governor admits one capability per action. Each action of takt's protocols that requires
/// a capability is named here with the one it requires, and the governor takes each protocol.
#[test]
fn every_takt_protocol_action_requires_one_capability_and_the_governor_takes_it() {
    let expected: &[(&str, &[(&str, &str)])] = &[
        (
            "story.delivery/1",
            &[("repository.merge", "repository.merge")],
        ),
        (
            "self.improvement/1",
            &[
                ("binary.promote", "takt.promote"),
                ("candidate.merge", "repository.merge"),
                (
                    "candidate.merge_judge_surface",
                    "repository.merge.judge_surface",
                ),
            ],
        ),
        (
            "loop.improvement/1",
            &[("loop.merge", "repository.merge.judge_surface")],
        ),
    ];
    for (id, actions) in expected {
        let protocol = LoadedProtocol::builtin(id).expect("the protocol");
        let doc: serde_yaml_ng::Value = serde_yaml_ng::from_str(&protocol.yaml).expect("yaml");
        let mut found: Vec<(String, Vec<String>)> = doc["actions"]
            .as_mapping()
            .expect("actions")
            .iter()
            .filter_map(|(name, action)| {
                let capabilities: Vec<String> = action["requires"]
                    .as_sequence()?
                    .iter()
                    .filter_map(|requirement| requirement["capability"].as_str())
                    .map(str::to_owned)
                    .collect();
                (!capabilities.is_empty())
                    .then(|| (name.as_str().expect("a name").to_owned(), capabilities))
            })
            .collect();
        found.sort();
        let named: Vec<(String, Vec<String>)> = actions
            .iter()
            .map(|(action, capability)| ((*action).to_owned(), vec![(*capability).to_owned()]))
            .collect();
        assert_eq!(found, named, "{id}");
        CanonGovernor::new(MemoryCaseStore::default())
            .with_protocol_yaml(&protocol.id, &protocol.yaml)
            .unwrap_or_else(|error| panic!("{id}: {error:?}"));
    }
}
