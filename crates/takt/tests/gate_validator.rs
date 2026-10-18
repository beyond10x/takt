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
            .contains("artifact: .engineering/cases/retained/artifacts/gate-result-1.bundle.json"),
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

#[test]
fn an_empty_gates_list_aborts_the_run_and_writes_no_record() {
    let tree = scratch_document(
        "empty",
        "format: engineering-gates/1\nlanguage: canon-expr/1\ngates: []\n",
        &[],
    );
    let ran = run_with(&tree, "empty", PROTOCOL, &NoHarness, 4);
    let error = format!(
        "{:#}",
        ran.stop.expect_err("an invalid document aborts the run")
    );
    assert!(
        error.contains("a gates document cannot be empty"),
        "{error}"
    );
    assert!(error.contains(".engineering/gates.yaml"), "{error}");
    assert!(ran.records.is_empty(), "{:?}", ran.records);
    assert!(!ran.text.contains("evidence: gate_result"), "{}", ran.text);
    assert_eq!(
        claim(&ran.decision, "gate.green"),
        "unknown",
        "{}",
        ran.decision.json
    );
}

#[test]
fn an_unregistered_function_aborts_the_run_rather_than_passing() {
    let tree = scratch_document(
        "unregistered",
        &document(&[("manifest", r#"no.such.function("Cargo.toml")"#)]),
        &[],
    );
    let ran = run_with(&tree, "unregistered", PROTOCOL, &NoHarness, 4);
    let error = format!(
        "{:#}",
        ran.stop
            .expect_err("an unregistered function aborts the run")
    );
    assert!(error.contains(".engineering/gates.yaml"), "{error}");
    assert!(ran.records.is_empty(), "{:?}", ran.records);
    assert!(!ran.text.contains("evidence: gate_result"), "{}", ran.text);
    assert_eq!(
        claim(&ran.decision, "gate.green"),
        "unknown",
        "{}",
        ran.decision.json
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
