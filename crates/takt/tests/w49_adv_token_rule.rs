//! Adversary pass 1 on story:the-token-rule-holds-for-an-unchanged-binary. The token rule
//! divides a case's tokens by the model requests its harness steps recorded, so every harness a
//! comparison covers has to record them: under the session executor a step's transcript is
//! what the vendor program printed (the raw record metaharness kept, which replaces its events),
//! and `requests_in` reads that. It holds a scenario at exactly 5 % ("more than 5 %" is
//! worse); and it judges tokens per request, never the case total, so a scenario whose cases
//! each take a request more at an unchanged cost a request holds.

#[path = "support/own_home.rs"]
mod own_home;

use std::fs;
use std::path::{Path, PathBuf};

use takt::case::CASES_DIR;
use takt::events::EVENTS_FILE;
use takt::harness::requests_in;
use takt::scorecard::{HELD_OUT_FILE, Named, Scorecard};

/// A scratch directory under this crate's build directory, removed when dropped.
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

fn scratch(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("w49-adv-token-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("a scratch tree");
    Scratch(dir)
}

fn lines(records: &[serde_json::Value]) -> Vec<u8> {
    records
        .iter()
        .map(|record| format!("{record}\n"))
        .collect::<String>()
        .into_bytes()
}

/// Claude Code's own stream, as a `--harness claude` step keeps it: one `assistant` record per
/// content block, each repeating its message's id and usage. Two messages, two requests. This
/// holds; it is the control for the two below.
#[test]
fn requests_in_counts_claude_codes_messages_once_each() {
    let usage = serde_json::json!({"input_tokens": 50, "output_tokens": 10});
    let assistant = |id: &str| {
        serde_json::json!({"type": "assistant", "message": {"id": id, "usage": usage},
            "parent_tool_use_id": null, "session_id": "s-1"})
    };
    let stream = lines(&[
        serde_json::json!({"type": "system", "subtype": "init", "claude_code_version": "2.1.304"}),
        assistant("msg_1"),
        assistant("msg_1"),
        serde_json::json!({"type": "user", "message": {"content": []}}),
        assistant("msg_2"),
        assistant("msg_2"),
        serde_json::json!({"type": "result", "num_turns": 2, "result": "{}"}),
    ]);
    assert_eq!(requests_in(&stream), Some(2));
}

/// Codex's rollout, as a `--harness codex` step keeps it (the record shape of the rollouts takt
/// keeps under its transcripts directory): `session_meta`, `event_msg` and `response_item`
/// lines, a `token_count` event after each model response. Two responses, two requests; it
/// reads none, so the step line records no `requests` and every comparison over a Codex
/// scenario case is refused for unknown requests, where it compared before this change.
#[test]
fn requests_in_counts_a_codex_rollouts_model_responses() {
    let at = "2026-10-20T18:00:00.000Z";
    let count = |input: u64| {
        serde_json::json!({"timestamp": at, "type": "event_msg", "payload": {
            "type": "token_count", "info": {
                "total_token_usage": {"input_tokens": input, "cached_input_tokens": 40,
                    "output_tokens": 20, "reasoning_output_tokens": 0, "total_tokens": input + 20},
                "last_token_usage": {"input_tokens": 50, "cached_input_tokens": 20,
                    "output_tokens": 10, "reasoning_output_tokens": 0, "total_tokens": 60}}}})
    };
    let rollout = lines(&[
        serde_json::json!({"timestamp": at, "type": "session_meta", "payload": {
            "id": "01a15a19-57e0-7d83-97b1-da333ebb91e5", "cli_version": "0.153.4",
            "originator": "codex_exec", "source": "exec", "model_provider": "openai"}}),
        serde_json::json!({"timestamp": at, "type": "event_msg", "payload": {
            "type": "task_started", "turn_id": "t-1"}}),
        serde_json::json!({"timestamp": at, "type": "response_item", "payload": {
            "type": "custom_tool_call", "name": "apply_patch", "call_id": "c-1", "input": ""}}),
        count(50),
        serde_json::json!({"timestamp": at, "type": "response_item", "payload": {
            "type": "custom_tool_call_output", "call_id": "c-1", "output": "ok"}}),
        serde_json::json!({"timestamp": at, "type": "response_item", "payload": {
            "type": "message", "role": "assistant", "phase": "final_answer",
            "content": [{"type": "output_text", "text": "{}"}]}}),
        count(110),
        serde_json::json!({"timestamp": at, "type": "event_msg", "payload": {
            "type": "task_complete", "turn_id": "t-1", "last_agent_message": "{}"}}),
    ]);
    assert!(
        requests_in(&rollout).is_some(),
        "story:the-token-rule-holds-for-an-unchanged-binary: a Codex step of two model \
         responses records its requests, else every comparison over its case is refused"
    );
}

/// b10x-harness's record, as a `--harness harness` step keeps it: a `usage` record a model
/// response (`kind`, not `event`). Two responses; it reads none.
#[test]
fn requests_in_counts_a_b10x_harness_records_model_responses() {
    let record = lines(&[
        serde_json::json!({"kind": "started", "model": "scripted"}),
        serde_json::json!({"kind": "turn-started", "turn": 1}),
        serde_json::json!({"kind": "usage", "model": "scripted",
            "input_tokens": 50, "output_tokens": 10, "cached_input_tokens": 20}),
        serde_json::json!({"kind": "turn-started", "turn": 2}),
        serde_json::json!({"kind": "usage", "model": "scripted",
            "input_tokens": 60, "output_tokens": 10, "cached_input_tokens": 20}),
        serde_json::json!({"kind": "finished", "stop": {"kind": "completed"}, "turns": 2}),
    ]);
    assert!(
        requests_in(&record).is_some(),
        "story:the-token-rule-holds-for-an-unchanged-binary: a b10x-harness step of two model \
         responses records its requests, else every comparison over its case is refused"
    );
}

/// One harness step of a case: its action, tokens and model requests.
type Step = (&'static str, u64, u64);

/// Writes the accepted scenario case `case` under `repo`, one run of `steps`.
fn write_case(repo: &Path, case: &str, steps: &[Step]) {
    let dir = repo.join(CASES_DIR).join(case);
    fs::create_dir_all(&dir).expect("a case directory");
    fs::write(dir.join(HELD_OUT_FILE), "PASS\n").expect("the held-out result");
    let at = |n: usize| format!("2026-10-19T10:00:{n:02}.000000Z");
    let mut lines = vec![serde_json::json!({
        "at": at(0), "step": 0, "event": "opened", "case": case,
        "protocol": "story.delivery/3", "inputs": [],
    })];
    let mut total = 0;
    for (n, (action, tokens, requests)) in steps.iter().enumerate() {
        lines.push(serde_json::json!({
            "at": at(n + 1), "step": n + 1, "event": "step", "by": "harness",
            "name": "claude-code", "action": action, "summary": "done", "tokens": tokens,
            "requests": requests,
        }));
        total += tokens;
    }
    lines.push(serde_json::json!({
        "at": at(steps.len() + 1), "step": steps.len(), "event": "stop",
        "stop": "Completed (accepted)", "steps": steps.len(), "tokens": total,
    }));
    let text: String = lines.iter().map(|line| format!("{line}\n")).collect();
    fs::write(dir.join(EVENTS_FILE), text).expect("the record");
}

fn write_cases(repo: &Path, first: u32, cases: &[Vec<Step>]) {
    for (n, steps) in cases.iter().enumerate() {
        write_case(
            repo,
            &format!("scenario-1-trial-{}", first + n as u32),
            steps,
        );
    }
}

fn saved(repo: &Path, saves: &Path, revision: &str) -> Scorecard {
    let path = Scorecard::read(repo)
        .expect("the saved side reads")
        .save(saves, revision, Named::Build)
        .expect("the saved side is saved");
    Scorecard::load(&path).expect("the saved scorecard reads")
}

fn compare(candidate: &Path, saved: &Scorecard) -> (usize, String) {
    let card = Scorecard::read(candidate).expect("the candidate reads");
    let mut out = Vec::new();
    let worse = card
        .against(saved, &mut out)
        .unwrap_or_else(|error| panic!("the comparison runs: {error:#}"));
    (worse, String::from_utf8(out).expect("utf-8"))
}

/// The rule is "more than 5 %". Saved cases of 1,000 tokens over 3 requests (333.33... a
/// request) and candidate cases of 1,050 tokens over the same 3 requests (350 a request) are
/// exactly 5 % apart, which holds. `milli_per_request` floors the saved side to 333.333 before
/// the cross-multiplication, so the candidate reads as 5.00001 % over and is refused.
#[test]
fn exactly_five_percent_over_a_fractional_saved_median_holds() {
    let base = scratch("exact-saved");
    let one = |tokens: u64| vec![("implementation.edit", tokens, 3)];
    write_cases(&base, 1, &vec![one(1000); 5]);
    let saves = scratch("exact-saves");
    let saved = saved(&base, &saves, "4444444444444444444444444444444444444444");
    let candidate = scratch("exact-candidate");
    write_cases(&candidate, 11, &vec![one(1050); 5]);
    let (worse, text) = compare(&candidate, &saved);
    assert_eq!(
        worse, 0,
        "story:the-token-rule-holds-for-an-unchanged-binary: 1050/3 against 1000/3 is exactly 5 %, \
         which is not more than 5 %\n{text}"
    );
}

/// The story's case: a step that makes one request more costs about one request's tokens.
/// Three of five candidate cases take an eighth request at the cost of the other seven, so
/// their totals are 14 % over and their tokens per request unchanged: the rule judges per
/// request and holds. A rule that judged the median of case totals within 5 % (the mutant
/// `token_rule.rs` does not catch: one case of five drawing a request more never moves a
/// median) refuses it.
#[test]
fn most_cases_one_request_longer_at_the_same_cost_hold() {
    const SAVED: [u64; 5] = [121_850, 122_100, 122_355, 122_380, 122_413];
    let base = scratch("longer-saved");
    let seven = |tokens: u64| vec![("implementation.edit", tokens, 7)];
    let eight = |tokens: u64| vec![("implementation.edit", tokens * 8 / 7, 8)];
    write_cases(
        &base,
        1,
        &SAVED.iter().map(|&t| seven(t)).collect::<Vec<_>>(),
    );
    let saves = scratch("longer-saves");
    let saved = saved(&base, &saves, "5555555555555555555555555555555555555555");
    let candidate = scratch("longer-candidate");
    let cases: Vec<Vec<Step>> = vec![
        seven(SAVED[0]),
        seven(SAVED[1]),
        eight(SAVED[2]),
        eight(SAVED[3]),
        eight(SAVED[4]),
    ];
    write_cases(&candidate, 11, &cases);
    let (worse, text) = compare(&candidate, &saved);
    assert_eq!(
        worse, 0,
        "story:the-token-rule-holds-for-an-unchanged-binary: a request more at the same cost a \
         request is not worse on tokens\n{text}"
    );
    assert!(
        text.contains("scenario 1  outcomes by rank held  tokens held  not worse"),
        "{text}"
    );
}
