//! The loop end to end with a scripted harness and a real validator over a scratch git tree: a
//! failing check, one `implementation.edit` that fixes it, the validator's pass, then the
//! harness's request of the merge gate. Every evidence record comes from the validator.

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use takt::harness::{Harness, StepContext, StepReport};
use takt::validator::{TestsRun, Validator};
use takt::{Run, RunOptions, Stop};

fn git(tree: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(args)
        .current_dir(tree)
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?}");
}

/// A scratch tree whose check passes once `check.txt` says `fixed`.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("takt-loop-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("scratch dir");
    fs::write(dir.join("check.txt"), "broken\n").expect("check file");
    fs::write(dir.join("check.sh"), "#!/bin/sh\ngrep -q fixed check.txt\n").expect("check script");
    fs::set_permissions(dir.join("check.sh"), fs::Permissions::from_mode(0o755)).expect("exec bit");
    git(&dir, &["init", "-q", "-b", "main"]);
    git(
        &dir,
        &[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@localhost",
            "add",
            "-A",
        ],
    );
    git(
        &dir,
        &[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@localhost",
            "commit",
            "-q",
            "-m",
            "broken",
        ],
    );
    dir
}

/// A harness that fixes the check on its first judgement step and requests the gate on its next.
struct Scripted;

impl Harness for Scripted {
    fn name(&self) -> &str {
        "scripted"
    }

    fn step(&self, context: &StepContext<'_>) -> Result<StepReport, anyhow::Error> {
        if context.decision.holds("tests.pass") {
            assert!(context.approvals.contains(&"repository.merge".to_owned()));
            return Ok(StepReport {
                action: "repository.merge".to_owned(),
                summary: "done".to_owned(),
                tokens: Some(10),
            });
        }
        assert!(context.offered.contains(&"implementation.edit".to_owned()));
        fs::write(context.tree.join("check.txt"), "fixed\n").expect("fix");
        Ok(StepReport {
            action: "implementation.edit".to_owned(),
            summary: "fixed the check".to_owned(),
            tokens: Some(100),
        })
    }
}

#[test]
fn a_change_is_validated_and_stops_at_the_merge_gate() {
    let tree = scratch("change");
    let options = RunOptions {
        protocol: "software.change/1".to_owned(),
        intent: "make the check pass".to_owned(),
        inputs: vec![
            ("risk".to_owned(), "trivial".to_owned()),
            ("change_kind".to_owned(), "feature".to_owned()),
            ("affects_security_boundary".to_owned(), "false".to_owned()),
        ],
        iterations: Some(8),
        hours: None,
        tokens: None,
        case_id: Some("loop-test".to_owned()),
        gate: Some("repository.merge".to_owned()),
    };
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(TestsRun::new("./check.sh").expect("a command"))];
    let mut run = Run::open(&tree, options, &Scripted, &validators).expect("opens");
    let mut out = Vec::new();
    let stop = run.run(&mut out).expect("runs");
    let text = String::from_utf8(out).expect("utf-8");
    assert_eq!(
        stop,
        Stop::ApprovalRequired(vec!["repository.merge".to_owned()]),
        "{text}"
    );
    assert_eq!(run.steps, 4, "{text}");
    assert_eq!(run.tokens, 110);
    assert!(text.contains("evidence: test_result fail"), "{text}");
    assert!(text.contains("evidence: test_result pass"), "{text}");
    let producers: Vec<String> = fs::read_dir(run.case.dir.join("evidence"))
        .expect("evidence dir")
        .map(|e| fs::read_to_string(e.expect("entry").path()).expect("record"))
        .map(|json| {
            serde_json_value(&json)["producer"]
                .as_str()
                .expect("producer")
                .to_owned()
        })
        .collect();
    assert_eq!(producers.len(), 2);
    assert!(
        producers.iter().all(|p| p == "takt.tests-run"),
        "{producers:?}"
    );
    let decision = run.case.evaluate(99).expect("evaluates");
    assert!(decision.holds("tests.pass"));
    assert!(decision.holds("implementation.verified"));
    assert_eq!(
        decision.actions.get("repository.merge").map(String::as_str),
        Some("approval-required")
    );
    let _ = fs::remove_dir_all(&tree);
}

#[test]
fn a_missing_case_input_is_refused() {
    let tree = scratch("inputs");
    let options = RunOptions {
        protocol: "software.change/1".to_owned(),
        intent: "x".to_owned(),
        inputs: vec![],
        iterations: Some(1),
        hours: None,
        tokens: None,
        case_id: Some("inputs-test".to_owned()),
        gate: None,
    };
    let validators: Vec<Box<dyn Validator>> = vec![];
    let error = Run::open(&tree, options, &Scripted, &validators)
        .err()
        .expect("refused")
        .to_string();
    assert!(error.contains("risk"), "{error}");
    let _ = fs::remove_dir_all(&tree);
}

fn serde_json_value(json: &str) -> serde_json::Value {
    serde_json::from_str(json).expect("json")
}
