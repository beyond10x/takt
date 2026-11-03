//! story:the-token-rule-holds-for-an-unchanged-binary: a scenario is worse on tokens when the
//! median over its cases of tokens per model request exceeds the saved side's same median by
//! more than 5 %. An identical binary measured with a harness whose requests grew a little, and
//! that drew one more request in one case, is not refused; a candidate whose every request is
//! 10 % dearer is. A case whose requests a harness step did not record is no figure, never 0:
//! the comparison refuses it, naming the case. Each step line's `requests` is what the
//! harness's stream shows: its distinct model messages.
//!
//! The figures are shaped like the story's: a saved side of median 122,355 tokens over seven
//! requests a case with a spread of 563, and a candidate whose median of 122,415 lies above the
//! saved maximum while its tokens per request do not move. They are shaped after the recorded
//! 0.2.2 and 44f96a48 scorecards, not read from them: those predate the requests and cannot be
//! compared.

#[path = "support/own_home.rs"]
mod own_home;

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use takt::case::CASES_DIR;
use takt::events::{EVENTS_FILE, Events};
use takt::harness::ClaudeCode;
use takt::scorecard::{HELD_OUT_FILE, Named, Scorecard};
use takt::validator::Validator;
use takt::{Run, RunOptions};

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
        .join(format!("token-rule-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(&dir).expect("a scratch tree");
    Scratch(dir)
}

/// One harness step of a case: its action, its tokens and the model requests it recorded
/// (`None`: the step line has no `requests`, as a line written before the field).
type Step = (&'static str, u64, Option<u64>);

/// The three Claude steps of a case whose run took `tokens` over `requests` model requests:
/// the requests spread over the steps as 2, 2 and the rest, the tokens in proportion.
fn three_steps(tokens: u64, requests: u64) -> Vec<Step> {
    let first = tokens * 2 / requests;
    vec![
        ("tests.write", first, Some(2)),
        ("implementation.edit", first, Some(2)),
        ("change.handover", tokens - 2 * first, Some(requests - 4)),
    ]
}

/// Writes the record of the accepted scenario case `case` under `repo`, one run of `steps`,
/// each a harness `step` line as the loop writes it, and its held-out result, a pass.
fn record(repo: &Path, case: &str, steps: &[Step]) {
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
        let mut line = serde_json::json!({
            "at": at(n + 1), "step": n + 1, "event": "step", "by": "harness",
            "name": "claude-code", "action": action, "summary": "done", "tokens": tokens,
        });
        if let Some(requests) = requests {
            line["requests"] = (*requests).into();
        }
        lines.push(line);
        total += tokens;
    }
    lines.push(serde_json::json!({
        "at": at(steps.len() + 1), "step": steps.len(), "event": "stop",
        "stop": "Completed (accepted)", "steps": steps.len(), "tokens": total,
    }));
    let text: String = lines.iter().map(|line| format!("{line}\n")).collect();
    fs::write(dir.join(EVENTS_FILE), text).expect("the record");
}

/// A repository holding scenario `scenario`'s cases, trials numbered from `first`, one a
/// figure of `cases`: each case's steps.
fn cases(repo: &Path, scenario: u32, first: u32, cases: &[Vec<Step>]) {
    for (n, steps) in cases.iter().enumerate() {
        record(
            repo,
            &format!("scenario-{scenario}-trial-{}", first + n as u32),
            steps,
        );
    }
}

/// The saved side: five cases of seven requests each, median 122,355 tokens, spread 563.
const SAVED: [u64; 5] = [121_850, 122_100, 122_355, 122_380, 122_413];

/// The tokens of one more model request at the size the story measured.
const ONE_MORE_REQUEST: u64 = 16_700;

/// Saves the scorecard of `repo` under `saves` and reads it back as a comparison does.
fn saved(repo: &Path, saves: &Path, revision: &str) -> Scorecard {
    let path = Scorecard::read(repo)
        .expect("the saved side reads")
        .save(saves, revision, Named::Build)
        .expect("the saved side is saved");
    Scorecard::load(&path).expect("the saved scorecard reads")
}

/// The comparison of `candidate`'s cases against `saved`: the number of scenarios worse and
/// what it printed.
fn compare(candidate: &Path, saved: &Scorecard) -> (usize, String) {
    let card = Scorecard::read(candidate).expect("the candidate reads");
    let mut out = Vec::new();
    let worse = card
        .against(saved, &mut out)
        .unwrap_or_else(|error| panic!("the comparison runs: {error:#}"));
    (worse, String::from_utf8(out).expect("utf-8"))
}

/// The acceptance, first line: two scenarios against the same saved side. Scenario 1 is the
/// identical binary as measured: its median, 122,415, lies above the saved maximum, 122,413, and
/// one case drew one request more (+16,700); its tokens per request barely move. Scenario 2 is
/// the same binary under a harness whose every request grew 3 %, again with one case one
/// request longer. Neither is worse on tokens.
#[test]
fn an_identical_binary_is_not_refused() {
    let base = scratch("identical-saved");
    for scenario in [1, 2] {
        let steps: Vec<Vec<Step>> = SAVED.iter().map(|&t| three_steps(t, 7)).collect();
        cases(&base, scenario, 1, &steps);
    }
    let saves = scratch("identical-saves");
    let saved = saved(&base, &saves, "44f96a4800000000000000000000000000000000");

    let candidate = scratch("identical-candidate");
    let measured: Vec<Vec<Step>> = [122_200, 122_400, 122_415, 122_500]
        .iter()
        .map(|&t| three_steps(t, 7))
        .chain([three_steps(122_415 + ONE_MORE_REQUEST, 8)])
        .collect();
    cases(&candidate, 1, 11, &measured);
    let drifted: Vec<Vec<Step>> = SAVED[..4]
        .iter()
        .map(|&t| three_steps(t * 103 / 100, 7))
        .chain([three_steps((SAVED[4] + ONE_MORE_REQUEST) * 103 / 100, 8)])
        .collect();
    cases(&candidate, 2, 11, &drifted);

    let (worse, text) = compare(&candidate, &saved);
    assert_eq!(
        worse, 0,
        "story:the-token-rule-holds-for-an-unchanged-binary: an identical binary is not worse on tokens\n{text}"
    );
    for line in [
        "tokens worse when the median tokens per request exceeds the saved median by more than 5 %",
        "scenario 1  candidate  n 5  tokens min 122200 median 122415 max 139115",
        "scenario 1  outcomes by rank held  tokens held  not worse",
        "scenario 2  outcomes by rank held  tokens held  not worse",
    ] {
        assert!(text.contains(line), "{line} in {text}");
    }
    assert!(
        text.contains(
            "scenario 1  saved      n 5  tokens per request min 17407.1 median 17479.3 max 17487.6"
        ),
        "{text}"
    );
}

/// The acceptance, second line: a candidate whose every request costs 10 % more than the saved
/// side's, over the same requests, is worse on tokens, and the line says by which medians.
#[test]
fn ten_percent_per_request_is_refused() {
    let base = scratch("dearer-saved");
    let steps: Vec<Vec<Step>> = SAVED.iter().map(|&t| three_steps(t, 7)).collect();
    cases(&base, 1, 1, &steps);
    let saves = scratch("dearer-saves");
    let saved = saved(&base, &saves, "44f96a4800000000000000000000000000000000");

    let candidate = scratch("dearer-candidate");
    let dearer: Vec<Vec<Step>> = SAVED
        .iter()
        .map(|&t| three_steps(t * 110 / 100, 7))
        .collect();
    cases(&candidate, 1, 11, &dearer);

    let (worse, text) = compare(&candidate, &saved);
    assert_eq!(worse, 1, "{text}");
    for line in [
        "scenario 1  median tokens per request 19227.1 exceeds the saved median 17479.3 by more than 5 %",
        "scenario 1  outcomes by rank held  tokens over  worse",
    ] {
        assert!(text.contains(line), "{line} in {text}");
    }
}

/// The tolerance is 5 % exactly: a median of tokens per request 5 % over the saved one holds,
/// one token more a request is over.
#[test]
fn five_percent_holds_and_one_token_more_is_over() {
    let base = scratch("edge-saved");
    let one = |tokens: u64| vec![("implementation.edit", tokens, Some(1))];
    cases(
        &base,
        1,
        1,
        &[one(1000), one(1000), one(1000), one(1000), one(1000)],
    );
    let saves = scratch("edge-saves");
    let saved = saved(&base, &saves, "1111111111111111111111111111111111111111");
    for (tokens, worse) in [(1050, 0), (1051, 1)] {
        let candidate = scratch(&format!("edge-{tokens}"));
        let steps: Vec<Vec<Step>> = (0..5).map(|_| one(tokens)).collect();
        cases(&candidate, 1, 11, &steps);
        let (counted, text) = compare(&candidate, &saved);
        assert_eq!(counted, worse, "{tokens} tokens a request\n{text}");
    }
}

/// A harness step that recorded no `requests` leaves its case's tokens per request unknown,
/// never a figure that counts it 0: the comparison refuses before anything is judged or
/// printed, naming the case and the side, whichever side it is on.
#[test]
fn a_case_without_its_requests_is_refused_on_either_side() {
    let known: Vec<Vec<Step>> = SAVED.iter().map(|&t| three_steps(t, 7)).collect();
    let mut unrecorded = known.clone();
    unrecorded[2][1].2 = None;

    let base = scratch("unknown-saved");
    cases(&base, 1, 1, &known);
    let saves = scratch("unknown-saves");
    let good = saved(&base, &saves, "2222222222222222222222222222222222222222");
    let candidate = scratch("unknown-candidate");
    cases(&candidate, 1, 11, &unrecorded);
    let card = Scorecard::read(&candidate).expect("the candidate reads");
    let mut out = Vec::new();
    let error = card
        .against(&good, &mut out)
        .expect_err("a case of unknown requests is refused");
    let error = format!("{error:#}");
    assert!(
        error.contains("scenario-1-trial-13 (this scorecard)") && error.contains("requests"),
        "{error}"
    );
    assert!(out.is_empty(), "nothing is printed before the refusal");

    let old = scratch("unknown-old-saved");
    cases(&old, 1, 1, &unrecorded);
    let old_saves = scratch("unknown-old-saves");
    let bad = saved(&old, &old_saves, "3333333333333333333333333333333333333333");
    let other = scratch("unknown-known-candidate");
    cases(&other, 1, 11, &known);
    let card = Scorecard::read(&other).expect("the candidate reads");
    let mut out = Vec::new();
    let error = card
        .against(&bad, &mut out)
        .expect_err("a saved case of unknown requests is refused");
    let error = format!("{error:#}");
    assert!(
        error.contains("scenario-1-trial-3 (the saved scorecard)") && error.contains("requests"),
        "{error}"
    );
}

/// A Claude Code step's line carries `requests`: the distinct model messages its stream shows
/// (a message written over several lines counted once), so a case's requests are the sum of
/// its steps'.
#[test]
fn a_step_line_counts_the_model_messages_of_its_stream() {
    let tree = scratch("stream-tree");
    fs::create_dir_all(tree.join(".engineering")).expect("the engineering directory");
    fs::write(tree.join("README.md"), "a story\n").expect("a file");
    fs::write(
        tree.join(".engineering/gates.yaml"),
        "format: engineering-gates/1\nlanguage: canon-expr/1\ngates:\n  - id: check\n    \
         text: The check holds.\n    assertions:\n      - 'fs.file.exists(\"README.md\")'\n",
    )
    .expect("the gates");
    for args in [
        &["init", "-q", "-b", "main"][..],
        &["add", "-A"],
        &["commit", "-q", "-m", "start"],
    ] {
        let output = Command::new("git")
            .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
            .args(args)
            .current_dir(&*tree)
            .output()
            .expect("git runs");
        assert!(output.status.success(), "git {args:?}");
    }
    let scripts = scratch("stream-scripts");
    let stream = [
        r#"{"type":"system","subtype":"init","claude_code_version":"2.1.0"}"#,
        r#"{"type":"assistant","message":{"id":"m1","usage":{"input_tokens":5,"output_tokens":1}}}"#,
        r#"{"type":"assistant","message":{"id":"m1","usage":{"input_tokens":5,"output_tokens":1}}}"#,
        r#"{"type":"user","message":{"content":[]}}"#,
        r#"{"type":"assistant","message":{"id":"m2","usage":{"input_tokens":6,"output_tokens":2}}}"#,
        r#"{"type":"result","result":"{\"action\": \"repository.inspect\", \"summary\": \"read\"}","usage":{"input_tokens":11,"output_tokens":3},"num_turns":5}"#,
    ];
    let program = scripts.join("claude.sh");
    let body: String = stream
        .iter()
        .map(|line| format!("printf '%s\\n' '{line}'\n"))
        .collect();
    fs::write(&program, format!("#!/bin/sh\n{body}")).expect("a script");
    fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).expect("exec bit");
    let harness = ClaudeCode {
        program: program.display().to_string(),
        ..ClaudeCode::default()
    };
    let validators: Vec<Box<dyn Validator>> = Vec::new();
    let options = RunOptions {
        protocol: "story.delivery/1".to_owned(),
        intent: "deliver the story".to_owned(),
        inputs: Vec::new(),
        iterations: Some(1),
        hours: None,
        tokens: None,
        case_id: Some("scenario-1-trial-1".to_owned()),
        gate: Some("repository.merge".to_owned()),
    };
    let mut run = Run::open(&tree, options, &harness, &validators).expect("the case opens");
    let mut out = Vec::new();
    let stopped = run.run(&mut out);
    stopped.unwrap_or_else(|error| {
        panic!(
            "the step runs: {error:#}\n{}",
            String::from_utf8_lossy(&out)
        )
    });
    let lines = Events::new(&run.case.dir).read().expect("the record reads");
    let step = lines
        .iter()
        .find(|line| line["event"] == "step" && line["by"] == "harness")
        .unwrap_or_else(|| panic!("a harness step in {lines:#?}"));
    assert_eq!(
        step["requests"], 2,
        "two model messages, not five turns: {step}"
    );
    assert_eq!(step["turns"], 5, "{step}");
}
