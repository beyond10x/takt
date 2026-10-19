//! Adversary pass 1 on story:scorecard-reader. Each case drives the scorecard, or the run loop
//! that writes the record it reads, from a record shaped the way the loop writes one; where a
//! line comes from an older takt, it is rewritten from one the current `takt::events` wrote,
//! so the bytes match what that takt wrote.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use takt::case::CASES_DIR;
use takt::events::{EVENTS_FILE, Event, Events};
use takt::harness::ClaudeCode;
use takt::scorecard::{Named, Scorecard};
use takt::validator::Validator;
use takt::{Run, RunOptions};

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
        .join(format!("scorecard-adv1-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(&dir).expect("a scratch tree");
    Scratch(dir)
}

fn opened(case: &str) -> Event {
    Event::Opened {
        case: case.to_owned(),
        protocol: "story.delivery/1".to_owned(),
        story: Some("story:scratch-one".to_owned()),
        inputs: Vec::new(),
        base: None,
        binary: None,
    }
}

fn judged(tokens: u64) -> Event {
    Event::Step {
        by: "harness".to_owned(),
        name: "claude-code".to_owned(),
        action: "implementation.edit".to_owned(),
        summary: "edited".to_owned(),
        tokens: Some(tokens),
        usage: None,
        revision: None,
        model: None,
        cost_usd: None,
        duration_ms: None,
        api_ms: None,
        turns: None,
        started_at: None,
    }
}

fn stop(stop: &str, steps: u64, tokens: u64) -> Event {
    Event::Stop {
        stop: stop.to_owned(),
        steps,
        tokens,
    }
}

/// Writes `runs` to the case `case`'s record under `repo` through `takt::events`, and for a
/// scenario case a held-out result that passed beside it.
fn record(repo: &Path, case: &str, runs: &[Vec<Event>]) -> PathBuf {
    let dir = repo.join(CASES_DIR).join(case);
    fs::create_dir_all(&dir).expect("a case directory");
    if takt::scorecard::scenario_of(case).is_some() {
        fs::write(dir.join(takt::scorecard::HELD_OUT_FILE), "PASS\n").expect("the held-out result");
    }
    let events = Events::new(&dir);
    for run in runs {
        let mut step = 0;
        for event in run {
            if matches!(event, Event::Step { .. }) {
                step += 1;
            }
            events.emit(step, event).expect("the event is written");
        }
    }
    dir
}

/// `card` compared with the scorecard saved at `saved`, under a rule of one trial: these cases
/// are about the ranks, the definitions and the wall time on fewer than five cases, and five
/// trials are story:scorecard-against-needs-the-trials'.
fn against(card: &Scorecard, saved: &Path) -> (usize, String) {
    let mut card = card.clone();
    card.rule.trials = 1;
    let mut out = Vec::new();
    let worse = card
        .against(
            &Scorecard::load(saved).expect("the saved scorecard reads"),
            &mut out,
        )
        .expect("the comparison prints");
    (worse, String::from_utf8(out).expect("utf-8"))
}

/// Scenario 1's documented path is one invocation plus one grant: the first run stops
/// `ApprovalRequired (repository.merge)`, `takt approve` grants it, and `takt run --case`
/// resumes the case to `accepted`. Every such case holds two runs. A scorecard compared with
/// itself, saved, cannot be worse than itself; here the first run of the very same case is
/// marked worse than the second.
#[test]
fn a_resumed_scenario_one_case_is_not_worse_than_itself() {
    let repo = scratch("self-resume");
    record(
        &repo,
        "scenario-1-trial-1",
        &[
            vec![
                opened("scenario-1-trial-1"),
                judged(500),
                stop("ApprovalRequired (repository.merge)", 1, 500),
            ],
            vec![
                opened("scenario-1-trial-1"),
                judged(40),
                stop("Completed (accepted)", 1, 40),
            ],
        ],
    );
    let card = Scorecard::read(&repo).expect("the scorecard reads");
    let saves = scratch("self-resume-saves");
    let saved = card
        .clone()
        .save(
            &saves,
            "1111111111111111111111111111111111111111",
            Named::Head,
        )
        .expect("saved");
    let (worse, text) = against(&card, &saved);
    assert_eq!(
        worse, 0,
        "the same record against itself marks a run worse:\n{text}"
    );
}

/// The same property over trials: a baseline whose scenario 1 trials ended `accepted` and
/// `NoProgress` (token totals and outcomes vary from trial to trial with no change), compared
/// with an identical candidate, is not worse than itself.
#[test]
fn a_scorecard_with_varying_trials_is_not_worse_than_itself() {
    let repo = scratch("self-trials");
    record(
        &repo,
        "scenario-1-trial-1",
        &[vec![
            opened("scenario-1-trial-1"),
            judged(500),
            stop("Completed (accepted)", 1, 500),
        ]],
    );
    record(
        &repo,
        "scenario-1-trial-2",
        &[vec![
            opened("scenario-1-trial-2"),
            judged(300),
            stop("NoProgress", 1, 300),
        ]],
    );
    let card = Scorecard::read(&repo).expect("the scorecard reads");
    let saves = scratch("self-trials-saves");
    let saved = card
        .clone()
        .save(
            &saves,
            "2222222222222222222222222222222222222222",
            Named::Head,
        )
        .expect("saved");
    let (worse, text) = against(&card, &saved);
    assert_eq!(
        worse, 0,
        "the same record against itself marks a run worse:\n{text}"
    );
}

/// A baseline scenario the candidate has no run of (its case directory has no record, or there
/// is no case at all: a candidate whose `takt scenario run` fails before the case opens) is not
/// "not worse": the comparison passes it silently and returns 0, the exit code of a clean
/// comparison.
#[test]
fn a_baseline_scenario_the_candidate_never_ran_is_not_passed_silently() {
    let baseline = scratch("missing-baseline");
    record(
        &baseline,
        "scenario-1-trial-1",
        &[vec![
            opened("scenario-1-trial-1"),
            judged(500),
            stop("Completed (accepted)", 1, 500),
        ]],
    );
    let saves = scratch("missing-saves");
    let saved = Scorecard::read(&baseline)
        .expect("the scorecard reads")
        .save(
            &saves,
            "3333333333333333333333333333333333333333",
            Named::Head,
        )
        .expect("saved");
    let candidate = scratch("missing-candidate");
    fs::create_dir_all(candidate.join(CASES_DIR).join("scenario-1-trial-2"))
        .expect("a case directory without a record");
    let mut card = Scorecard::read(&candidate).expect("the scorecard reads");
    // Under a rule of one trial, as `against` compares; no case is fewer than one, so the
    // comparison gives no verdict and refuses, as a promotion does (review N7).
    card.rule.trials = 1;
    let mut out = Vec::new();
    let compared = card.against(
        &Scorecard::load(&saved).expect("the saved scorecard reads"),
        &mut out,
    );
    let text = String::from_utf8(out).expect("utf-8");
    let error = format!(
        "{:#}",
        compared.expect_err(&format!(
            "scenario 1 has a saved run and no candidate run, and the comparison is clean:\n{text}"
        ))
    );
    assert!(error.contains("scenario 1 (0 of 1)"), "{error}");
    assert!(
        text.contains("scenario 1  too few cases (0 of 1)"),
        "{text}"
    );
}

/// Sets the `at` of line `n` (from 0) of the record in `dir` to `instants[n]` where that is
/// given; every other line, the torn one among them, is kept byte for byte. A line's `at` is
/// its first key, as `takt::events` writes it.
fn pin(dir: &Path, instants: &[Option<&str>]) {
    let path = dir.join(EVENTS_FILE);
    let text = fs::read_to_string(&path).expect("the record");
    let mut pinned = String::new();
    for (n, line) in text.split_inclusive('\n').enumerate() {
        let rest = line
            .strip_prefix(r#"{"at":""#)
            .and_then(|rest| rest.split_once('"'));
        match (instants.get(n).copied().flatten(), rest) {
            (Some(at), Some((_, rest))) => {
                pinned.push_str(r#"{"at":""#);
                pinned.push_str(at);
                pinned.push('"');
                pinned.push_str(rest);
            }
            _ => pinned.push_str(line),
        }
    }
    fs::write(&path, pinned).expect("the record at pinned instants");
}

/// A run killed mid-write leaves a torn line; the next run's first write ends it and writes
/// the `skipped` line before its `opened`, at the next run's instant. The killed run's wall
/// time is its own lines' span, not the time until somebody resumed the case. Every instant is
/// set, none read from the clock: the killed run's two lines 100 ms apart, its torn line
/// 100 ms later, the resumed run's lines 600 ms after that.
#[test]
fn a_crashed_runs_wall_time_ends_at_its_own_last_line() {
    let repo = scratch("crash");
    let dir = record(
        &repo,
        "scenario-1-trial-1",
        &[vec![opened("scenario-1-trial-1"), judged(100)]],
    );
    pin(
        &dir,
        &[
            Some("2026-10-18T09:59:59.800000Z"),
            Some("2026-10-18T09:59:59.900000Z"),
        ],
    );
    let mut text = fs::read_to_string(dir.join(EVENTS_FILE)).expect("the record");
    text.push_str(r#"{"at":"2026-10-18T10:00:00.000000Z","step":2,"event":"st"#);
    fs::write(dir.join(EVENTS_FILE), text).expect("the torn record");
    let events = Events::new(&dir);
    events
        .emit(0, &opened("scenario-1-trial-1"))
        .expect("the resumed run opens");
    events
        .emit(0, &stop("NoProgress", 0, 0))
        .expect("the resumed run stops");
    let resumed = Some("2026-10-18T10:00:00.600000Z");
    pin(&dir, &[None, None, None, resumed, resumed, resumed]);
    let lines = fs::read_to_string(dir.join(EVENTS_FILE)).expect("the record");
    assert!(
        lines.lines().nth(3).is_some_and(|line| {
            line.starts_with(r#"{"at":"2026-10-18T10:00:00.600000Z""#)
                && line.contains(r#""event":"skipped""#)
        }),
        "the premise: the resumed run's first write ended the torn line and named it skipped\n{lines}"
    );
    let card = Scorecard::read(&repo).expect("the scorecard reads");
    assert_eq!(card.rows.len(), 2, "{:?}", card.rows);
    assert_eq!(card.rows[0].outcome, "unfinished", "{:?}", card.rows[0]);
    assert_eq!(
        card.rows[0].wall_ms, 100,
        "the killed run's wall time is its own two lines' span, not the span to the torn line \
         (200 ms) or to the resume (800 ms)"
    );
}

/// A wave 09 record, as the takt of the base wrote it: whole-second instants, and the
/// `store_evidence` line's own `at` beside the record's instant under the same key `at`. In
/// scenario 1 the pass record was observed in the first run, before the grant; the accepted
/// run writes it to the store. The accepted run's wall time is its own lines' span.
#[test]
fn a_wave_09_store_evidence_line_does_not_stretch_the_runs_wall_time() {
    let repo = scratch("wave-09");
    let dir = record(
        &repo,
        "scenario-1-trial-1",
        &[vec![
            opened("scenario-1-trial-1"),
            Event::StoreEvidence {
                story: "story:scratch-one".to_owned(),
                kind: "test_result".to_owned(),
                reference: ".engineering/cases/scenario-1-trial-1/evidence/0001.json".to_owned(),
                observed_at: "2026-10-18T09:00:00Z".to_owned(),
                commit: None,
            },
            stop("Completed (accepted)", 0, 0),
        ]],
    );
    let written = fs::read_to_string(dir.join(EVENTS_FILE)).expect("the record");
    let mut wave_09 = String::new();
    for (line, at) in written.lines().zip([
        "2026-10-18T10:05:00Z",
        "2026-10-18T10:05:01Z",
        "2026-10-18T10:05:02Z",
    ]) {
        let rest = line
            .strip_prefix("{\"at\":\"")
            .and_then(|rest| rest.get(28..))
            .unwrap_or_else(|| panic!("a line led by its instant: {line}"));
        wave_09
            .push_str(&format!("{{\"at\":\"{at}\"{rest}").replace("\"observed_at\":", "\"at\":"));
        wave_09.push('\n');
    }
    assert!(
        wave_09.contains(r#""at":"2026-10-18T09:00:00Z""#),
        "{wave_09}"
    );
    fs::write(dir.join(EVENTS_FILE), &wave_09).expect("the wave 09 record");
    let card = Scorecard::read(&repo).expect("the wave 09 record reads");
    assert_eq!(card.rows.len(), 1, "{:?}", card.rows);
    assert_eq!(
        card.rows[0].wall_ms, 2000,
        "the run spans 10:05:00 to 10:05:02:\n{wave_09}"
    );
}

/// The story's third check is about not letting a redefinition make a candidate look better.
/// A saved scorecard whose `outcome` definition has another version: a `NoProgress` run
/// against a saved `accepted` one is still a run whose outcome is worse.
#[test]
fn a_changed_outcome_definition_does_not_switch_the_outcome_gate_off() {
    let baseline = scratch("outcome-version-baseline");
    record(
        &baseline,
        "scenario-1-trial-1",
        &[vec![
            opened("scenario-1-trial-1"),
            judged(500),
            stop("Completed (accepted)", 1, 500),
        ]],
    );
    let saves = scratch("outcome-version-saves");
    let saved = Scorecard::read(&baseline)
        .expect("the scorecard reads")
        .save(
            &saves,
            "4444444444444444444444444444444444444444",
            Named::Head,
        )
        .expect("saved");
    let mut file: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&saved).expect("the file")).expect("json");
    let current = file["metrics"]["outcome"].as_u64().expect("a version");
    file["metrics"]["outcome"] = (current + 1).into();
    fs::write(&saved, serde_json::to_string(&file).expect("json")).expect("the file");

    let candidate = scratch("outcome-version-candidate");
    record(
        &candidate,
        "scenario-1-trial-2",
        &[vec![
            opened("scenario-1-trial-2"),
            judged(600),
            stop("NoProgress", 1, 600),
        ]],
    );
    let (worse, text) = against(
        &Scorecard::read(&candidate).expect("the scorecard reads"),
        &saved,
    );
    assert!(
        worse > 0,
        "a definition change turned the outcome comparison off:\n{text}"
    );
}

/// Tokens the harness reported past what a u64 sums: the reader refuses or saturates, it
/// does not panic (debug) or wrap to a small number (release).
#[test]
fn huge_reported_tokens_do_not_panic_or_wrap() {
    let repo = scratch("huge");
    record(
        &repo,
        "scenario-1-trial-1",
        &[vec![
            opened("scenario-1-trial-1"),
            judged(u64::MAX),
            judged(u64::MAX),
            stop("Completed (accepted)", 2, u64::MAX),
        ]],
    );
    let read = std::panic::catch_unwind(|| Scorecard::read(&repo));
    let read = read.expect("the reader does not panic on the record");
    if let Ok(card) = read {
        assert_eq!(card.rows[0].tokens, u64::MAX, "{:?}", card.rows[0]);
    }
}

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

/// The delivery protocol, relative to this crate.
const PROTOCOL: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../protocols/story-delivery/1.yaml"
);

/// The brief: a step event carries the four token components the harness reports beside
/// their sum. The harness parses all four from `claude`'s `usage` today
/// (`crates/takt/src/harness.rs`); a judgement step through the loop records them.
#[test]
fn the_loop_records_the_four_components_the_harness_parsed() {
    use std::os::unix::fs::PermissionsExt as _;
    let tree = scratch("usage-tree");
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
    let scripts = scratch("usage-scripts");
    let program = scripts.join("claude.sh");
    fs::write(
        &program,
        "#!/bin/sh\nprintf '%s' '{\"result\": \"{\\\"action\\\": \\\"implementation.edit\\\", \\\"summary\\\": \\\"read\\\"}\", \"usage\": {\"input_tokens\": 11, \"output_tokens\": 22, \"cache_read_input_tokens\": 33, \"cache_creation_input_tokens\": 44}}'\n",
    )
    .expect("a script");
    fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).expect("exec bit");
    let harness = ClaudeCode {
        program: program.display().to_string(),
        ..ClaudeCode::default()
    };
    let validators: Vec<Box<dyn Validator>> = Vec::new();
    let options = RunOptions {
        protocol: PROTOCOL.to_owned(),
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
    let text = String::from_utf8_lossy(&out).into_owned();
    stopped.unwrap_or_else(|error| panic!("the step runs: {error:#}\n{text}"));
    let lines = Events::new(&run.case.dir).read().expect("the record reads");
    let step = lines
        .iter()
        .find(|line| line["event"] == "step" && line["by"] == "harness")
        .unwrap_or_else(|| panic!("a harness step in {lines:#?}"));
    assert_eq!(step["tokens"], 110, "{step}");
    assert_eq!(
        step["usage"],
        serde_json::json!({"input": 11, "output": 22, "cache_read": 33, "cache_creation": 44}),
        "the harness parsed the four components, and the step line drops them: {step}"
    );
}
