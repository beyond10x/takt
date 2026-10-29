//! story:step-records-per-call-usage: a harness step's `step` line carries, besides `usage`,
//! what the harness reported for the call: `model`, `cost_usd`, `duration_ms`, `api_ms`,
//! `turns` and `started_at`, each absent when the harness did not state it; and a record
//! written before the fields existed still reads.

#[path = "support/own_home.rs"]
mod own_home;

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use takt::events::Events;
use takt::harness::ClaudeCode;
use takt::validator::Validator;
use takt::{Run, RunOptions};

/// The delivery protocol, the engineering protocols' built-in.
const PROTOCOL: &str = "story.delivery/1";

/// The run records kept as fixtures, relative to this crate.
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/trace");

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
        .join(format!("step-call-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(&dir).expect("a scratch tree");
    Scratch(dir)
}

fn git(tree: &Path, args: &[&str]) {
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
}

/// A repository with one gate that holds, committed.
fn repository(name: &str) -> Scratch {
    let tree = scratch(name);
    fs::create_dir_all(tree.join(".engineering")).expect("the engineering directory");
    fs::write(tree.join("README.md"), "a story\n").expect("a file");
    fs::write(
        tree.join(".engineering/gates.yaml"),
        "format: engineering-gates/1\nlanguage: canon-expr/1\ngates:\n  - id: check\n    \
         text: The check holds.\n    assertions:\n      - 'fs.file.exists(\"README.md\")'\n",
    )
    .expect("the gates");
    git(&tree, &["init", "-q", "-b", "main"]);
    git(&tree, &["add", "-A"]);
    git(&tree, &["commit", "-q", "-m", "start"]);
    tree
}

/// A fake `claude` that prints `reply` whatever it is asked.
fn fake_claude(dir: &Path, reply: &str) -> ClaudeCode {
    let program = dir.join("claude.sh");
    fs::write(&program, format!("#!/bin/sh\nprintf '%s' '{reply}'\n")).expect("a script");
    fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).expect("exec bit");
    ClaudeCode {
        program: program.display().to_string(),
        ..ClaudeCode::default()
    }
}

/// One judgement step on the delivery protocol with no validator; the harness step's line.
fn one_step(tree: &Path, harness: &ClaudeCode, case: &str) -> serde_json::Value {
    let validators: Vec<Box<dyn Validator>> = Vec::new();
    let options = RunOptions {
        protocol: PROTOCOL.to_owned(),
        intent: "deliver the story".to_owned(),
        inputs: Vec::new(),
        iterations: Some(1),
        hours: None,
        tokens: None,
        case_id: Some(case.to_owned()),
        gate: Some("repository.merge".to_owned()),
    };
    let mut run = Run::open(tree, options, harness, &validators).expect("the case opens");
    let mut out = Vec::new();
    let stopped = run.run(&mut out);
    let text = String::from_utf8_lossy(&out).into_owned();
    stopped.unwrap_or_else(|error| panic!("the step runs: {error:#}\n{text}"));
    let lines = Events::new(&run.case.dir).read().expect("the record reads");
    lines
        .iter()
        .find(|line| line["event"] == "step" && line["by"] == "harness")
        .unwrap_or_else(|| panic!("a harness step in {lines:#?}"))
        .clone()
}

/// The result line a fake prints: `claude --output-format json`'s shape, with the usage,
/// the cost, the turns, the two durations and the model.
const FULL_REPLY: &str = r#"{"type": "result", "result": "{\"action\": \"repository.inspect\", \"summary\": \"read\"}", "usage": {"input_tokens": 11, "output_tokens": 22, "cache_read_input_tokens": 33, "cache_creation_input_tokens": 44}, "total_cost_usd": 0.2787, "num_turns": 7, "duration_ms": 41885, "duration_api_ms": 41124, "modelUsage": {"claude-opus-5-5": {"inputTokens": 11, "outputTokens": 22, "costUSD": 0.2787}}}"#;

/// The same reply with none of the five call fields.
const BARE_REPLY: &str = r#"{"type": "result", "result": "{\"action\": \"repository.inspect\", \"summary\": \"read\"}", "usage": {"input_tokens": 11, "output_tokens": 22, "cache_read_input_tokens": 33, "cache_creation_input_tokens": 44}}"#;

/// A fake harness that prints `total_cost_usd`, `num_turns`, `duration_ms`, `duration_api_ms`
/// and a `modelUsage` key: the step line's `cost_usd`, `turns`, `duration_ms`, `api_ms` and
/// `model` equal what it printed, and its `started_at` precedes its `at`.
#[test]
fn the_step_line_carries_what_the_harness_reported_for_the_call() {
    let tree = repository("full-tree");
    let scripts = scratch("full-scripts");
    let harness = fake_claude(&scripts, FULL_REPLY);
    let step = one_step(&tree, &harness, "scenario-1-trial-1");
    assert_eq!(step["cost_usd"], 0.2787, "{step}");
    assert_eq!(step["turns"], 7, "{step}");
    assert_eq!(step["duration_ms"], 41885, "{step}");
    assert_eq!(step["api_ms"], 41124, "{step}");
    assert_eq!(step["model"], "claude-opus-5-5", "{step}");
    assert_eq!(step["tokens"], 110, "{step}");
    assert_eq!(
        step["usage"],
        serde_json::json!({"input": 11, "output": 22, "cache_read": 33, "cache_creation": 44}),
        "{step}"
    );
    let started_at = step["started_at"]
        .as_str()
        .unwrap_or_else(|| panic!("a started_at in {step}"));
    let at = step["at"].as_str().expect("an at");
    assert_eq!(
        started_at.len(),
        at.len(),
        "both to the microsecond: {step}"
    );
    assert!(
        started_at < at,
        "the call started before its line was written: {started_at} against {at}"
    );
}

/// A fake that prints none of the five fields: the step line has none of their keys, and no
/// key holds 0 in their place.
#[test]
fn a_harness_that_states_none_of_them_writes_a_step_line_without_the_keys() {
    let tree = repository("bare-tree");
    let scripts = scratch("bare-scripts");
    let harness = fake_claude(&scripts, BARE_REPLY);
    let step = one_step(&tree, &harness, "scenario-1-trial-2");
    let object = step.as_object().expect("an object");
    for key in ["model", "cost_usd", "duration_ms", "api_ms", "turns"] {
        assert!(
            !object.contains_key(key),
            "{key} is absent, not 0 or null: {step}"
        );
    }
    assert_eq!(step["tokens"], 110, "{step}");
    assert!(
        object.contains_key("started_at"),
        "takt stamps the start: {step}"
    );
}

/// A record written at a0b8576 (T-16, scenario 1), before the fields existed, still reads,
/// its harness steps carrying `tokens` and none of the new keys.
#[test]
fn a_record_written_before_the_fields_still_reads() {
    let dir = Path::new(FIXTURES).join("t-16");
    let lines = Events::new(&dir).read().expect("the T-16 record reads");
    assert_eq!(lines.len(), 54, "every line of the record reads");
    let steps: Vec<&serde_json::Value> = lines
        .iter()
        .filter(|line| line["event"] == "step" && line["by"] == "harness")
        .collect();
    assert_eq!(steps.len(), 3);
    for step in steps {
        assert!(step["tokens"].is_u64(), "{step}");
        for key in [
            "usage",
            "model",
            "cost_usd",
            "duration_ms",
            "api_ms",
            "turns",
            "started_at",
        ] {
            assert!(
                step.get(key).is_none(),
                "{key} was not recorded then: {step}"
            );
        }
    }
}
