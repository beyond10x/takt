//! story:validators-perform-as-effects: each validator is an `EffectPort` the runtime of loom-sdk
//! hands an admitted request, and the evidence it writes reaches the case only through
//! `submit_evidence`, with the validator named as producer. A choice record is written by an
//! effect too, under `takt.choice`. With an authority provider that allows every capability, the
//! merge effect still merges only under a grant of `repository.merge` at the case's revision.

use std::cell::RefCell;
use std::collections::{BTreeSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

use anyhow::anyhow;
use loom_sdk::EffectPort;
use loom_sdk::commission::model::json as cjson;
use loom_sdk::commission::model::primitives::Uuid;
use loom_sdk::commission::model::responsibility::{
    ActionRequestData, AuthorityVerdict, CaseId, CommissionData, EffectOutcome, EvidenceData,
    EvidenceId, ObservationId, Unit,
};
use loom_sdk::commission::ports::authority::{AuthorityProvider, AuthorityProviderError};
use loom_sdk::commission::ports::effect::AdmittedRequest;
use loom_sdk::commission::ports::evidence::{EvidenceError, submit_evidence};
use takt::case::{CASES_DIR, CaseAuthority, CaseDir, grant};
use takt::evidence::{CaseEvidence, Record};
use takt::forge::{Checks, Forge, Memory};
use takt::harness::{Harness, StepContext, StepReport};
use takt::improvement::{
    BinaryPromote, CandidateMerged, Checking, FindingFile, Mainline, ScenarioReproduce,
    ScenarioVerify, ScorecardCompare,
};
use takt::loop_scorecard::{LoopCompare, LoopDefinitionCheck, LoopMeasure};
use takt::run::CHOICE_PRODUCER;
use takt::validator::{
    BUILT_IN, ChangeRead, CiRead, GateRun, Observation, PrMerged, PrOpen, Step, TestsRun,
    Validator, ValidatorEffect, is_built_in_action, outcome, run_effect,
};
use takt::{LoadedProtocol, Run, RunOptions, Stop};

fn git(tree: &Path, args: &[&str]) -> String {
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
    String::from_utf8(output.stdout)
        .expect("utf-8")
        .trim()
        .to_owned()
}

struct Scratch(PathBuf);

impl std::ops::Deref for Scratch {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// A scratch tree with one commit on `main`.
fn scratch(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("effects-{}-{name}", std::process::id()));
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

fn options(id: &str) -> RunOptions {
    RunOptions {
        protocol: "story.delivery/1".to_owned(),
        intent: "remove a todo by its number".to_owned(),
        inputs: Vec::new(),
        iterations: Some(40),
        hours: None,
        tokens: None,
        case_id: Some(id.to_owned()),
        gate: Some("repository.merge".to_owned()),
    }
}

/// A validator that observes `pass` for its action at every revision, under its own name.
struct Green {
    name: &'static str,
    action: &'static str,
    kind: &'static str,
}

impl Validator for Green {
    fn name(&self) -> &str {
        self.name
    }

    fn action(&self) -> &str {
        self.action
    }

    fn kind(&self) -> &str {
        self.kind
    }

    fn subject(&self) -> &str {
        "implementation"
    }

    fn observe(&self, _: &Path, revision: &str, seq: usize) -> anyhow::Result<Observation> {
        Ok(Observation {
            summary: "pass".to_owned(),
            artifact: None,
            records: vec![Record::new(
                format!("{}-{seq}", self.kind),
                self.kind,
                Some("pass".to_owned()),
                self.subject(),
                revision,
                self.name(),
            )],
        })
    }
}

/// Edits the implementation, then writes the tests, one step each; then requests the merge when
/// it is offered, and otherwise hands the change over by taking `change.handover`.
struct Developer {
    edits: RefCell<VecDeque<(&'static str, &'static str)>>,
}

impl Developer {
    fn new() -> Self {
        Self {
            edits: RefCell::new(VecDeque::from([
                ("implementation.edit", "src/remove.txt"),
                ("tests.write", "tests/remove_test.txt"),
            ])),
        }
    }
}

impl Harness for Developer {
    fn name(&self) -> &str {
        "developer"
    }

    fn step(&self, context: &StepContext<'_>) -> anyhow::Result<StepReport> {
        let next = self.edits.borrow_mut().pop_front();
        if let Some((action, path)) = next {
            let at = context.tree.join(path);
            fs::create_dir_all(at.parent().expect("a parent")).expect("a directory");
            fs::write(&at, format!("{action}\n")).expect("a file");
            return Ok(StepReport {
                action: action.to_owned(),
                summary: format!("wrote {path}"),
                tokens: Some(100),
            });
        }
        let action = if context.approvals.contains(&"repository.merge".to_owned()) {
            "repository.merge"
        } else {
            "change.handover"
        };
        Ok(StepReport {
            action: action.to_owned(),
            summary: "the change is complete".to_owned(),
            tokens: Some(100),
        })
    }
}

/// A harness the run after the grant never reaches: every step left is a validator's.
struct NoHarness;

impl Harness for NoHarness {
    fn name(&self) -> &str {
        "none"
    }

    fn step(&self, context: &StepContext<'_>) -> anyhow::Result<StepReport> {
        Err(anyhow!(
            "no harness: a judgement step among {} is left",
            context.offered.join(", ")
        ))
    }
}

/// A step that is never taken: the effects below are only asked what they perform.
struct Never;

impl Step for Never {
    fn perform(
        &mut self,
        validator: &dyn Validator,
        _: &AdmittedRequest,
    ) -> anyhow::Result<EffectOutcome> {
        panic!("{} was invoked", validator.action())
    }
}

/// The validator's observation at `revision`, with no step guard around it, its records
/// submitted to the case through `evidence`.
struct Observes<'e, 'c> {
    tree: &'e Path,
    revision: String,
    evidence: &'e CaseEvidence<'c>,
}

impl Step for Observes<'_, '_> {
    fn perform(
        &mut self,
        validator: &dyn Validator,
        request: &AdmittedRequest,
    ) -> anyhow::Result<EffectOutcome> {
        let (observation, waits) = validator.observe_waiting(self.tree, &self.revision, 1)?;
        for record in &observation.records {
            self.evidence.submit(
                validator.name(),
                record,
                Some(&request.data().action_request_id.0.0),
            )?;
        }
        Ok(outcome(&observation, waits))
    }
}

/// An authority provider that allows every capability to every request.
struct AllowAll;

impl AuthorityProvider for AllowAll {
    fn decide(
        &self,
        _: &CommissionData,
        _: &ActionRequestData,
        _: &str,
    ) -> Result<AuthorityVerdict, AuthorityProviderError> {
        Ok(AuthorityVerdict::Allow(Unit(true)))
    }
}

/// One validator of each built-in type, in the order of [`BUILT_IN`].
fn built_in(forge: &Rc<dyn Forge>) -> Vec<Box<dyn Validator>> {
    let cache = Path::new(env!("CARGO_TARGET_TMPDIR")).join("effects-check-finding");
    vec![
        Box::new(TestsRun::new("true").expect("a command")),
        Box::new(GateRun::new(".engineering/gates.yaml")),
        Box::new(PrOpen::new(forge.clone())),
        Box::new(CiRead::new(forge.clone())),
        Box::new(PrMerged::new(forge.clone())),
        Box::new(ChangeRead),
        Box::new(LoopMeasure),
        Box::new(LoopDefinitionCheck),
        Box::new(LoopCompare),
        Box::new(FindingFile(Checking::new(cache.clone()))),
        Box::new(ScenarioReproduce(Checking::new(cache.clone()))),
        Box::new(ScenarioVerify(Checking::new(cache))),
        Box::new(ScorecardCompare::from_env()),
        Box::new(CandidateMerged::new(Mainline::Local)),
        Box::new(CandidateMerged::judge_surface(Mainline::Local)),
        Box::new(BinaryPromote::from_env()),
    ]
}

/// The records under the case's `evidence/`, in file order, each with its file name.
fn records(case_dir: &Path) -> Vec<(String, serde_json::Value)> {
    let mut paths: Vec<PathBuf> = fs::read_dir(case_dir.join("evidence"))
        .expect("an evidence directory")
        .map(|entry| entry.expect("an entry").path())
        .collect();
    paths.sort();
    paths
        .iter()
        .map(|path| {
            (
                path.file_name()
                    .expect("a name")
                    .to_string_lossy()
                    .into_owned(),
                serde_json::from_str(&fs::read_to_string(path).expect("a record")).expect("json"),
            )
        })
        .collect()
}

/// The observations the case received, in order.
fn observations(case_dir: &Path) -> Vec<serde_json::Value> {
    let path = case_dir.join("observations.json");
    let text = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    serde_json::from_str(&text).expect("json")
}

/// The validator steps of `action` the run printed.
fn validator_steps(text: &str, action: &str) -> usize {
    text.lines()
        .filter(|line| line.starts_with("step ") && line.ends_with(&format!(": {action} {{}}")))
        .count()
}

/// Acceptance 1: the actions each of takt's protocols gives a validator are the actions of
/// takt's built-in validators it declares, and each is performed by the effect of the built-in
/// validator for it, which performs that action and no other.
#[test]
fn every_action_a_protocol_gives_a_validator_is_performed_by_its_validators_effect() {
    let expected: [(&str, &[&str]); 3] = [
        (
            "story.delivery/1",
            &[
                "change.read",
                "tests.run",
                "gate.run",
                "pr.open",
                "ci.read",
                "repository.merge",
            ],
        ),
        (
            "self.improvement/1",
            &[
                "finding.file",
                "scenario.reproduce",
                "scenario.verify",
                "candidate.merge",
                "candidate.merge_judge_surface",
                "scorecard.compare",
                "binary.promote",
            ],
        ),
        (
            "loop.improvement/1",
            &["loop.measure", "loop.definition.check", "loop.compare"],
        ),
    ];
    let forge: Rc<dyn Forge> = Rc::new(Memory::default());
    let validators = built_in(&forge);
    for (id, actions) in expected {
        let protocol = LoadedProtocol::builtin(id).expect("reads");
        let given: BTreeSet<&str> = protocol
            .actions
            .iter()
            .map(String::as_str)
            .filter(|action| is_built_in_action(action))
            .collect();
        assert_eq!(
            given,
            actions.iter().copied().collect::<BTreeSet<_>>(),
            "{id}"
        );
        for action in actions {
            let validator = validators
                .iter()
                .find(|validator| validator.action() == *action)
                .unwrap_or_else(|| panic!("no built-in validator performs {action}"));
            let mut never = Never;
            let effect = ValidatorEffect::new(validator.as_ref(), &mut never);
            assert!(effect.performs(action), "{id}: {action}");
        }
    }
}

/// Acceptance 1: one effect per built-in validator, each performing its validator's action and
/// no other, and together every action [`BUILT_IN`] lists.
#[test]
fn each_validators_effect_performs_its_one_action() {
    let forge: Rc<dyn Forge> = Rc::new(Memory::default());
    let validators = built_in(&forge);
    assert_eq!(validators.len(), BUILT_IN.len());
    let mut performed = BTreeSet::new();
    for (validator, built_in) in validators.iter().zip(BUILT_IN) {
        assert_eq!(
            validator.action(),
            built_in.action,
            "{}",
            built_in.validator
        );
        let mut never = Never;
        let effect = ValidatorEffect::new(validator.as_ref(), &mut never);
        for other in BUILT_IN {
            assert_eq!(
                effect.performs(other.action),
                other.action == built_in.action,
                "the effect of {} asked about {}",
                validator.name(),
                other.action
            );
        }
        assert!(!effect.performs("implementation.edit"));
        assert!(!effect.performs("change.handover"));
        performed.insert(validator.action().to_owned());
    }
    let listed: BTreeSet<String> = BUILT_IN.iter().map(|b| b.action.to_owned()).collect();
    assert_eq!(performed, listed);
}

/// Acceptances 1 and 2 over a whole delivery run, through the gate and the merge: every
/// validator step the run took was handed to its validator's effect by the runtime (one
/// `effect` observation of its action each), and every record under the case names a validator
/// of the run or `takt.choice` as producer, was received citing the observation its producer
/// made of it, and was written by the effect of the request that observation names.
#[test]
fn a_delivery_run_performs_its_validator_steps_as_effects_and_writes_records_only_through_them() {
    let tree = scratch("delivery");
    let memory = Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    });
    let forge: Rc<dyn Forge> = memory.clone();
    let validators: Vec<Box<dyn Validator>> = vec![
        Box::new(Green {
            name: "green.tests-run",
            action: "tests.run",
            kind: "test_result",
        }),
        Box::new(Green {
            name: "green.gate-run",
            action: "gate.run",
            kind: "gate_result",
        }),
        Box::new(PrOpen::new(forge.clone())),
        Box::new(CiRead::new(forge.clone())),
        Box::new(ChangeRead),
        Box::new(PrMerged::new(forge)),
    ];
    let developer = Developer::new();
    let mut text = String::new();
    {
        let mut run =
            Run::open(&tree, options("delivery"), &developer, &validators).expect("the case opens");
        let mut out = Vec::new();
        let stop = run.run(&mut out).map_err(|error| format!("{error:#}"));
        text.push_str(&String::from_utf8(out).expect("utf-8"));
        assert_eq!(
            stop,
            Ok(Stop::ApprovalRequired(vec!["repository.merge".to_owned()])),
            "{text}"
        );
    }
    grant(&tree, "delivery", "repository.merge", "human:operator").expect("the grant is written");
    {
        let mut run = Run::resume(
            &tree,
            options("delivery"),
            "delivery",
            &NoHarness,
            &validators,
        )
        .expect("the case resumes");
        let mut out = Vec::new();
        let stop = run.run(&mut out).map_err(|error| format!("{error:#}"));
        text.push_str(&String::from_utf8(out).expect("utf-8"));
        assert_eq!(stop, Ok(Stop::Completed("accepted".to_owned())), "{text}");
    }
    assert!(
        memory.calls.borrow().iter().any(|call| call == "merge #1"),
        "{:?}",
        memory.calls.borrow()
    );

    let case_dir = tree.join(CASES_DIR).join("delivery");
    let observed = observations(&case_dir);
    let effects: Vec<&serde_json::Value> = observed
        .iter()
        .filter(|observation| observation["source"] == "effect")
        .collect();
    // Acceptance 1: each validator step the run printed was handed to its effect, once.
    for validator in &validators {
        let action = validator.action();
        let handed = effects
            .iter()
            .filter(|effect| effect["payload"]["action"] == action)
            .count();
        assert_eq!(
            handed,
            validator_steps(&text, action),
            "{action}: the effects observed {effects:#?}\n{text}"
        );
        assert!(handed > 0, "{action} was never taken\n{text}");
    }
    // Acceptance 2: every record's producer, its observation and the effect that wrote it.
    let producers: BTreeSet<&str> = validators
        .iter()
        .map(|validator| validator.name())
        .chain([CHOICE_PRODUCER])
        .collect();
    let written = records(&case_dir);
    assert!(
        written
            .iter()
            .any(|(_, record)| record["producer"] == CHOICE_PRODUCER),
        "no choice record: {written:#?}\n{text}"
    );
    assert!(
        written
            .iter()
            .any(|(_, record)| record["kind"] == "merge_observation"),
        "{written:#?}\n{text}"
    );
    for (file, record) in &written {
        let producer = record["producer"].as_str().expect("a producer");
        assert!(
            producers.contains(producer),
            "{file} names {producer}, not a validator of the run or {CHOICE_PRODUCER}"
        );
        let observation = observed
            .iter()
            .find(|observation| {
                observation["source"] == producer
                    && observation["payload"]["record"] == record["id"]
            })
            .unwrap_or_else(|| panic!("{file}: no observation by {producer} of it: {observed:#?}"));
        assert_eq!(observation["observed_at"], record["observed_at"], "{file}");
        let request = &observation["payload"]["action_request"];
        let effect = effects
            .iter()
            .find(|effect| &effect["payload"]["action_request"] == request)
            .unwrap_or_else(|| panic!("{file}: written by no effect: {observation:#?}"));
        assert_eq!(effect["payload"]["outcome"], "Performed", "{file}");
        let action = effect["payload"]["action"].as_str().expect("an action");
        if producer == CHOICE_PRODUCER {
            assert_eq!(action, "change.handover", "{file}");
        } else {
            let validator = validators
                .iter()
                .find(|validator| validator.name() == producer)
                .expect("a validator of the run");
            assert_eq!(action, validator.action(), "{file}");
        }
    }
}

/// Acceptance 2: the case's evidence port takes a record only from a producer of the run, only
/// citing an observation the case received at the record's `observed_at`; any other is refused
/// with nothing written. The producer the record is written under is the one submitted, never
/// the one the record names.
#[test]
fn the_case_evidence_port_takes_records_only_from_the_runs_producers() {
    let tree = scratch("port");
    let validators: Vec<Box<dyn Validator>> = vec![Box::new(Green {
        name: "green.tests-run",
        action: "tests.run",
        kind: "test_result",
    })];
    drop(Run::open(&tree, options("port"), &NoHarness, &validators).expect("the case opens"));
    let mut case = CaseDir::resume(&tree, "port").expect("the case reads back");
    let revision = case
        .revision("implementation")
        .expect("implementation has a revision");
    let evidence_dir = case.dir.join("evidence");
    let files = |dir: &Path| -> Vec<PathBuf> {
        let mut paths: Vec<PathBuf> = fs::read_dir(dir)
            .expect("an evidence directory")
            .map(|entry| entry.expect("an entry").path())
            .collect();
        paths.sort();
        paths
    };
    let before = files(&evidence_dir);
    let producers = vec!["green.tests-run".to_owned(), CHOICE_PRODUCER.to_owned()];
    let record = Record::new(
        "forged-1",
        "test_result",
        Some("pass".to_owned()),
        "implementation",
        &revision,
        "harness",
    );
    {
        let evidence = CaseEvidence::new(&mut case, producers.clone()).expect("the port opens");
        let refused = evidence
            .submit("harness", &record, None)
            .expect_err("a producer outside the run's is refused");
        assert!(format!("{refused:#}").contains("harness"), "{refused:#}");
        let data = EvidenceData {
            evidence_id: EvidenceId(Uuid("00000000-0000-8000-8000-000000000001".to_owned())),
            case_id: CaseId("port".to_owned()),
            kind: "test_result".to_owned(),
            subject_revision: 0,
            producer: String::new(),
            observation_ids: Vec::new(),
            facts: record.facts().expect("the record as facts"),
            provenance: cjson::Value::Null,
        };
        assert!(
            matches!(
                submit_evidence(&evidence, "green.tests-run", data.clone()),
                Err(EvidenceError::NoObservation)
            ),
            "a record citing no observation"
        );
        let unseen = EvidenceData {
            observation_ids: vec![ObservationId(Uuid(
                "00000000-0000-8000-8000-000000000002".to_owned(),
            ))],
            ..data
        };
        assert!(
            matches!(
                submit_evidence(&evidence, "green.tests-run", unseen),
                Err(EvidenceError::Governor(_))
            ),
            "a record citing an observation the case never received"
        );
    }
    assert_eq!(files(&evidence_dir), before, "a refused record was written");

    let evidence = CaseEvidence::new(&mut case, producers).expect("the port opens");
    let path = evidence
        .submit("green.tests-run", &record, None)
        .expect("a producer of the run is taken");
    let written: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).expect("the record")).expect("json");
    assert_eq!(written["producer"], "green.tests-run");
    assert_eq!(written["id"], "forged-1");
    assert_eq!(written["subject_revision"], revision.as_str());
}

/// Whether the line of code `code` may write a record into a case or names a path into a case's
/// `evidence/`: a call of `add_evidence(` or of `rebuild_case(` (replay's copy of a case), the
/// name `"evidence"` given to a path (`join("evidence")`, `Path::new("evidence")`) or bound to a
/// name, or a path literal through `evidence/` (`"evidence/…"`, `…/evidence/…`). The planning
/// store's `.engineering/evidence/` is another directory, no case's, and is left out.
fn names_case_evidence(code: &str) -> bool {
    let code = code.replace(".engineering/evidence/", "");
    code.contains("add_evidence(")
        || code.contains("rebuild_case(")
        || code.contains("(\"evidence\")")
        || code.contains(": &str = \"evidence\"")
        || code.contains(" = \"evidence\";")
        || code.contains("\"evidence/")
        || code.contains("/evidence/")
}

/// The lines of the code of every crate under `crates` (`<crate>/src/**/*.rs`) that may write a
/// record into a case or name a path into a case's `evidence/` ([`names_case_evidence`]), each
/// trimmed, with its file relative to `crates`, in order; comment lines are skipped.
fn evidence_lines(crates: &Path) -> Vec<(String, String)> {
    let mut lines = Vec::new();
    let mut dirs: Vec<PathBuf> = fs::read_dir(crates)
        .expect("the crates directory")
        .map(|entry| entry.expect("an entry").path().join("src"))
        .filter(|src| src.is_dir())
        .collect();
    while let Some(dir) = dirs.pop() {
        for entry in fs::read_dir(&dir).expect("a source directory") {
            let path = entry.expect("an entry").path();
            if path.is_dir() {
                dirs.push(path);
                continue;
            }
            if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }
            let text = fs::read_to_string(&path).expect("a source file");
            for line in text.lines() {
                let code = line.trim();
                if code.starts_with("//") || !names_case_evidence(code) {
                    continue;
                }
                let file = path.strip_prefix(crates).expect("under crates").display();
                lines.push((file.to_string(), code.to_owned()));
            }
        }
    }
    lines.sort();
    lines
}

/// The guard below finds a record written past the port in any crate, by `add_evidence` or by a
/// plain write into a case's `evidence/`: two such lines in a scratch copy of the crates' layout
/// are both found.
#[test]
fn the_port_guard_finds_a_record_written_past_it_in_any_crate() {
    let crates = scratch("port-guard");
    for (file, code) in [
        ("takt/src/lib.rs", "case.add_evidence(&record)?;"),
        (
            "takt-cli/src/main.rs",
            "std::fs::write(case.join(\"evidence\").join(\"0001-x.json\"), record)?;",
        ),
    ] {
        let path = crates.join(file);
        fs::create_dir_all(path.parent().expect("a parent")).expect("a source directory");
        fs::write(&path, format!("fn f() {{\n    {code}\n}}\n")).expect("a source file");
    }
    assert_eq!(
        evidence_lines(&crates),
        vec![
            (
                "takt-cli/src/main.rs".to_owned(),
                "std::fs::write(case.join(\"evidence\").join(\"0001-x.json\"), record)?;"
                    .to_owned()
            ),
            (
                "takt/src/lib.rs".to_owned(),
                "case.add_evidence(&record)?;".to_owned()
            ),
        ]
    );
}

/// Every line of takt's crates that may write a record into a case or names a path into a
/// case's `evidence/` ([`evidence_lines`]), with what it does: the port's one write, and
/// otherwise a read, a name, the empty directory a case opens with, or replay's copy into a
/// scratch tree. A line found and not here, or here and not found, fails the guard.
const EVIDENCE_LINES: [(&str, &str, &str); 17] = [
    (
        "takt/src/evidence.rs",
        "match self.case.borrow_mut().add_evidence(&record) {",
        "the port, `CaseEvidence`: the one write of a record into a case",
    ),
    (
        "takt/src/case.rs",
        "pub fn add_evidence(&mut self, record: &Record) -> Result<PathBuf> {",
        "the write the port calls",
    ),
    (
        "takt/src/case.rs",
        ".join(\"evidence\")",
        "the path `add_evidence` writes the record at",
    ),
    (
        "takt/src/case.rs",
        "std::fs::create_dir_all(self.dir.join(\"evidence\"))",
        "the empty directory a case is opened with",
    ),
    (
        "takt/src/case.rs",
        "let evidence = case_dir.join(\"evidence\");",
        "reads the records (`evidence_files`)",
    ),
    (
        "takt/src/case.rs",
        "evidence_id: EvidenceId(Uuid(uuid_from(&format!(\"{case}/evidence/{name}\")))),",
        "a record's id, derived from its name; nothing written",
    ),
    (
        "takt/src/case.rs",
        "cjson::Value::Text(format!(\"evidence/{name}\")),",
        "names a record the case holds; nothing written",
    ),
    (
        "takt/src/evidence.rs",
        "evidence_id: EvidenceId(Uuid(uuid_from(&format!(\"{id}/evidence/{}\", record.id)))),",
        "a record's id, derived from its name; nothing written",
    ),
    (
        "takt/src/loop_scorecard.rs",
        "let dir = crate::case::current(tree)?.join(\"evidence\");",
        "reads the records (`case_records`)",
    ),
    (
        "takt/src/run.rs",
        "let mine = Path::new(CASES_DIR).join(&self.case.id).join(\"evidence\");",
        "a path compared with a record line's; nothing written",
    ),
    (
        "takt/src/replay.rs",
        "let (_, name) = path.rsplit_once(\"/evidence/\")?;",
        "reads a record's name from the record's line",
    ),
    (
        "takt/src/replay.rs",
        "pub fn rebuild_case(start: &Start, tree: &Path, revision: &str) -> Result<PathBuf> {",
        "replay's copy of a case into a scratch tree, defined",
    ),
    (
        "takt/src/replay.rs",
        "let case_dir = rebuild_case(start, &tree, &revision)?;",
        "its one call, by `replay_once`, into the scratch worktree it made at `<dir>/tree`",
    ),
    (
        "takt/src/replay.rs",
        "plain_dir(&dir.join(\"evidence\"))?;",
        "the scratch case's directory, in the scratch tree",
    ),
    (
        "takt/src/replay.rs",
        "let from = start.case_dir.join(\"evidence\").join(name);",
        "reads a record of the case, never writes it",
    ),
    (
        "takt/src/replay.rs",
        "write_plain(&dir.join(\"evidence\").join(name), &bytes)?;",
        "copies that record, byte for byte, into the scratch case only",
    ),
    (
        "takt/src/validator.rs",
        "let dir = case_dir.join(\"evidence\");",
        "reads the case's records for the pull request's body (story:the-pull-request-and-the-store-carry-the-evidence)",
    ),
];

/// Acceptance 2, for every run: no code of takt writes a record into a case but the case's
/// evidence port. In every crate's source, `CaseDir::add_evidence` is called from `evidence.rs`
/// alone, and every other line that names a path into a case's `evidence/` reads, names, opens
/// the empty directory, or copies a record into replay's scratch tree ([`EVIDENCE_LINES`]).
#[test]
fn no_record_reaches_a_case_but_through_the_case_evidence_port() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the crates directory");
    let mut found = evidence_lines(crates);
    found.sort();
    let mut known: Vec<(String, String)> = EVIDENCE_LINES
        .iter()
        .map(|(file, code, _)| ((*file).to_owned(), (*code).to_owned()))
        .collect();
    known.sort();
    let unknown: Vec<&(String, String)> =
        found.iter().filter(|line| !known.contains(line)).collect();
    assert!(
        unknown.is_empty(),
        "lines that may write a record into a case past its evidence port: {unknown:#?}"
    );
    let gone: Vec<&(String, String)> = known.iter().filter(|line| !found.contains(line)).collect();
    assert!(
        gone.is_empty(),
        "lines EVIDENCE_LINES names that are no longer in the code: {gone:#?}"
    );
    assert_eq!(found, known, "each line as often as it is named");
}

/// Brings a case on the delivery protocol to the gate over the forge double: the tests and the
/// gate green at a new implementation revision, the change handed over and read, the pull
/// request opened and its checks read. Answers the case's directory and the revision.
fn to_the_gate(tree: &Path, id: &str, memory: &Rc<Memory>) -> (PathBuf, String) {
    let forge: Rc<dyn Forge> = memory.clone();
    let validators: Vec<Box<dyn Validator>> = vec![
        Box::new(Green {
            name: "green.tests-run",
            action: "tests.run",
            kind: "test_result",
        }),
        Box::new(Green {
            name: "green.gate-run",
            action: "gate.run",
            kind: "gate_result",
        }),
        Box::new(PrOpen::new(forge.clone())),
        Box::new(CiRead::new(forge.clone())),
        Box::new(ChangeRead),
        Box::new(PrMerged::new(forge)),
    ];
    let developer = Developer::new();
    let mut run = Run::open(tree, options(id), &developer, &validators).expect("the case opens");
    let mut out = Vec::new();
    let stop = run.run(&mut out).map_err(|error| format!("{error:#}"));
    let text = String::from_utf8(out).expect("utf-8");
    assert_eq!(
        stop,
        Ok(Stop::ApprovalRequired(vec!["repository.merge".to_owned()])),
        "{text}"
    );
    let revision = run
        .case
        .revision("implementation")
        .expect("implementation has a revision");
    (run.case.dir.clone(), revision)
}

/// Acceptance 3: with an authority provider that allows every capability, the runtime hands the
/// merge effect a request for `repository.merge` the case holds no grant of, and the effect still
/// refuses: the forge is asked for no merge and no `merge_observation` is written. The case's
/// own authority does not hand it the request at all; under the operator's grant at the case's
/// revision it merges.
#[test]
fn with_an_authority_that_allows_everything_the_merge_effect_still_merges_only_under_a_grant() {
    let tree = scratch("allow-all");
    let memory = Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    });
    let (case_dir, revision) = to_the_gate(&tree, "allow-all", &memory);
    let forge: Rc<dyn Forge> = memory.clone();
    let merged = PrMerged::new(forge);
    let producers = vec![merged.name().to_owned()];
    let merges = || {
        memory
            .calls
            .borrow()
            .iter()
            .filter(|call| call.starts_with("merge "))
            .count()
    };
    let merge_records = || {
        records(&case_dir)
            .iter()
            .filter(|(_, record)| record["kind"] == "merge_observation")
            .count()
    };

    let mut case = CaseDir::resume(&tree, "allow-all").expect("the case reads back");
    let protocol = case.protocol.clone();
    {
        let evidence = CaseEvidence::new(&mut case, producers.clone()).expect("the port opens");
        let mut step = Observes {
            tree: &tree,
            revision: revision.clone(),
            evidence: &evidence,
        };
        let effect = ValidatorEffect::new(&merged, &mut step);
        let end = run_effect(
            &case_dir,
            "allow-all",
            &protocol,
            &AllowAll,
            &effect,
            PrMerged::ACTION,
            "allow-all-1",
        )
        .expect("the runtime ends");
        assert!(
            matches!(
                end.effects.as_slice(),
                [EffectOutcome::Refused(refused)] if !refused.ends_run
            ),
            "{:?}",
            end.effects
        );
    }
    assert_eq!(merges(), 0, "{:?}", memory.calls.borrow());
    assert_eq!(merge_records(), 0);

    // The case's own authority: no grant on file, so the request is never handed to the effect.
    {
        let evidence = CaseEvidence::new(&mut case, producers.clone()).expect("the port opens");
        let mut step = Observes {
            tree: &tree,
            revision: revision.clone(),
            evidence: &evidence,
        };
        let effect = ValidatorEffect::new(&merged, &mut step);
        let authority = CaseAuthority::new(&case_dir, "allow-all");
        let end = run_effect(
            &case_dir,
            "allow-all",
            &protocol,
            &authority,
            &effect,
            PrMerged::ACTION,
            "allow-all-2",
        )
        .expect("the runtime ends");
        assert!(end.effects.is_empty(), "{:?}", end.effects);
    }
    assert_eq!(merges(), 0, "{:?}", memory.calls.borrow());

    // Under the operator's grant at the case's revision, the same effect merges.
    grant(&tree, "allow-all", "repository.merge", "human:operator").expect("the grant is written");
    {
        let evidence = CaseEvidence::new(&mut case, producers).expect("the port opens");
        let mut step = Observes {
            tree: &tree,
            revision: revision.clone(),
            evidence: &evidence,
        };
        let effect = ValidatorEffect::new(&merged, &mut step);
        let authority = CaseAuthority::new(&case_dir, "allow-all");
        let end = run_effect(
            &case_dir,
            "allow-all",
            &protocol,
            &authority,
            &effect,
            PrMerged::ACTION,
            "allow-all-3",
        )
        .expect("the runtime ends");
        assert!(
            matches!(end.effects.as_slice(), [EffectOutcome::Performed(_)]),
            "{:?}",
            end.effects
        );
    }
    assert_eq!(merges(), 1, "{:?}", memory.calls.borrow());
    assert_eq!(merge_records(), 1);
}
