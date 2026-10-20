//! story:run-trace-and-report: a case's run record renders as one `takt-trace/1` file, and
//! the report prints where its time and tokens went. The fixtures are scrubbed records of
//! real runs: trial 1 (a worktree unit at a0b8576, `docs/runs/2026-10-18-trial-1-worktree.md`),
//! T-24 (scenario 1 at 834205f) and T-16 (scenario 1 at a0b8576), each with the runner shim's
//! `steps.jsonl` reduced to the fields the trace reads.

use std::fs;
use std::path::{Path, PathBuf};

use takt::case::CASES_DIR;
use takt::events::{Event, Events};
use takt::report::{self, Basis};
use takt::scorecard::Scorecard;
use takt::trace::{Actor, Evidence, Kind, Trace};

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
    let dir =
        Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("trace-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(&dir).expect("a scratch tree");
    Scratch(dir)
}

fn fixture(name: &str) -> PathBuf {
    Path::new(FIXTURES).join(name)
}

/// The fixture's trace, with its steps file when `with_steps`.
fn trace(name: &str, with_steps: bool) -> Trace {
    let dir = fixture(name);
    let steps = dir.join("steps.jsonl");
    Trace::from_case(&dir, with_steps.then_some(steps.as_path()), None)
        .unwrap_or_else(|error| panic!("{name} traces: {error:#}"))
}

/// The report of `trace` as text.
fn report_text(trace: &Trace) -> String {
    let mut out = Vec::new();
    report::print(std::slice::from_ref(trace), &mut out).expect("prints");
    String::from_utf8(out).expect("utf-8")
}

/// The trace as the file `takt trace` writes.
fn trace_text(trace: &Trace) -> String {
    let mut out = Vec::new();
    trace.write(&mut out).expect("writes");
    String::from_utf8(out).expect("utf-8")
}

/// On trial 1's case the report prints validators 107 s, model calls 130 s and other 5 s,
/// the figures of `docs/runs/2026-10-18-trial-1-worktree.md`, with the shim's `steps.jsonl`
/// supplying the calls' durations; 3 model calls, 33 turns, 592,698 tokens, $0.849.
#[test]
fn trial_1_attribution_matches_the_run_record() {
    let trace = trace("trial-1", true);
    let a = report::attribution(&trace);
    assert_eq!(a.wall_s, Some(242.0), "{a:?}");
    assert_eq!(a.of(Actor::Validator).map(f64::round), Some(107.0), "{a:?}");
    assert_eq!(a.of(Actor::Harness).map(f64::round), Some(130.0), "{a:?}");
    assert_eq!(a.of(Actor::Takt).map(f64::round), Some(5.0), "{a:?}");
    assert_eq!(a.of(Actor::Forge), Some(0.0), "{a:?}");
    assert_eq!(a.of(Actor::Idle), Some(0.0), "{a:?}");
    assert_eq!(a.basis, Basis::Reported);
    assert_eq!(a.api_s.map(f64::round), Some(128.0), "{a:?}");
    assert_eq!((a.steps, a.model_calls, a.turns), (6, 3, Some(33)), "{a:?}");
    let header = &trace.header;
    assert_eq!(header.tokens.sum, Some(592_698));
    assert_eq!(
        (
            header.tokens.input,
            header.tokens.output,
            header.tokens.cache_read,
            header.tokens.cache_write
        ),
        (Some(48), Some(12_596), Some(518_400), Some(61_654))
    );
    let cost = header.cost_usd.expect("a cost from the steps file");
    assert!((cost - 0.849_024).abs() < 0.000_5, "{cost}");
    assert_eq!(header.model.as_deref(), Some("claude-opus-5-5"));
    assert_eq!(header.harness.as_deref(), Some("claude-code"));
    assert_eq!(
        header.stop.as_deref(),
        Some("ApprovalRequired (repository.merge)")
    );
    assert_eq!(header.outcome, "ApprovalRequired");
    assert_eq!((header.invocations, header.resumes), (1, 0));
    assert_eq!(
        header.notes,
        vec![
            "steps file: model, cost_usd, turns, duration_ms, api_ms, token components for 3 of 3 calls"
                .to_owned()
        ]
    );
}

/// The report text on trial 1 is the fixture beside the record, byte for byte.
#[test]
fn trial_1_report_matches_the_fixture_text() {
    let trace = trace("trial-1", true);
    let expected =
        fs::read_to_string(fixture("trial-1").join("expected.txt")).expect("the fixture");
    let printed = report_text(&trace);
    assert_eq!(printed, expected, "\n--- printed ---\n{printed}");
}

/// Without the steps file the record states no duration, so the harness's seconds are the
/// steps' spans (135 s: a call's process start and takt's commit fall inside), and the model,
/// cost, turns, durations and token components are listed as not recorded.
#[test]
fn trial_1_without_the_steps_file_uses_the_spans_and_says_what_is_missing() {
    let trace = trace("trial-1", false);
    let a = report::attribution(&trace);
    assert_eq!(a.basis, Basis::Spans);
    assert_eq!(a.of(Actor::Harness).map(f64::round), Some(135.0), "{a:?}");
    assert_eq!(a.of(Actor::Validator).map(f64::round), Some(107.0), "{a:?}");
    assert_eq!(a.of(Actor::Takt).map(f64::round), Some(0.0), "{a:?}");
    assert_eq!(a.api_s, None);
    assert_eq!(a.turns, None);
    assert_eq!(trace.header.model, None);
    assert_eq!(trace.header.cost_usd, None);
    assert_eq!(trace.header.tokens.sum, Some(592_698));
    assert_eq!(trace.header.tokens.input, None);
    assert!(trace.header.notes.is_empty(), "{:?}", trace.header.notes);
    assert_eq!(
        report::not_recorded(&trace),
        vec![
            "acceptance",
            "takt revision",
            "model",
            "cost",
            "turns",
            "call durations",
            "api seconds",
            "token components"
        ]
    );
    let text = report_text(&trace);
    assert!(
        text.contains("| harness | 135 | 56% |"),
        "the spans stand in for the durations:\n{text}"
    );
    assert!(
        text.contains("harness seconds: the step spans in the record"),
        "{text}"
    );
}

/// T-24's trace: 18 step operations, 2 `idle` operations of 30 s, 3 model calls with four
/// token components (recorded at 834205f), the forge's steps as `forge`, the second CI pass
/// and the approval-only call labelled; 21 operations with the stop.
#[test]
fn t_24_trace_holds_the_steps_the_idle_waits_and_the_calls() {
    let trace = trace("t-24", false);
    let steps: Vec<_> = trace.ops.iter().filter(|op| op.is_step()).collect();
    assert_eq!(steps.len(), 18, "{:#?}", trace.ops);
    assert_eq!(trace.header.steps, 18);
    let idle: Vec<_> = trace
        .ops
        .iter()
        .filter(|op| op.actor == Actor::Idle)
        .collect();
    assert_eq!(idle.len(), 2, "{:#?}", trace.ops);
    for op in &idle {
        let seconds = op.seconds().expect("both ends");
        assert!((seconds - 30.0).abs() < 0.1, "{op:?}");
        assert_eq!(op.kind, Kind::CiWait, "{op:?}");
    }
    let calls = trace.calls();
    assert_eq!(calls.len(), 3);
    for call in &calls {
        let tokens = call
            .tokens
            .unwrap_or_else(|| panic!("four components on {call:?}"));
        assert_eq!(call.tokens_sum, Some(tokens.sum()), "{call:?}");
        assert_eq!(call.evidence, Some(Evidence::Declared));
    }
    assert_eq!(trace.header.tokens.sum, Some(122_095));
    assert_eq!(trace.header.tokens.input, Some(16));
    assert_eq!(trace.header.tokens.output, Some(3_311));
    assert_eq!(trace.header.tokens.cache_read, Some(98_707));
    assert_eq!(trace.header.tokens.cache_write, Some(20_061));
    assert_eq!(trace.ops.len(), 21, "18 steps, 2 waits, the stop");
    let stop = trace.ops.last().expect("a stop");
    assert_eq!((stop.actor, stop.kind), (Actor::Human, Kind::ApprovalWait));
    assert_eq!(stop.seconds(), Some(0.0));
    let forge: Vec<_> = steps
        .iter()
        .filter(|op| op.actor == Actor::Forge)
        .map(|op| op.action.clone().unwrap_or_default())
        .collect();
    assert_eq!(forge.len(), 9, "{forge:?}");
    assert!(
        forge
            .iter()
            .all(|action| action == "pr.open" || action == "ci.read")
    );
    let labels: Vec<(u64, String)> = trace
        .ops
        .iter()
        .filter_map(|op| op.label.clone().map(|label| (op.step, label)))
        .collect();
    assert_eq!(
        labels,
        vec![
            (3, "seed pr.open".to_owned()),
            (17, "2nd CI".to_owned()),
            (18, "approval-only call".to_owned()),
        ]
    );
    assert_eq!(trace.header.scenario.as_deref(), Some("1"));
    assert_eq!(trace.header.binary.as_deref(), Some("unknown revision"));
    assert_eq!(trace.header.revision, None);
    let a = report::attribution(&trace);
    assert_eq!(a.of(Actor::Idle).map(f64::round), Some(60.0), "{a:?}");
    assert_eq!(a.wall_s.map(f64::round), Some(123.0), "{a:?}");
    // The CI cost: five `ci.read` steps (4, 9, 10, 15, 17; two of them found nothing to
    // read), 5.263 s of step spans, and the two 30 s polls on the forge before reads 10 and
    // 17, 60.007 s: what a read costs the run is (5.263 + 60.007) / 5, about 13.1 s.
    assert_eq!(a.ci_reads, 5, "{a:?}");
    let read_s = a.ci_read_s.value().expect("every read has a span");
    assert!((read_s - 5.263).abs() < 0.002, "{a:?}");
    assert_eq!(a.ci_waits, 2, "{a:?}");
    let wait_s = a.ci_wait_s.value().expect("every wait has a span");
    assert!((wait_s - 60.007).abs() < 0.002, "{a:?}");
    let per = a.per_ci_read().expect("reads");
    assert!((per - 13.054).abs() < 0.001, "{per}");
    // Two CI cycles: the pull request opened at step 8 and passed at 10; the push at 14 was
    // replaced by the push at 16 before a pass, which passed at 17.
    assert_eq!(a.ci_cycles, 2, "{a:?}");
    let text = report_text(&trace);
    assert!(text.contains("\nci reads: 5\nci cycles: 2\n"), "{text}");
    assert!(
        text.contains(
            "\nseconds per ci read: 13.1; 5.3 s over 5 reads; waits on ci.read between them: 2, 60.0 s\n"
        ),
        "{text}"
    );
    assert!(
        text.contains("| 5 | 13.1 | not recorded |\n"),
        "the cross-run columns: {text}"
    );
}

/// With the shim's file, T-24's calls gain model, cost, turns and durations, and the token
/// components stay the record's own.
#[test]
fn t_24_with_the_steps_file_gains_the_calls_fields_and_keeps_its_tokens() {
    let trace = trace("t-24", true);
    let a = report::attribution(&trace);
    assert_eq!(a.basis, Basis::Reported);
    assert_eq!(a.turns, Some(14));
    assert_eq!(trace.header.model.as_deref(), Some("claude-opus-5-5"));
    let cost = trace.header.cost_usd.expect("a cost");
    assert!((cost - 0.246_513).abs() < 0.000_5, "{cost}");
    assert_eq!(trace.header.tokens.sum, Some(122_095));
    assert_eq!(
        trace.header.notes,
        vec!["steps file: model, cost_usd, turns, duration_ms, api_ms for 3 of 3 calls".to_owned()]
    );
}

/// For trial 1 and T-24 the trace's token sum equals `takt scorecard`'s for the same case.
#[test]
fn the_token_sum_equals_the_scorecards() {
    for name in ["trial-1", "t-24", "t-16"] {
        let trace = trace(name, false);
        let repo = scratch(&format!("scorecard-{name}"));
        let case = repo.join(CASES_DIR).join(&trace.header.case);
        fs::create_dir_all(&case).expect("a case directory");
        fs::copy(
            fixture(name).join("events.jsonl"),
            case.join("events.jsonl"),
        )
        .expect("a copy");
        fs::write(case.join(takt::scorecard::HELD_OUT_FILE), "PASS\n")
            .expect("the held-out result");
        let card = Scorecard::read(&repo).expect("the scorecard reads");
        assert_eq!(card.rows.len(), 1, "{name}: {card:?}");
        assert_eq!(card.rows[0].tokens, trace.header.tokens.sum, "{name}");
        assert_eq!(card.rows[0].steps, trace.header.steps, "{name}");
        assert_eq!(card.rows[0].outcome, trace.header.outcome, "{name}");
        let wall = trace.header.wall_s.expect("a wall time");
        assert!(
            ((wall * 1000.0).round() - card.rows[0].wall_ms as f64).abs() < 1.0,
            "{name}: {wall} against {}",
            card.rows[0].wall_ms
        );
    }
}

/// No line of a trace names a home directory: summaries are classified, not copied, and the
/// sources are named relative to the case directory.
#[test]
fn no_trace_line_names_a_home_directory() {
    let home = std::env::var("HOME").expect("HOME");
    for name in ["trial-1", "t-24", "t-16"] {
        let text = trace_text(&trace(name, true));
        for (n, line) in text.lines().enumerate() {
            assert!(!line.contains("/home/"), "{name} line {}: {line}", n + 1);
            assert!(!line.contains(&home), "{name} line {}: {line}", n + 1);
        }
        let first: serde_json::Value =
            serde_json::from_str(text.lines().next().expect("a header")).expect("the header reads");
        assert_eq!(first["type"], "header");
        assert_eq!(first["format"], "takt-trace/1");
        assert_eq!(first["arm"], "takt");
    }
}

/// T-16's record, written at a0b8576 with whole-second instants and `tokens` only, traces:
/// its three calls carry a token sum and no components until the steps file supplies them.
#[test]
fn a_record_written_before_the_fields_traces() {
    let bare = trace("t-16", false);
    assert_eq!(bare.calls().len(), 3);
    assert!(
        bare.calls()
            .iter()
            .all(|op| op.tokens.is_none() && op.tokens_sum.is_some())
    );
    assert_eq!(bare.header.tokens.input, None);
    assert!(bare.header.tokens.sum.is_some_and(|sum| sum > 0));
    assert!(bare.header.wall_s.is_some_and(|wall| wall > 0.0));
    let full = trace("t-16", true);
    assert!(full.calls().iter().all(|op| op.tokens.is_some()));
    assert_eq!(
        full.header.tokens.sum, bare.header.tokens.sum,
        "the record's sum stands whatever the steps file says"
    );
    assert_eq!(report::attribution(&full).basis, Basis::Reported);
}

/// A trace written to a file and loaded again is the same trace, so `takt report --trace`
/// reads what `takt trace --out` wrote.
#[test]
fn a_trace_written_then_loaded_is_the_same() {
    let dir = scratch("roundtrip");
    let trace = trace("t-24", true);
    let path = dir.join("traces").join("t-24.jsonl");
    trace.save(&path).expect("saves");
    let loaded = Trace::load(&path).expect("loads");
    assert_eq!(loaded, trace);
    assert_eq!(report_text(&loaded), report_text(&trace));
    let not_a_trace = dir.join("x.jsonl");
    fs::write(&not_a_trace, "{\"type\":\"op\"}\n").expect("a file");
    let error = Trace::load(&not_a_trace).expect_err("refused");
    assert!(format!("{error:#}").contains("x.jsonl"), "{error:#}");
}

/// A case directory without a record is refused by name.
#[test]
fn a_case_without_a_record_is_refused() {
    let dir = scratch("no-record");
    let error = Trace::from_case(&dir, None, None).expect_err("refused");
    assert!(format!("{error:#}").contains("events.jsonl"), "{error:#}");
}

/// Two validator failures with the same text (durations aside) and nothing new between them
/// are a defect: the second is labelled `repeated failure (as step N)` and the report names
/// it.
#[test]
fn a_repeated_validator_failure_is_labelled_and_reported() {
    let lines = vec![
        serde_json::json!({"at": "2026-10-19T10:00:00.000000Z", "step": 0, "event": "opened", "case": "takt-x", "protocol": "software.change/1", "inputs": []}),
        serde_json::json!({"at": "2026-10-19T10:00:10.000000Z", "step": 1, "event": "step", "by": "validator", "name": "takt.tests-run", "action": "tests.run", "summary": "the test command exited 101: fail; test result: FAILED. 6 passed; 2 failed; finished in 7.89s"}),
        serde_json::json!({"at": "2026-10-19T10:00:10.000100Z", "step": 1, "event": "evidence", "id": "test-result-1", "kind": "test_result", "result": "fail", "path": "evidence/0001-test-result-1.json"}),
        serde_json::json!({"at": "2026-10-19T10:00:40.000000Z", "step": 2, "event": "step", "by": "harness", "name": "claude-code", "action": "implementation.edit", "summary": "edited", "tokens": 1000, "usage": {"input": 100, "output": 200, "cache_read": 300, "cache_creation": 400}, "model": "claude-opus-5-5", "cost_usd": 0.5, "duration_ms": 28000, "api_ms": 27000, "turns": 4, "started_at": "2026-10-19T10:00:10.500000Z"}),
        serde_json::json!({"at": "2026-10-19T10:00:40.000100Z", "step": 2, "event": "effect", "paths": ["src/lib.rs"], "revision": "0123456789abcdef0123456789abcdef01234567"}),
        serde_json::json!({"at": "2026-10-19T10:00:50.000000Z", "step": 3, "event": "step", "by": "validator", "name": "takt.tests-run", "action": "tests.run", "summary": "the test command exited 101: fail; test result: FAILED. 6 passed; 2 failed; finished in 0.12s"}),
        serde_json::json!({"at": "2026-10-19T10:00:50.000100Z", "step": 3, "event": "evidence", "id": "test-result-3", "kind": "test_result", "result": "fail", "path": "evidence/0002-test-result-3.json"}),
        serde_json::json!({"at": "2026-10-19T10:01:00.000000Z", "step": 4, "event": "step", "by": "harness", "name": "claude-code", "action": "repository.inspect", "summary": "read", "tokens": 500}),
        serde_json::json!({"at": "2026-10-19T10:01:00.000100Z", "step": 4, "event": "effect", "paths": [], "revision": null}),
        serde_json::json!({"at": "2026-10-19T10:01:00.000200Z", "step": 4, "event": "stop", "stop": "NoProgress", "steps": 4, "tokens": 1500}),
    ];
    let trace = Trace::from_lines(
        &lines,
        None,
        "takt-x",
        Some("synthetic"),
        vec!["events.jsonl".to_owned()],
    );
    let labels: Vec<(u64, String)> = trace
        .ops
        .iter()
        .filter_map(|op| op.label.clone().map(|label| (op.step, label)))
        .collect();
    assert_eq!(
        labels,
        vec![
            (3, "repeated failure (as step 1)".to_owned()),
            (4, "inspect, then NoProgress".to_owned()),
        ]
    );
    let call = &trace.ops[1];
    assert_eq!((call.actor, call.kind), (Actor::Harness, Kind::Edit));
    assert_eq!(call.outcome.as_deref(), Some("changed"));
    assert_eq!(call.paths, vec!["src/lib.rs".to_owned()]);
    assert_eq!(call.model.as_deref(), Some("claude-opus-5-5"));
    assert_eq!(
        (call.cost_usd, call.duration_ms, call.api_ms, call.turns),
        (Some(0.5), Some(28_000), Some(27_000), Some(4))
    );
    assert_eq!(call.tokens.map(|tokens| tokens.cache_write), Some(400));
    let inspect = &trace.ops[3];
    assert_eq!(
        (inspect.kind, inspect.outcome.as_deref()),
        (Kind::Read, Some("unchanged"))
    );
    assert_eq!(trace.header.outcome, "NoProgress");
    assert_eq!(trace.header.run, "synthetic");
    assert_eq!(trace.header.tokens.sum, Some(1500));
    assert_eq!(
        trace.header.cost_usd, None,
        "one call states no cost, so the sum is unknown"
    );
    let text = report_text(&trace);
    assert!(
        text.contains("- step 3: repeated failure (as step 1) (10.0 s)"),
        "{text}"
    );
    assert!(
        text.contains("- step 4: inspect, then NoProgress"),
        "{text}"
    );
    assert!(
        text.contains("| synthetic | software.change/1 | - | NoProgress | 60 |"),
        "{text}"
    );
}

/// A record whose text a harness shaped cannot drive the terminal the report is read on, and
/// no local path travels in the trace: every text a record contributes (the run's name, case,
/// model, stop, outcome, action, protocol, story, the harness's name, a refusal's `by`, the
/// paths) passes one escape, in the trace file as in the report, which writes a C0 or C1
/// control character and DEL as `\u{..}`, and the home directory as `~`.
#[test]
fn the_report_escapes_every_text_a_record_contributes() {
    let home = std::env::var("HOME").ok().filter(|home| home.len() > 1);
    let under_home = home
        .as_deref()
        .map_or_else(|| "/elsewhere/x/y".to_owned(), |home| format!("{home}/x/y"));
    let lines = vec![
        serde_json::json!({"at": "2026-10-19T10:00:00.000000Z", "step": 0, "event": "opened", "case": "takt-esc", "protocol": "software.change/1\u{9b}31m", "story": "story:\u{7f}x", "inputs": []}),
        serde_json::json!({"at": "2026-10-19T10:00:10.000000Z", "step": 1, "event": "step", "by": "validator", "name": "takt.tests-run", "action": "tests.run\u{1b}[2J", "summary": "the test command exited 0: pass"}),
        serde_json::json!({"at": "2026-10-19T10:00:40.000000Z", "step": 2, "event": "step", "by": "harness", "name": "claude-code\u{1b}[31m", "action": "implementation.edit", "summary": "edited", "tokens": 1000, "model": "\u{1b}[31mclaude-opus-5-5\u{1b}[0m"}),
        serde_json::json!({"at": "2026-10-19T10:00:40.000100Z", "step": 2, "event": "refusal", "by": "takt\u{1b}]0;title\u{7}", "reason": "no"}),
        serde_json::json!({"at": "2026-10-19T10:00:41.000000Z", "step": 2, "event": "stop", "stop": format!("Error (read {under_home} \u{1b}[2J)"), "steps": 2, "tokens": 1000}),
    ];
    let trace = Trace::from_lines(
        &lines,
        None,
        "takt-esc",
        Some("esc\u{1b}[0m"),
        vec!["events.jsonl".to_owned()],
    );
    let shown = if home.is_some() {
        "~/x/y".to_owned()
    } else {
        under_home.clone()
    };
    let expected_stop = format!("Error (read {shown} \\u{{1b}}[2J)");
    assert_eq!(trace.header.run, "esc\\u{1b}[0m");
    assert_eq!(trace.header.stop.as_deref(), Some(expected_stop.as_str()));
    assert_eq!(trace.header.outcome, "Error");
    assert_eq!(
        trace.header.model.as_deref(),
        Some("\\u{1b}[31mclaude-opus-5-5\\u{1b}[0m")
    );
    assert_eq!(
        trace.header.harness.as_deref(),
        Some("claude-code\\u{1b}[31m")
    );
    assert_eq!(
        trace.header.protocol.as_deref(),
        Some("software.change/1\\u{9b}31m")
    );
    assert_eq!(trace.header.story.as_deref(), Some("story:\\u{7f}x"));
    assert_eq!(trace.ops[0].action.as_deref(), Some("tests.run\\u{1b}[2J"));
    let refusal = trace
        .ops
        .iter()
        .find(|op| op.kind == Kind::Refusal)
        .expect("a refusal");
    assert_eq!(refusal.name.as_deref(), Some("takt\\u{1b}]0;title\\u{7}"));
    let file = trace_text(&trace);
    for (n, line) in file.lines().enumerate() {
        assert!(
            !line.contains(&under_home),
            "line {}: a local path travels: {line}",
            n + 1
        );
        assert!(
            !line.bytes().any(|byte| byte < 0x20 || byte == 0x7f),
            "line {}: {line}",
            n + 1
        );
    }
    if home.is_some() {
        let header = file.lines().next().expect("a header");
        assert!(header.contains("~/x/y"), "{header}");
    }
    let text = report_text(&trace);
    for (n, byte) in text.bytes().enumerate() {
        assert!(
            byte == b'\n' || (byte >= 0x20 && byte != 0x7f),
            "byte {byte:#x} at {n} of:\n{text}"
        );
    }
    assert!(
        !text.contains('\u{9b}'),
        "a C1 control reached the output:\n{text}"
    );
    assert!(text.contains("### esc\\u{1b}[0m\n"), "{text}");
    assert!(
        text.contains(
            "harness claude-code\\u{1b}[31m; model \\u{1b}[31mclaude-opus-5-5\\u{1b}[0m;"
        ),
        "{text}"
    );
    assert!(
        text.contains("| software.change/1\\u{9b}31m | story:\\u{7f}x | Error |"),
        "{text}"
    );
    assert!(
        text.contains("| 1 | validator | tests.run\\u{1b}[2J |"),
        "{text}"
    );
    assert!(
        text.contains("- step 2: refusal by takt\\u{1b}]0;title\\u{7}\n"),
        "{text}"
    );
    if let Some(home) = &home {
        assert!(
            text.contains("; stop Error (read ~/x/y \\u{1b}[2J)\n"),
            "{text}"
        );
        assert!(
            !text.contains(home.as_str()),
            "the home directory is named:\n{text}"
        );
    }
}

/// The header's sources name the files relative to the case directory: `events.jsonl`, and
/// `steps.jsonl` when it lies beside the record; a steps file elsewhere by its file name
/// alone; the same for a case given by a relative path. No source is an absolute path or one
/// under `~`.
#[test]
fn sources_are_named_relative_to_the_case_directory() {
    let trace = trace("trial-1", true);
    assert_eq!(
        trace.header.sources,
        vec!["events.jsonl".to_owned(), "steps.jsonl".to_owned()]
    );
    let dir = scratch("sources");
    let elsewhere = dir.join("shim").join("t1-steps.jsonl");
    fs::create_dir_all(elsewhere.parent().expect("a parent")).expect("a directory");
    fs::copy(fixture("trial-1").join("steps.jsonl"), &elsewhere).expect("a copy");
    let trace =
        Trace::from_case(&fixture("trial-1"), Some(elsewhere.as_path()), None).expect("traces");
    assert_eq!(
        trace.header.sources,
        vec!["events.jsonl".to_owned(), "t1-steps.jsonl".to_owned()]
    );
    let relative = Path::new("tests/fixtures/trace/trial-1");
    let steps = relative.join("steps.jsonl");
    let trace =
        Trace::from_case(relative, Some(steps.as_path()), None).expect("a relative case traces");
    assert_eq!(
        trace.header.sources,
        vec!["events.jsonl".to_owned(), "steps.jsonl".to_owned()]
    );
    for source in &trace.header.sources {
        assert!(
            !source.starts_with('/') && !source.starts_with('~'),
            "{source}"
        );
    }
}

/// A record with no instants and a call that states no tokens: a figure no source states
/// prints `unknown`, never 0. One `ci.read` with no span is `unknown (0.0 s over 0 of 1
/// read)`, never `0.0 s over 1 read`; the tokens are `unknown`; takt's remainder is unknown;
/// a sum over no operation is 0, stated; and the not-recorded list names the tokens and the
/// times.
#[test]
fn an_unstated_figure_prints_unknown_never_zero() {
    let lines = vec![
        serde_json::json!({"step": 0, "event": "opened", "case": "takt-bare", "protocol": "software.change/1", "inputs": []}),
        serde_json::json!({"step": 1, "event": "step", "by": "validator", "name": "takt.ci-read", "action": "ci.read", "summary": "checks on pull request #4 at abc: pass"}),
        serde_json::json!({"step": 2, "event": "step", "by": "harness", "name": "claude-code", "action": "repository.inspect", "summary": "read"}),
        serde_json::json!({"step": 2, "event": "stop", "stop": "NoProgress", "steps": 2, "tokens": 0}),
    ];
    let trace = Trace::from_lines(
        &lines,
        None,
        "takt-bare",
        None,
        vec!["events.jsonl".to_owned()],
    );
    assert_eq!(trace.header.wall_s, None);
    assert_eq!(
        trace.header.tokens.sum, None,
        "the one call states no tokens, so the sum is unknown"
    );
    let a = report::attribution(&trace);
    assert_eq!(a.of(Actor::Forge), None, "{a:?}");
    assert_eq!(a.of(Actor::Harness), None, "{a:?}");
    assert_eq!(a.of(Actor::Takt), None, "{a:?}");
    assert_eq!(
        a.of(Actor::Validator),
        Some(0.0),
        "no validator step: a sum over nothing is 0, stated: {a:?}"
    );
    assert_eq!(
        (a.ci_reads, a.ci_read_s.known, a.ci_read_s.of),
        (1, 0, 1),
        "{a:?}"
    );
    assert_eq!(a.per_ci_read(), None, "{a:?}");
    assert_eq!(
        (a.tokens.known, a.tokens.of, a.tokens.value()),
        (0, 1, None),
        "{a:?}"
    );
    let text = report_text(&trace);
    assert!(
        text.contains("\ntokens unknown (0 over 0 of 1 call): components not recorded; cost -\n"),
        "{text}"
    );
    assert!(!text.contains("0.0 s over 1 read"), "{text}");
    assert!(
        text.contains("\nseconds per ci read: -; unknown (0.0 s over 0 of 1 read); waits on ci.read between them: 0, 0.0 s\n"),
        "{text}"
    );
    assert!(
        text.contains("| forge | unknown (0 over 0 of 1 step) | - |\n"),
        "{text}"
    );
    assert!(
        text.contains("| harness | unknown (0 over 0 of 1 call) | - |\n"),
        "{text}"
    );
    assert!(text.contains("| validator | 0 | - |\n"), "{text}");
    assert!(text.contains("| takt | unknown | - |\n"), "{text}");
    assert!(
        text.contains("| 1 | forge | ci.read | - | - | - | - | - |\n"),
        "{text}"
    );
    assert!(
        text.contains("| takt-bare | software.change/1 | - | NoProgress | - | 2 | 1 | - | unknown (0 over 0 of 1 call) | - | 0 | unknown (0 over 0 of 1 step) | 0 | 0 | unknown (0 over 0 of 1 call) | - | unknown | 1 | - | not recorded |\n"),
        "the cross-run row: {text}"
    );
    let missing = report::not_recorded(&trace);
    assert!(missing.contains(&"tokens"), "{missing:?}");
    assert!(missing.contains(&"times"), "{missing:?}");
}

/// A sum some calls cannot join says how many did: of two calls, one states tokens and a
/// duration and has instants, the other none of them, so the tokens and the harness's
/// seconds print as `unknown (<stated> over 1 of 2 calls)`, the validator's seconds stand,
/// and takt's remainder is unknown.
#[test]
fn a_sum_some_calls_cannot_join_says_how_many_did() {
    let lines = vec![
        serde_json::json!({"at": "2026-10-19T10:00:00.000000Z", "step": 0, "event": "opened", "case": "takt-mixed", "protocol": "software.change/1", "inputs": []}),
        serde_json::json!({"at": "2026-10-19T10:00:10.000000Z", "step": 1, "event": "step", "by": "validator", "name": "takt.tests-run", "action": "tests.run", "summary": "the test command exited 0: pass"}),
        serde_json::json!({"step": 2, "event": "step", "by": "harness", "name": "claude-code", "action": "repository.inspect", "summary": "read"}),
        serde_json::json!({"at": "2026-10-19T10:00:30.000000Z", "step": 3, "event": "step", "by": "harness", "name": "claude-code", "action": "implementation.edit", "summary": "edited", "tokens": 1000, "duration_ms": 15000}),
        serde_json::json!({"at": "2026-10-19T10:00:30.000000Z", "step": 3, "event": "stop", "stop": "Budget (steps)", "steps": 3, "tokens": 1000}),
    ];
    let trace = Trace::from_lines(
        &lines,
        None,
        "takt-mixed",
        None,
        vec!["events.jsonl".to_owned()],
    );
    assert_eq!(trace.header.wall_s, Some(30.0));
    assert_eq!(trace.header.tokens.sum, None);
    let a = report::attribution(&trace);
    assert_eq!(a.basis, Basis::Spans);
    assert_eq!(a.of(Actor::Validator), Some(10.0), "{a:?}");
    assert_eq!(a.of(Actor::Harness), None, "{a:?}");
    assert_eq!(a.of(Actor::Takt), None, "{a:?}");
    let harness = a.seconds[&Actor::Harness];
    assert_eq!(
        (harness.stated, harness.known, harness.of),
        (20.0, 1, 2),
        "{a:?}"
    );
    assert_eq!((a.tokens.stated, a.tokens.known, a.tokens.of), (1000, 1, 2));
    let text = report_text(&trace);
    assert!(text.contains("| validator | 10 | 33% |\n"), "{text}");
    assert!(
        text.contains("| harness | unknown (20 over 1 of 2 calls) | - |\n"),
        "{text}"
    );
    assert!(text.contains("| takt | unknown | - |\n"), "{text}");
    assert!(
        text.contains(
            "\ntokens unknown (1000 over 1 of 2 calls): components not recorded; cost -\n"
        ),
        "{text}"
    );
    assert!(
        text.contains("| 2 | harness | repository.inspect | - | - | - | - | - |\n"),
        "{text}"
    );
    assert!(
        text.contains("| 3 | harness | implementation.edit | 20.0 | 15.0 | - | 1000 | - |\n"),
        "{text}"
    );
    assert!(
        text.contains("| takt-mixed | software.change/1 | - | Budget | 30 | 3 | 2 | - | unknown (1000 over 1 of 2 calls) | - | 10 | 0 | 0 | 0 | unknown (20 over 1 of 2 calls) | - | unknown | 0 | - | not recorded |\n"),
        "the cross-run row: {text}"
    );
    let missing = report::not_recorded(&trace);
    assert!(
        missing.contains(&"tokens") && missing.contains(&"times"),
        "{missing:?}"
    );
    assert!(missing.contains(&"call durations"), "{missing:?}");
}

/// A symbolic link where `events.jsonl` should be is refused, never followed: by the trace
/// and by the scorecard, which both read through [`Events::read`], and by [`Events::emit`],
/// which leaves the link's target as it was. A link that leads nowhere is refused too, not
/// read as no record and not written through.
#[test]
fn a_symlinked_record_is_refused_and_never_followed() {
    let dir = scratch("symlink");
    let real = dir.join("real");
    fs::create_dir_all(&real).expect("a directory");
    fs::copy(
        fixture("trial-1").join("events.jsonl"),
        real.join("events.jsonl"),
    )
    .expect("a copy");
    let before = fs::read(real.join("events.jsonl")).expect("reads");
    let repo = dir.join("repo");
    let case = repo.join(CASES_DIR).join("takt-linked");
    fs::create_dir_all(&case).expect("a case directory");
    std::os::unix::fs::symlink(real.join("events.jsonl"), case.join("events.jsonl"))
        .expect("a link");

    let error = Trace::from_case(&case, None, None).expect_err("refused");
    assert!(format!("{error:#}").contains("symbolic link"), "{error:#}");
    assert!(format!("{error:#}").contains("events.jsonl"), "{error:#}");
    let error = Scorecard::read(&repo).expect_err("refused");
    assert!(format!("{error:#}").contains("symbolic link"), "{error:#}");
    let planted = Event::Error {
        error: "planted".to_owned(),
    };
    let error = Events::new(&case).emit(0, &planted).expect_err("refused");
    assert!(format!("{error:#}").contains("symbolic link"), "{error:#}");
    assert_eq!(
        fs::read(real.join("events.jsonl")).expect("reads"),
        before,
        "the link's target is as it was"
    );

    let dangling = dir.join("dangling");
    fs::create_dir_all(&dangling).expect("a directory");
    std::os::unix::fs::symlink(dir.join("nowhere.jsonl"), dangling.join("events.jsonl"))
        .expect("a link");
    let error = Events::new(&dangling)
        .read()
        .expect_err("refused, not read as no record");
    assert!(format!("{error:#}").contains("symbolic link"), "{error:#}");
    let error = Events::new(&dangling)
        .emit(0, &planted)
        .expect_err("refused");
    assert!(format!("{error:#}").contains("symbolic link"), "{error:#}");
    assert!(
        !dir.join("nowhere.jsonl").exists(),
        "nothing was created through the link"
    );
}

/// story:the-report-attributes-cost-and-failures-to-layers (ADR 0101): the report prints, per
/// run, cost and failures by layer, read from the records each layer wrote. Trial 1's record
/// has no `layers`: its layers column says `not recorded`; its model layer's tokens and cost
/// come from the harness steps' lines, named by file and line; takt's tokens are not reported,
/// never 0; the harness session layer's and the runtime's failures are not reported, since no
/// record of theirs is read.
#[test]
fn trial_1_report_attributes_cost_by_layer_from_the_steps_lines() {
    let printed = report_text(&trace("trial-1", true));
    assert!(printed.contains("| layers |"), "{printed}");
    assert!(
        printed.contains("| 0 | - | not recorded |"),
        "the layers column of a record without layers: {printed}"
    );
    assert!(
        printed.contains("| layer | revision | tokens | cost $ | wall s | failures | records |"),
        "{printed}"
    );
    assert!(
        printed.contains(
            "| model | not recorded | 592698 | 0.849 | 128 | not reported | events.jsonl:6,12,18"
        ),
        "{printed}"
    );
    assert!(
        printed.contains("| takt | not recorded | not reported | not reported | 112 | 0 |"),
        "{printed}"
    );
    assert!(
        printed.contains("| harness | not recorded | not reported | not reported | 130 | not reported | events.jsonl:6,12,18"),
        "{printed}"
    );
    assert!(
        printed.contains("| runtime | not recorded | not reported | not reported | not reported | not reported | - |"),
        "{printed}"
    );
    assert!(printed.contains("layers reported today:"), "{printed}");
}

/// A record with `layers` prints them in the layers column, the model layer's from the steps'
/// reported model, and each failure under the layer whose record states it, with the record's
/// file and line: takt's refusal and `NoProgress` stop; the total across runs is by layer too.
#[test]
fn a_layered_record_reports_its_layers_and_the_failures_by_line() {
    let layered = trace("layered-1", false);
    assert_eq!(
        layered.header.layers,
        Some(
            [
                ("takt", "0123456789abcdef0123456789abcdef01234567"),
                ("harness", "2.1.304 (Claude Code)"),
                ("model", "claude-opus-5-5"),
            ]
            .into_iter()
            .map(|(name, revision)| (name.to_owned(), revision.to_owned()))
            .collect()
        ),
        "{:#?}",
        layered.header
    );
    let lines: Vec<Option<usize>> = layered.ops.iter().map(|op| op.line).collect();
    assert_eq!(lines, vec![Some(3), Some(5), Some(6)], "{:#?}", layered.ops);
    let mut out = Vec::new();
    report::print(&[trace("trial-1", true), layered], &mut out).expect("prints");
    let printed = String::from_utf8(out).expect("utf-8");
    assert!(
        printed.contains(
            "| takt 0123456789abcdef0123456789abcdef01234567; harness 2.1.304 (Claude Code); model claude-opus-5-5 |"
        ),
        "{printed}"
    );
    assert!(
        printed.contains(
            "| model | claude-opus-5-5 | 1000 | 0.500 | 18 | not reported | events.jsonl:3 |"
        ),
        "{printed}"
    );
    assert!(
        printed.contains("| harness | 2.1.304 (Claude Code) | not reported | not reported | 20 | not reported | events.jsonl:3 |"),
        "{printed}"
    );
    assert!(
        printed.contains("| takt | 0123456789abcdef0123456789abcdef01234567 | not reported | not reported | 1 | 2 |"),
        "{printed}"
    );
    assert!(
        printed.contains("- takt: step 1: refusal by takt (events.jsonl:5)"),
        "{printed}"
    );
    assert!(
        printed.contains("- takt: step 1: stop NoProgress (events.jsonl:6)"),
        "{printed}"
    );
    assert!(printed.contains("## total by layer"), "{printed}");
    assert!(
        printed.contains("| model | 593698 | 1.349 | 146 | not reported | 2 |"),
        "{printed}"
    );
    assert!(
        printed.contains("| takt | not reported | not reported | 113 | 2 | 2 |"),
        "{printed}"
    );
}
