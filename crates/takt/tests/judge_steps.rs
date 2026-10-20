//! story:the-judge-compares-the-steps-a-candidate-targets: a candidate is judged on the steps it
//! targets (the actions named), per scenario and action: better when every candidate case's
//! tokens at the action are below the saved side's cheapest, or when a one-sided exact
//! Mann-Whitney U test at five cases a side rejects equality; the whole run is still judged not
//! worse under the run-level rule, and a candidate better on its steps but worse on the run is
//! refused, naming both.
//!
//! The re-judged cycle: candidate 1 of the trial cycle on release 0.3.0, its second re-trial
//! (`0c9311cb`, cases `scenario-1-trial-96` to `-100`), against the trigger's five runs of
//! `bee77822` (`scenario-1-trial-86` to `-90`). The fixture `judge-steps-0.3.0-cand-1` holds
//! those runs' recorded `events.jsonl`, cut to the `opened`, `step` and `stop` lines and the
//! fields the scorecard reads, and each case's `held-out.txt` as the runner wrote it; no new run.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use takt::case::CASES_DIR;
use takt::scorecard::{
    FORMAT, METRICS, RANK_TEST_CASES, Row, Rule, Scorecard, StepVerdict, rank_test,
};

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
        .join(format!("judge-steps-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(&dir).expect("a scratch tree");
    Scratch(dir)
}

const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/judge-steps-0.3.0-cand-1"
);

/// A repository whose cases directory holds the fixture's `side` (`baseline`, `candidate`).
fn side(name: &str, side: &str) -> Scratch {
    let repo = scratch(&format!("{name}-{side}"));
    let from = Path::new(FIXTURE).join(side);
    for case in fs::read_dir(&from).expect("the fixture side") {
        let case = case.expect("a case").path();
        let to = repo
            .join(CASES_DIR)
            .join(case.file_name().expect("a case name"));
        fs::create_dir_all(&to).expect("the case directory");
        for file in ["events.jsonl", "held-out.txt"] {
            fs::copy(case.join(file), to.join(file)).expect("a fixture file");
        }
    }
    repo
}

fn actions(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}

/// Acceptance 3: re-judging candidate 1 of the 0.3.0 cycle from its recorded rollouts reports
/// it better on `change.handover`, every candidate case below the trigger's cheapest; and the
/// run-level result, which the whole-run rule alone judged, is not worse: outcomes held, its
/// token median 67628 under the trigger's maximum 84088, though 598 above the trigger's median.
#[test]
fn candidate_1_of_the_0_3_0_cycle_is_better_on_change_handover_and_not_worse_on_the_run() {
    let (baseline, candidate) = (side("cycle", "baseline"), side("cycle", "candidate"));
    let saved = Scorecard::read(&baseline).expect("the trigger's runs read");
    let card = Scorecard::read(&candidate).expect("the candidate's runs read");
    let mut out = Vec::new();
    let judged = card
        .judge_steps(&saved, &actions(&["change.handover"]), &mut out)
        .expect("a verdict");
    let text = String::from_utf8(out).expect("utf-8");
    assert_eq!(judged.verdict, StepVerdict::Better, "{text}");
    assert!(judged.worse.is_empty(), "{text}");
    assert_eq!(judged.steps.len(), 1, "{text}");
    let step = &judged.steps[0];
    assert_eq!(
        (step.scenario.as_str(), step.action.as_str()),
        ("1", "change.handover")
    );
    let mut tokens: Vec<u64> = step.candidate.iter().map(|(_, tokens)| *tokens).collect();
    tokens.sort_unstable();
    assert_eq!(tokens, [8405, 8481, 8528, 8562, 8588], "{text}");
    assert_eq!(step.saved_cheapest, Some(15875), "{text}");
    assert!(step.every_below, "{text}");
    let rank = step.rank.expect("a rank test at five cases a side");
    assert_eq!((rank.u, rank.at_most, rank.total), (0, 1, 252), "{text}");
    assert!(rank.rejects && step.better, "{text}");
    assert!(
        text.contains("scenario 1  median tokens +598"),
        "the run-level result is stated: {text}"
    );
    assert!(
        text.contains("scenario 1  outcomes by rank held  tokens held  not worse"),
        "{text}"
    );
    assert!(
        text.contains("better on its steps (change.handover), not worse on the run"),
        "{text}"
    );
}

/// Acceptance 1: a step the candidate does not change is not judged better on the same records:
/// `tests.write` is cheaper in two cases and dearer in three.
#[test]
fn the_same_cycle_is_not_better_on_a_step_it_does_not_target() {
    let (baseline, candidate) = (
        side("untargeted", "baseline"),
        side("untargeted", "candidate"),
    );
    let saved = Scorecard::read(&baseline).expect("the trigger's runs read");
    let card = Scorecard::read(&candidate).expect("the candidate's runs read");
    let mut out = Vec::new();
    let judged = card
        .judge_steps(&saved, &actions(&["tests.write"]), &mut out)
        .expect("a verdict");
    let text = String::from_utf8(out).expect("utf-8");
    assert_eq!(judged.verdict, StepVerdict::NotBetter, "{text}");
    let step = &judged.steps[0];
    assert!(!step.every_below && !step.better, "{text}");
    assert!(!step.rank.expect("a rank test").rejects, "{text}");
    assert!(
        text.contains("verdict  not better on its steps (tests.write; not better: tests.write), not worse on the run"),
        "{text}"
    );
}

/// A run of scenario `s`, case `n`, with `tokens` in all and `steps` by action.
fn row(n: usize, outcome: &str, tokens: u64, steps: &[(&str, u64)]) -> Row {
    Row {
        case: format!("scenario-s-trial-{n}"),
        scenario: Some("s".to_owned()),
        outcome: outcome.to_owned(),
        stop: outcome.to_owned(),
        held_out: None,
        steps: steps.len() as u64,
        tokens,
        refusals: 0,
        no_progress: 0,
        errors: 0,
        wall_ms: 0,
        saturated: false,
        binary: None,
        opened_binary: None,
        completed: None,
        step_tokens: steps
            .iter()
            .map(|(action, tokens)| ((*action).to_owned(), *tokens))
            .collect(),
    }
}

fn card(rows: Vec<Row>) -> Scorecard {
    Scorecard {
        format: FORMAT.to_owned(),
        revision: None,
        named_by: None,
        metrics: METRICS
            .iter()
            .map(|(metric, version)| ((*metric).to_owned(), *version))
            .collect(),
        rule: Rule::default(),
        rows,
        no_record: Vec::new(),
        unstated: BTreeMap::new(),
    }
}

/// Five accepted cases, 100 tokens each, 50 of them at `a.step`.
fn saved() -> Scorecard {
    card(
        (0..5)
            .map(|n| row(n, "accepted", 100, &[("a.step", 50 + n as u64)]))
            .collect(),
    )
}

/// Acceptance 2: better on its step and worse on the run (its token median over the saved
/// maximum) is refused, naming both.
#[test]
fn better_on_its_steps_but_worse_on_the_run_is_refused_naming_both() {
    let candidate = card(
        (0..5)
            .map(|n| row(n, "accepted", 200, &[("a.step", 10)]))
            .collect(),
    );
    let mut out = Vec::new();
    let judged = candidate
        .judge_steps(&saved(), &actions(&["a.step"]), &mut out)
        .expect("a verdict");
    let text = String::from_utf8(out).expect("utf-8");
    assert_eq!(judged.verdict, StepVerdict::WorseOnTheRun, "{text}");
    assert_eq!(judged.worse, ["s"], "{text}");
    assert!(judged.steps[0].better, "{text}");
    assert!(
        text.contains("refused: better on its steps (a.step), but worse on the run (scenario s)"),
        "{text}"
    );
}

/// Acceptance 2: outcomes by rank are held too: two cases fewer accepted is worse on the run,
/// however cheap the step.
#[test]
fn better_on_its_steps_with_outcomes_short_is_refused() {
    let candidate = card(
        (0..5)
            .map(|n| {
                let outcome = if n < 2 { "NoProgress" } else { "accepted" };
                row(n, outcome, 90, &[("a.step", 10)])
            })
            .collect(),
    );
    let mut out = Vec::new();
    let judged = candidate
        .judge_steps(&saved(), &actions(&["a.step"]), &mut out)
        .expect("a verdict");
    let text = String::from_utf8(out).expect("utf-8");
    assert_eq!(judged.verdict, StepVerdict::WorseOnTheRun, "{text}");
    assert!(text.contains("outcomes by rank short"), "{text}");
}

/// Acceptance 1: one candidate case at or above the saved cheapest is not every case below, but
/// the rank test may still reject equality: at five a side, U of at most 4 (12 of 252
/// arrangements, p 0.048) rejects at 0.05 one-sided; 5 (19 of 252, p 0.075) does not. A tie
/// counts against the candidate.
#[test]
fn the_rank_test_rejects_equality_at_u_4_and_not_at_u_5_at_five_a_side() {
    let saved = [10, 11, 12, 13, 14];
    let four = rank_test(&[1, 2, 3, 4, 13], &saved).expect("a rank test");
    assert_eq!((four.u, four.at_most, four.total), (4, 12, 252));
    assert!(four.rejects);
    let five = rank_test(&[1, 2, 3, 4, 14], &saved).expect("a rank test");
    assert_eq!((five.u, five.at_most, five.total), (5, 19, 252));
    assert!(!five.rejects);
    let tied = rank_test(&[10, 10, 10, 10, 10], &[10, 10, 10, 10, 10]).expect("a rank test");
    assert_eq!(tied.u, 25);
    assert!(!tied.rejects);
    assert_eq!(RANK_TEST_CASES, 5);
    assert!(
        rank_test(&[1, 2, 3, 4], &saved).is_none(),
        "no rank test under five cases a side"
    );
}

/// Acceptance 1 through the judgement: a candidate one of whose cases is above the saved
/// cheapest is better by the rank test alone, and the output says which.
#[test]
fn better_by_the_rank_test_alone() {
    let candidate = card(
        [10, 11, 12, 13, 51]
            .iter()
            .enumerate()
            .map(|(n, tokens)| row(n, "accepted", 90, &[("a.step", *tokens)]))
            .collect(),
    );
    let mut out = Vec::new();
    let judged = candidate
        .judge_steps(&saved(), &actions(&["a.step"]), &mut out)
        .expect("a verdict");
    let text = String::from_utf8(out).expect("utf-8");
    assert_eq!(judged.verdict, StepVerdict::Better, "{text}");
    assert!(!judged.steps[0].every_below, "{text}");
    assert!(text.contains("U 2"), "{text}");
}

/// A candidate case that took no step of the action is no evidence the step got cheaper: the
/// action is not better, the case named.
#[test]
fn a_case_without_the_targeted_step_is_not_better_and_named() {
    let mut rows: Vec<Row> = (0..4)
        .map(|n| row(n, "accepted", 90, &[("a.step", 10)]))
        .collect();
    rows.push(row(4, "accepted", 90, &[("other.step", 10)]));
    let mut out = Vec::new();
    let judged = card(rows)
        .judge_steps(&saved(), &actions(&["a.step"]), &mut out)
        .expect("a verdict");
    let text = String::from_utf8(out).expect("utf-8");
    assert_eq!(judged.verdict, StepVerdict::NotBetter, "{text}");
    assert_eq!(judged.steps[0].missing, ["scenario-s-trial-4"], "{text}");
    assert!(text.contains("scenario-s-trial-4"), "{text}");
}

/// A targeted action no case on either side took is not better: the judgement covers nothing.
#[test]
fn an_action_no_case_took_is_not_better() {
    let candidate = card(
        (0..5)
            .map(|n| row(n, "accepted", 90, &[("a.step", 10)]))
            .collect(),
    );
    let mut out = Vec::new();
    let judged = candidate
        .judge_steps(&saved(), &actions(&["no.such"]), &mut out)
        .expect("a verdict");
    let text = String::from_utf8(out).expect("utf-8");
    assert_eq!(judged.verdict, StepVerdict::NotBetter, "{text}");
    assert!(text.contains("no.such"), "{text}");
}

/// Acceptance 4: a saved scorecard taken under no `step_tokens` definition has no per-step
/// figures to judge against: refused, naming the metric, not judged as if every step were 0.
#[test]
fn a_saved_scorecard_without_the_step_tokens_metric_is_refused() {
    let mut old = saved();
    old.metrics.remove("step_tokens");
    let candidate = card(
        (0..5)
            .map(|n| row(n, "accepted", 90, &[("a.step", 10)]))
            .collect(),
    );
    let mut out = Vec::new();
    let refused = candidate
        .judge_steps(&old, &actions(&["a.step"]), &mut out)
        .expect_err("refused")
        .to_string();
    assert!(refused.contains("step_tokens"), "{refused}");
}

/// Acceptance 4: the per-step figure is a metric with a definition version of its own, saved
/// with every scorecard, and a run's tokens by action are saved and read back.
#[test]
fn step_tokens_has_a_definition_version_and_survives_a_save() {
    assert!(METRICS.contains(&("step_tokens", 1)), "{METRICS:?}");
    let candidate = side("save", "candidate");
    let card = Scorecard::read(&candidate).expect("the candidate's runs read");
    assert_eq!(card.metrics.get("step_tokens"), Some(&1));
    let saves = scratch("save-dir");
    let path = card
        .clone()
        .save(
            &saves,
            "0c9311cb4b3ae6a432c2847982c252382e41dd3d",
            takt::scorecard::Named::Build,
        )
        .expect("saved");
    let loaded = Scorecard::load(&path).expect("loaded");
    assert_eq!(loaded.rows, card.rows);
    let handover: Vec<u64> = loaded
        .rows
        .iter()
        .map(|row| row.step_tokens["change.handover"])
        .collect();
    assert_eq!(handover, [8405, 8588, 8528, 8562, 8481]);
}
