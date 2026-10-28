//! The gate validator through the loop: a scratch repository whose `.engineering/gates.yaml`
//! holds one assertion, a run with no harness on a protocol that declares `gate_result` and
//! `gate.green`. A true assertion writes one `gate_result` pass record about `implementation`
//! at HEAD and the next decision holds `gate.green`; a false one writes one `fail` record and
//! the claim is false; an assertion the provider cannot observe writes nothing, because unknown
//! is not false. Every record comes from the validator.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::anyhow;
use takt::harness::{Harness, StepContext, StepReport};
use takt::validator::{GateRun, Validator};
use takt::{Decision, Run, RunOptions, Stop};

/// The protocol under test, relative to this crate: `gate_result`, `gate.green`, `gate.run`.
const PROTOCOL: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/gate-only/1.yaml"
);

/// Runs `git <args>` in `tree`; its trimmed standard output.
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

/// A scratch git tree with one commit, under this crate's build directory. Dropping it removes
/// the tree; a removal that fails fails the case, unless the case is already failing.
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

/// A committed scratch repository with a manifest and a gates document holding `assertion`.
fn scratch(name: &str, assertion: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("gate-validator-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(dir.join(".engineering")).expect("scratch dir");
    fs::write(dir.join("Cargo.toml"), "[package]\nname = \"scratch\"\n").expect("a manifest");
    fs::write(
        dir.join(".engineering/gates.yaml"),
        format!(
            "format: engineering-gates/1\nlanguage: canon-expr/1\ngates:\n  - id: manifest\n    \
             text: The repository has its manifest.\n    assertions:\n      - '{assertion}'\n"
        ),
    )
    .expect("a gates document");
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "start"]);
    Scratch(dir)
}

/// A harness the runs below never reach: every step they take is the validator's.
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

/// What a run left: how it stopped, what it printed, the records it wrote, the decision
/// after it, and the case directory.
struct Outcome {
    stop: Stop,
    text: String,
    records: Vec<serde_json::Value>,
    decision: Decision,
    case_dir: PathBuf,
}

/// A run on the fixture protocol over `tree` with the gate validator over
/// `.engineering/gates.yaml` as the only validator, two steps at most.
fn gated_run(tree: &Path, id: &str) -> Outcome {
    let options = RunOptions {
        protocol: PROTOCOL.to_owned(),
        intent: "pass the gate".to_owned(),
        inputs: Vec::new(),
        iterations: Some(2),
        hours: None,
        tokens: None,
        case_id: Some(id.to_owned()),
        gate: None,
    };
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(GateRun::new(".engineering/gates.yaml"))];
    let mut run = Run::open(tree, options, &NoHarness, &validators).expect("the case opens");
    let mut out = Vec::new();
    let stop = run.run(&mut out).expect("the run stops without an error");
    let text = String::from_utf8(out).expect("utf-8");
    let mut paths: Vec<PathBuf> = fs::read_dir(run.case.dir.join("evidence"))
        .expect("an evidence directory")
        .map(|entry| entry.expect("an entry").path())
        .collect();
    paths.sort();
    let records = paths
        .iter()
        .map(|path| {
            serde_json::from_str(&fs::read_to_string(path).expect("a record")).expect("json")
        })
        .collect();
    let decision = run.case.evaluate(99).expect("evaluates");
    Outcome {
        stop,
        text,
        records,
        decision,
        case_dir: run.case.dir.clone(),
    }
}

/// The bundle the library evaluated, `engineering-gate-evidence/1`, is kept under the case as
/// `artifacts/<record-id>.bundle.json`, with a record and without one, and the run names it.
#[test]
fn the_evaluated_bundle_is_retained_under_the_case_artifacts() {
    let tree = scratch("retained", r#"fs.file.exists("Cargo.toml")"#);
    let outcome = gated_run(&tree, "retained");
    assert_eq!(outcome.records.len(), 1, "{}", outcome.text);
    let bundle = outcome
        .case_dir
        .join("artifacts")
        .join("gate-result-1.bundle.json");
    assert!(bundle.is_file(), "{}\n{}", bundle.display(), outcome.text);
    let document: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&bundle).expect("the bundle")).expect("json");
    assert_eq!(
        document["format"], "engineering-gate-evidence/1",
        "{document}"
    );
    assert_eq!(
        document["observations"].as_array().map(Vec::len),
        Some(1),
        "{document}"
    );
    assert!(
        outcome
            .text
            .contains("artifact: .takt/cases/retained/artifacts/gate-result-1.bundle.json"),
        "{}",
        outcome.text
    );
    assert!(
        outcome
            .text
            .contains("retained as gate-result-1.bundle.json"),
        "{}",
        outcome.text
    );

    let unknown = scratch("retained-unknown", r#"text.contains("missing.md", "x")"#);
    let outcome = gated_run(&unknown, "retained-unknown");
    assert!(outcome.records.is_empty(), "{}", outcome.text);
    let bundle = outcome
        .case_dir
        .join("artifacts")
        .join("gate-result-1.bundle.json");
    assert!(bundle.is_file(), "{}\n{}", bundle.display(), outcome.text);
}

/// The bundle's `source_identity` is the repository tree's: what the loop writes under
/// `.takt/cases` between two observations at one revision does not move it, a change
/// to the tree does.
#[test]
fn the_bundle_source_identity_leaves_the_case_directory_out() {
    let tree = scratch("identity", r#"fs.file.exists("Cargo.toml")"#);
    // The gate validator reads the gates document from the case's base revision: the tree is
    // on a case's branch, as a run leaves it.
    let options = RunOptions {
        protocol: PROTOCOL.to_owned(),
        intent: "pass the gate".to_owned(),
        inputs: Vec::new(),
        iterations: Some(0),
        hours: None,
        tokens: None,
        case_id: Some("identity".to_owned()),
        gate: None,
    };
    let none: Vec<Box<dyn Validator>> = Vec::new();
    let opened = Run::open(&tree, options, &NoHarness, &none).expect("the case opens");
    let validator = GateRun::new(".engineering/gates.yaml");
    let identity = |observation: &takt::validator::Observation| -> String {
        let bytes = &observation.artifact.as_ref().expect("a bundle").bytes;
        let bundle: serde_json::Value = serde_json::from_slice(bytes).expect("json");
        bundle["source_identity"]
            .as_str()
            .expect("a source identity")
            .to_owned()
    };
    let first = validator.observe(&tree, "r0", 1).expect("observes");
    let case = tree.join(".takt/cases/identity");
    fs::create_dir_all(case.join("decisions")).expect("a case directory");
    fs::write(case.join("decisions/0001.json"), "{}\n").expect("a decision");
    fs::write(case.join("intent.md"), "pass the gate").expect("an intent");
    let second = validator.observe(&tree, "r0", 2).expect("observes");
    assert_eq!(identity(&first), identity(&second));
    fs::write(tree.join("README.md"), "changed\n").expect("a change to the tree");
    let third = validator.observe(&tree, "r0", 3).expect("observes");
    assert_ne!(identity(&first), identity(&third));
    drop(opened);
}

fn claim<'a>(decision: &'a Decision, id: &str) -> &'a str {
    decision
        .claims
        .get(id)
        .map(String::as_str)
        .unwrap_or("absent")
}

#[test]
fn a_true_assertion_writes_one_pass_record_and_gate_green_holds() {
    let tree = scratch("pass", r#"fs.file.exists("Cargo.toml")"#);
    let head = git(&tree, &["rev-parse", "HEAD"]);
    let outcome = gated_run(&tree, "pass");
    assert_eq!(
        outcome.stop,
        Stop::Completed("accepted".to_owned()),
        "{}",
        outcome.text
    );
    assert!(
        outcome.text.contains("step 1: gate.run {}"),
        "{}",
        outcome.text
    );
    assert!(
        outcome.text.contains("evidence: gate_result pass"),
        "{}",
        outcome.text
    );
    assert_eq!(outcome.records.len(), 1, "{}", outcome.text);
    let record = &outcome.records[0];
    assert_eq!(record["format"], "canon-evidence/1", "{record}");
    assert_eq!(record["kind"], "gate_result", "{record}");
    assert_eq!(record["result"], "pass", "{record}");
    assert_eq!(record["subject"], "implementation", "{record}");
    assert_eq!(record["subject_revision"], head.as_str(), "{record}");
    assert_eq!(record["producer"], "takt.gate-run", "{record}");
    assert_eq!(
        claim(&outcome.decision, "gate.green"),
        "true",
        "{}",
        outcome.decision.json
    );
}

#[test]
fn a_false_assertion_writes_one_fail_record_and_gate_green_is_false() {
    let tree = scratch("fail", r#"fs.file.exists("missing.toml")"#);
    let head = git(&tree, &["rev-parse", "HEAD"]);
    let outcome = gated_run(&tree, "fail");
    // The record is there, so the validator is not due again, and nothing else is admissible.
    assert_eq!(outcome.stop, Stop::NothingAdmissible, "{}", outcome.text);
    assert!(
        outcome.text.contains("evidence: gate_result fail"),
        "{}",
        outcome.text
    );
    assert_eq!(outcome.records.len(), 1, "{}", outcome.text);
    let record = &outcome.records[0];
    assert_eq!(record["kind"], "gate_result", "{record}");
    assert_eq!(record["result"], "fail", "{record}");
    assert_eq!(record["subject"], "implementation", "{record}");
    assert_eq!(record["subject_revision"], head.as_str(), "{record}");
    assert_eq!(record["producer"], "takt.gate-run", "{record}");
    assert_eq!(
        claim(&outcome.decision, "gate.green"),
        "false",
        "{}",
        outcome.decision.json
    );
}

#[test]
fn an_assertion_the_provider_cannot_observe_writes_no_record() {
    // `text.contains` over a file that is not there is unavailable, not false.
    let tree = scratch("unknown", r#"text.contains("missing.md", "x")"#);
    let outcome = gated_run(&tree, "unknown");
    assert_ne!(
        outcome.stop,
        Stop::Completed("accepted".to_owned()),
        "{}",
        outcome.text
    );
    assert!(
        outcome.text.contains("step 1: gate.run {}"),
        "{}",
        outcome.text
    );
    assert!(
        !outcome.text.contains("evidence: gate_result"),
        "{}",
        outcome.text
    );
    assert!(outcome.text.contains("unknown"), "{}", outcome.text);
    assert!(outcome.records.is_empty(), "{:?}", outcome.records);
    assert_eq!(
        claim(&outcome.decision, "gate.green"),
        "unknown",
        "{}",
        outcome.decision.json
    );
}

// Adversary pass 1 over story:gate-validator: what the cases above do not say. Every helper
// below is this pass's own; nothing above is changed.

use std::cell::Cell;

/// A protocol with a judgement step beside the gate: `implementation.edit` with no
/// precondition, `gate.run` owned by the validator, accepted once `gate.green` holds.
const EDIT_PROTOCOL: &str = "format: protocol/1

protocol:
  id: gate.edit
  revision: 1
  description: >-
    Edit the implementation until the repository's gate is green at its current revision.

artifacts:
  implementation:
    description: a revision of the code

evidence_kinds:
  gate_result:
    description: the result of the repository's gate against an implementation revision

claims:
  gate.green:
    description: the repository's gate is green for the current implementation revision
    true_when:
      evidence:
        kind: gate_result
        result: pass
        subject: implementation

obligations:
  gate.required:
    description: the repository's gate is green for the current implementation revision
    discharged_when:
      claim: gate.green

actions:
  implementation.edit:
    description: change the implementation in the repository's working revision
    effect: write
  gate.run:
    description: run the repository's gate against an implementation revision
    effect: read
    may_produce:
      - evidence: gate_result

outcomes:
  accepted:
    description: the repository's gate is green for the current implementation revision
    requires:
      claim: gate.green
";

/// A committed scratch repository with a manifest, the gates document `document` verbatim at
/// `.engineering/gates.yaml`, and the `extra` files (path, content) beside them.
fn scratch_document(name: &str, document: &str, extra: &[(&str, &str)]) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("gate-validator-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(dir.join(".engineering")).expect("scratch dir");
    fs::write(dir.join("Cargo.toml"), "[package]\nname = \"scratch\"\n").expect("a manifest");
    fs::write(dir.join(".engineering/gates.yaml"), document).expect("a gates document");
    for (path, content) in extra {
        let path = dir.join(path);
        fs::create_dir_all(path.parent().expect("a parent")).expect("a directory");
        fs::write(path, content).expect("an extra file");
    }
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "start"]);
    Scratch(dir)
}

/// An `engineering-gates/1` document over `gates`: (id, its one assertion), in that order.
fn document(gates: &[(&str, &str)]) -> String {
    let mut text = String::from("format: engineering-gates/1\nlanguage: canon-expr/1\ngates:\n");
    for (id, assertion) in gates {
        text.push_str(&format!(
            "  - id: {id}\n    text: The gate {id} holds.\n    assertions:\n      - '{assertion}'\n"
        ));
    }
    text
}

/// The evidence records under `case_dir`, in file order.
fn records_in(case_dir: &Path) -> Vec<serde_json::Value> {
    let mut paths: Vec<PathBuf> = fs::read_dir(case_dir.join("evidence"))
        .expect("an evidence directory")
        .map(|entry| entry.expect("an entry").path())
        .collect();
    paths.sort();
    paths
        .iter()
        .map(|path| {
            serde_json::from_str(&fs::read_to_string(path).expect("a record")).expect("json")
        })
        .collect()
}

/// What a run left when it may also have failed: the stop or the error, what it printed, the
/// records it wrote, the steps it took and the decision after it.
struct Ran {
    stop: anyhow::Result<Stop>,
    text: String,
    records: Vec<serde_json::Value>,
    steps: u64,
    decision: Decision,
}

/// A run on `protocol` over `tree` with the gate validator over `.engineering/gates.yaml` as
/// the only validator and `harness` as the judgement actor, `iterations` steps at most.
fn run_with(tree: &Path, id: &str, protocol: &str, harness: &dyn Harness, iterations: u64) -> Ran {
    let options = RunOptions {
        protocol: protocol.to_owned(),
        intent: "pass the gate".to_owned(),
        inputs: Vec::new(),
        iterations: Some(iterations),
        hours: None,
        tokens: None,
        case_id: Some(id.to_owned()),
        gate: None,
    };
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(GateRun::new(".engineering/gates.yaml"))];
    let mut run = Run::open(tree, options, harness, &validators).expect("the case opens");
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let text = String::from_utf8(out).expect("utf-8");
    let records = records_in(&run.case.dir);
    let decision = run.case.evaluate(99).expect("evaluates");
    Ran {
        stop,
        text,
        records,
        steps: run.steps,
        decision,
    }
}

/// A harness that, offered `implementation.edit`, writes `path` with `content`; it counts its
/// calls so a case can say whether the loop ever reached it.
struct Writes {
    path: &'static str,
    content: &'static str,
    calls: Cell<usize>,
}

impl Writes {
    fn new(path: &'static str, content: &'static str) -> Self {
        Self {
            path,
            content,
            calls: Cell::new(0),
        }
    }
}

impl Harness for Writes {
    fn name(&self) -> &str {
        "writes"
    }

    fn step(&self, context: &StepContext<'_>) -> Result<StepReport, anyhow::Error> {
        self.calls.set(self.calls.get() + 1);
        assert!(
            context.offered.contains(&"implementation.edit".to_owned()),
            "implementation.edit is offered, not only {:?}",
            context.offered
        );
        fs::write(context.tree.join(self.path), self.content)?;
        Ok(StepReport {
            action: "implementation.edit".to_owned(),
            summary: format!("wrote {}", self.path),
            tokens: None,
        })
    }
}

#[test]
fn with_two_gates_and_the_second_false_the_result_is_fail_and_the_summary_names_it() {
    let tree = scratch_document(
        "second-false",
        &document(&[
            ("manifest", r#"fs.file.exists("Cargo.toml")"#),
            ("license", r#"fs.file.exists("LICENSE")"#),
        ]),
        &[],
    );
    let outcome = gated_run(&tree, "second-false");
    assert_eq!(outcome.stop, Stop::NothingAdmissible, "{}", outcome.text);
    assert_eq!(outcome.records.len(), 1, "{}", outcome.text);
    assert_eq!(outcome.records[0]["result"], "fail", "{}", outcome.text);
    assert!(
        outcome
            .text
            .contains(r#"gate license false: fs.file.exists("LICENSE")"#),
        "{}",
        outcome.text
    );
    assert!(!outcome.text.contains("gate manifest"), "{}", outcome.text);
    assert_eq!(
        claim(&outcome.decision, "gate.green"),
        "false",
        "{}",
        outcome.decision.json
    );
}

#[test]
fn an_unknown_gate_beside_a_false_one_is_fail_and_the_summary_names_the_false_gate() {
    // Kleene: unknown and false is false, so the document decided, and the false gate is the one
    // that decided it, not the unknown one before it.
    let tree = scratch_document(
        "unknown-then-false",
        &document(&[
            ("readme", r#"text.contains("README.md", "x")"#),
            ("license", r#"fs.file.exists("LICENSE")"#),
        ]),
        &[],
    );
    let outcome = gated_run(&tree, "unknown-then-false");
    assert_eq!(outcome.records.len(), 1, "{}", outcome.text);
    assert_eq!(outcome.records[0]["result"], "fail", "{}", outcome.text);
    assert!(
        outcome.text.contains("gate license false:"),
        "{}",
        outcome.text
    );
    assert!(!outcome.text.contains("gate readme"), "{}", outcome.text);
    assert_eq!(
        claim(&outcome.decision, "gate.green"),
        "false",
        "{}",
        outcome.decision.json
    );
}

/// The refusal of a case opened over `tree` with the gate validator over
/// `.engineering/gates.yaml` as the only validator (story:gates-project-checked-at-open: a
/// document the gate's preparation refuses refuses the case when it opens, no step run): the
/// error, after checking that no case was left under the cases directory.
fn refused_at_open(tree: &Path, id: &str) -> String {
    let options = RunOptions {
        protocol: PROTOCOL.to_owned(),
        intent: "pass the gate".to_owned(),
        inputs: Vec::new(),
        iterations: Some(4),
        hours: None,
        tokens: None,
        case_id: Some(id.to_owned()),
        gate: None,
    };
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(GateRun::new(".engineering/gates.yaml"))];
    let error = match Run::open(tree, options, &NoHarness, &validators) {
        Ok(_) => panic!(
            "story:gates-project-checked-at-open: the case opened over a gates document the gate refuses"
        ),
        Err(error) => format!("{error:#}"),
    };
    assert!(
        !tree.join(takt::case::CASES_DIR).join(id).exists(),
        "story:gates-project-checked-at-open: the case {id} was left: {error}"
    );
    error
}

#[test]
fn an_empty_gates_list_aborts_the_run_and_writes_no_record() {
    let tree = scratch_document(
        "empty",
        "format: engineering-gates/1\nlanguage: canon-expr/1\ngates: []\n",
        &[],
    );
    let error = refused_at_open(&tree, "empty");
    assert!(
        error.contains("a gates document cannot be empty"),
        "{error}"
    );
    assert!(error.contains(".engineering/gates.yaml"), "{error}");
}

#[test]
fn an_unregistered_function_aborts_the_run_rather_than_passing() {
    let tree = scratch_document(
        "unregistered",
        &document(&[("manifest", r#"no.such.function("Cargo.toml")"#)]),
        &[],
    );
    let error = refused_at_open(&tree, "unregistered");
    assert!(error.contains(".engineering/gates.yaml"), "{error}");
    assert!(
        error.contains("unknown function no.such.function"),
        "{error}"
    );
}

#[test]
fn an_unknown_gate_stops_the_run_as_no_progress_after_one_step() {
    // The budget is wide so that a loop that ran the validator every pass would show as
    // `Iterations (10)` with ten steps, not as the one idle step and `NoProgress` claimed.
    let tree = scratch("unknown-stops", r#"text.contains("missing.md", "x")"#);
    let ran = run_with(&tree, "unknown-stops", PROTOCOL, &NoHarness, 10);
    assert_eq!(
        ran.stop.expect("the run stops"),
        Stop::NoProgress,
        "{}",
        ran.text
    );
    assert_eq!(ran.steps, 1, "{}", ran.text);
    assert_eq!(
        ran.text
            .lines()
            .filter(|line| line.starts_with("step "))
            .count(),
        1,
        "{}",
        ran.text
    );
}

#[test]
fn an_unknown_gate_does_not_starve_the_judgement_step_that_would_make_it_observable() {
    // The gate reads a file the implementation does not have yet; the harness, offered
    // `implementation.edit`, would write it. The gate is unknown at HEAD, so the run must give
    // the harness its step and evaluate the gate again at the commit that follows.
    let tree = scratch_document(
        "starved",
        &document(&[("readme", r#"text.contains("README.md", "ready")"#)]),
        &[("protocol.yaml", EDIT_PROTOCOL)],
    );
    let protocol = tree.join("protocol.yaml");
    let harness = Writes::new("README.md", "ready\n");
    let ran = run_with(
        &tree,
        "starved",
        protocol.to_str().expect("utf-8"),
        &harness,
        6,
    );
    let stop = ran.stop.expect("the run stops");
    assert_eq!(
        harness.calls.get(),
        1,
        "the harness is offered implementation.edit once while the gate is unknown at HEAD; \
         the run stopped {stop} after {} step(s):\n{}",
        ran.steps,
        ran.text
    );
    let head = git(&tree, &["rev-parse", "HEAD"]);
    assert_eq!(stop, Stop::Completed("accepted".to_owned()), "{}", ran.text);
    assert_eq!(ran.records.len(), 1, "{}", ran.text);
    assert_eq!(ran.records[0]["result"], "pass", "{}", ran.text);
    assert_eq!(
        ran.records[0]["subject_revision"],
        head.as_str(),
        "{}",
        ran.text
    );
    assert_eq!(
        claim(&ran.decision, "gate.green"),
        "true",
        "{}",
        ran.decision.json
    );
}

#[test]
fn after_a_commit_the_gate_runs_again_at_the_new_head_and_a_stale_record_does_not_count() {
    let tree = scratch_document(
        "recommit",
        &document(&[("license", r#"fs.file.exists("LICENSE")"#)]),
        &[("protocol.yaml", EDIT_PROTOCOL)],
    );
    let head0 = git(&tree, &["rev-parse", "HEAD"]);
    let protocol = tree.join("protocol.yaml");
    let harness = Writes::new("LICENSE", "MIT\n");
    let ran = run_with(
        &tree,
        "recommit",
        protocol.to_str().expect("utf-8"),
        &harness,
        6,
    );
    assert_eq!(
        ran.stop.expect("the run stops"),
        Stop::Completed("accepted".to_owned()),
        "{}",
        ran.text
    );
    let head1 = git(&tree, &["rev-parse", "HEAD"]);
    assert_ne!(head0, head1, "{}", ran.text);
    assert_eq!(harness.calls.get(), 1, "{}", ran.text);
    assert_eq!(ran.steps, 3, "{}", ran.text);
    assert_eq!(ran.records.len(), 2, "{}", ran.text);
    let (stale, fresh) = (&ran.records[0], &ran.records[1]);
    assert_eq!(stale["id"], "gate-result-1", "{stale}");
    assert_eq!(stale["result"], "fail", "{stale}");
    assert_eq!(stale["subject_revision"], head0.as_str(), "{stale}");
    assert_eq!(stale["producer"], "takt.gate-run", "{stale}");
    assert_eq!(fresh["id"], "gate-result-3", "{fresh}");
    assert_eq!(fresh["result"], "pass", "{fresh}");
    assert_eq!(fresh["subject_revision"], head1.as_str(), "{fresh}");
    assert_eq!(fresh["producer"], "takt.gate-run", "{fresh}");
    assert_eq!(
        claim(&ran.decision, "gate.green"),
        "true",
        "{}",
        ran.decision.json
    );
}

#[test]
fn a_command_bound_gate_with_its_project_file_is_evaluated() {
    // The guide's own project example: a command admitted in an
    // `engineering-assertions-project/1` file, asserted through `process.succeeded`. The
    // project file's path is this case's assumption (`.engineering/assertions-project.yaml`,
    // the guide's example path); the validator takes none at all.
    use std::os::unix::fs::PermissionsExt as _;
    let tree = scratch_document(
        "project",
        &document(&[("check", r#"process.succeeded("check")"#)]),
        &[
            (
                ".engineering/assertions-project.yaml",
                "format: engineering-assertions-project/1\ncommands:\n  check:\n    \
                 executable: check.sh\n",
            ),
            ("check.sh", "#!/bin/sh\nexit 0\n"),
        ],
    );
    fs::set_permissions(tree.join("check.sh"), fs::Permissions::from_mode(0o755))
        .expect("exec bit");
    // The mode change is committed: a case opens only on a clean tree, and a validator
    // observes only a tree with nothing uncommitted.
    git(&tree, &["commit", "-qam", "check.sh is executable"]);
    let ran = run_with(&tree, "project", PROTOCOL, &NoHarness, 4);
    assert_eq!(
        ran.stop
            .expect("the run stops: the gate is a command the project file admits"),
        Stop::Completed("accepted".to_owned()),
        "{}",
        ran.text
    );
    assert_eq!(ran.records.len(), 1, "{}", ran.text);
    assert_eq!(ran.records[0]["result"], "pass", "{}", ran.text);
}

// Adversary pass 2 over story:gate-validator: the correction's idle step and its retained
// bundle. Every helper below is this pass's own; nothing above is changed.

use takt::evidence::Record;
use takt::validator::Observation;

/// A protocol whose two actions are both validator-owned, `tests.run` and `gate.run`, each
/// with no precondition, so no judgement step is ever offered; accepted once `gate.green`
/// holds.
const TWO_VALIDATORS_PROTOCOL: &str = "format: protocol/1

protocol:
  id: gate.and.tests
  revision: 1
  description: >-
    Run the tests and the repository's gate against the implementation at its current revision;
    accepted once the gate is green.

artifacts:
  implementation:
    description: a revision of the code

evidence_kinds:
  test_result:
    description: the result of running tests against an implementation revision
  gate_result:
    description: the result of the repository's gate against an implementation revision

claims:
  tests.pass:
    description: the tests pass for the current implementation revision
    true_when:
      evidence:
        kind: test_result
        result: pass
        subject: implementation
  gate.green:
    description: the repository's gate is green for the current implementation revision
    true_when:
      evidence:
        kind: gate_result
        result: pass
        subject: implementation

obligations:
  gate.required:
    description: the repository's gate is green for the current implementation revision
    discharged_when:
      claim: gate.green

actions:
  tests.run:
    description: run the tests against an implementation revision
    effect: read
    may_produce:
      - evidence: test_result
  gate.run:
    description: run the repository's gate against an implementation revision
    effect: read
    may_produce:
      - evidence: gate_result

outcomes:
  accepted:
    description: the repository's gate is green for the current implementation revision
    requires:
      claim: gate.green
";

/// `tests.run` without a test command: one `test_result` pass at the revision.
struct TestsPass;

impl Validator for TestsPass {
    fn name(&self) -> &str {
        "adv2.tests-pass"
    }

    fn action(&self) -> &str {
        "tests.run"
    }

    fn kind(&self) -> &str {
        "test_result"
    }

    fn subject(&self) -> &str {
        "implementation"
    }

    fn observe(&self, _: &Path, revision: &str, seq: usize) -> Result<Observation, anyhow::Error> {
        Ok(Observation {
            summary: "pass".to_owned(),
            records: vec![Record::new(
                format!("test-result-{seq}"),
                self.kind(),
                Some("pass".to_owned()),
                self.subject(),
                revision,
                self.name(),
            )],
            artifact: None,
        })
    }
}

#[test]
fn an_unknown_gate_does_not_stop_the_run_while_another_validator_is_due() {
    // The gate validator comes first and observes unknown; the tests validator is admissible
    // and has no evidence, so it is due at the next pass. The idle step the correction added
    // must not stop the run before that due validator has run.
    let tree = scratch_document(
        "gate-then-tests",
        &document(&[("readme", r#"text.contains("README.md", "x")"#)]),
        &[("protocol.yaml", TWO_VALIDATORS_PROTOCOL)],
    );
    let protocol = tree.join("protocol.yaml");
    let options = RunOptions {
        protocol: protocol.to_str().expect("utf-8").to_owned(),
        intent: "pass the gate".to_owned(),
        inputs: Vec::new(),
        iterations: Some(6),
        hours: None,
        tokens: None,
        case_id: Some("gate-then-tests".to_owned()),
        gate: None,
    };
    let validators: Vec<Box<dyn Validator>> = vec![
        Box::new(GateRun::new(".engineering/gates.yaml")),
        Box::new(TestsPass),
    ];
    let mut run = Run::open(&tree, options, &NoHarness, &validators).expect("the case opens");
    let mut out = Vec::new();
    let stop = run.run(&mut out).expect("the run stops");
    let text = String::from_utf8(out).expect("utf-8");
    let records = records_in(&run.case.dir);
    let test_results = records
        .iter()
        .filter(|record| record["kind"] == "test_result")
        .count();
    assert_eq!(
        test_results, 1,
        "tests.run was admissible with no evidence at the second pass and never ran; the run \
         stopped {stop} after {} step(s):\n{text}",
        run.steps
    );
    assert_eq!(run.steps, 2, "{text}");
    assert_eq!(
        claim(&run.case.evaluate(99).expect("evaluates"), "tests.pass"),
        "true",
        "{text}"
    );
}

/// Makes `path` writable again when dropped, so the scratch guard can remove the tree.
struct WritableAgain(PathBuf);

impl Drop for WritableAgain {
    fn drop(&mut self) {
        use std::os::unix::fs::PermissionsExt as _;
        let _ = fs::set_permissions(&self.0, fs::Permissions::from_mode(0o755));
    }
}

/// A harness that, offered `implementation.edit`, writes `LICENSE` and makes the case's
/// `artifacts/` directory read-only, so the bundle of the observation that follows the commit
/// cannot be retained.
struct WritesAndLocksArtifacts {
    artifacts: PathBuf,
}

impl Harness for WritesAndLocksArtifacts {
    fn name(&self) -> &str {
        "writes-and-locks"
    }

    fn step(&self, context: &StepContext<'_>) -> Result<StepReport, anyhow::Error> {
        use std::os::unix::fs::PermissionsExt as _;
        assert!(
            context.offered.contains(&"implementation.edit".to_owned()),
            "implementation.edit is offered, not only {:?}",
            context.offered
        );
        fs::write(context.tree.join("LICENSE"), "MIT\n")?;
        fs::set_permissions(&self.artifacts, fs::Permissions::from_mode(0o555))?;
        Ok(StepReport {
            action: "implementation.edit".to_owned(),
            summary: "wrote LICENSE".to_owned(),
            tokens: None,
        })
    }
}

#[test]
fn a_record_does_not_land_when_its_bundle_cannot_be_retained() {
    // The summary of every observation says "retained as <bundle>", and the story made the
    // retained evidence the record's reference. When the loop cannot write the bundle, the
    // run fails; the record that would have referred to it must not be left behind as a
    // `pass` at HEAD with nothing retained.
    let tree = scratch_document(
        "unretained",
        &document(&[("license", r#"fs.file.exists("LICENSE")"#)]),
        &[("protocol.yaml", EDIT_PROTOCOL)],
    );
    let artifacts = tree.join(".takt/cases/unretained/artifacts");
    let _writable = WritableAgain(artifacts.clone());
    let protocol = tree.join("protocol.yaml");
    let harness = WritesAndLocksArtifacts {
        artifacts: artifacts.clone(),
    };
    let ran = run_with(
        &tree,
        "unretained",
        protocol.to_str().expect("utf-8"),
        &harness,
        6,
    );
    let error = format!(
        "{:#}",
        ran.stop
            .expect_err("the bundle could not be retained, so the run fails")
    );
    assert!(
        error.contains("gate-result-3.bundle.json"),
        "{error}\n{}",
        ran.text
    );
    assert!(
        !artifacts.join("gate-result-3.bundle.json").exists(),
        "{}",
        ran.text
    );
    let head = git(&tree, &["rev-parse", "HEAD"]);
    let at_head: Vec<&serde_json::Value> = ran
        .records
        .iter()
        .filter(|record| record["subject_revision"] == head.as_str())
        .collect();
    assert!(
        at_head.is_empty(),
        "a gate_result landed at {head} without its retained bundle: {at_head:?}\n{}",
        ran.text
    );
    assert_eq!(
        claim(&ran.decision, "gate.green"),
        "unknown",
        "{}",
        ran.decision.json
    );
}

#[test]
fn every_observation_retains_its_own_bundle_and_each_replays_to_its_truth() {
    // Two observations in one run, a fail at HEAD0 and a pass at HEAD1: two bundles, named by
    // their records, and each replays through the library to the truth its record carries.
    let tree = scratch_document(
        "two-bundles",
        &document(&[("license", r#"fs.file.exists("LICENSE")"#)]),
        &[("protocol.yaml", EDIT_PROTOCOL)],
    );
    let protocol = tree.join("protocol.yaml");
    let harness = Writes::new("LICENSE", "MIT\n");
    let ran = run_with(
        &tree,
        "two-bundles",
        protocol.to_str().expect("utf-8"),
        &harness,
        6,
    );
    assert_eq!(
        ran.stop.expect("the run stops"),
        Stop::Completed("accepted".to_owned()),
        "{}",
        ran.text
    );
    let artifacts = tree.join(".takt/cases/two-bundles/artifacts");
    let mut names: Vec<String> = fs::read_dir(&artifacts)
        .expect("an artifacts directory")
        .map(|entry| {
            entry
                .expect("an entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    assert_eq!(
        names,
        vec![
            "gate-result-1.bundle.json".to_owned(),
            "gate-result-3.bundle.json".to_owned()
        ],
        "{}",
        ran.text
    );
    for (name, expected) in [
        ("gate-result-1.bundle.json", canon_expr::Truth::False),
        ("gate-result-3.bundle.json", canon_expr::Truth::True),
    ] {
        let bytes = fs::read(artifacts.join(name)).expect("the bundle");
        let bundle: canon_engineering_assertions::Bundle =
            serde_json::from_slice(&bytes).expect("an engineering-gate-evidence/1 bundle");
        let report =
            canon_engineering_assertions::evaluate(&bundle, bundle.evaluated_at, None, None)
                .expect("the retained bundle replays");
        assert!(
            report.truth == expected,
            "{name} replays to {:?}, not {expected:?}\n{}",
            report.truth,
            ran.text
        );
    }
}
