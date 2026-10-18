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

/// What a run left: how it stopped, what it printed, the records it wrote, and the decision
/// after it.
struct Outcome {
    stop: Stop,
    text: String,
    records: Vec<serde_json::Value>,
    decision: Decision,
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
    }
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
