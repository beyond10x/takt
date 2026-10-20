//! story:run-tokens-unknown-is-not-zero: a run whose harness step recorded no tokens (or 0, which
//! takt never writes) has unknown run tokens, never a sum that counts the step 0; a validator
//! step still counts 0. A comparison (`takt scorecard --against`, and so `takt promote`) with a
//! scenario case of unknown run tokens on either side is refused before anything is judged,
//! naming the case and the step; and the `tokens` definition moves, so a takt that read a
//! missing record as 0 does not read a scorecard that can hold an unknown.
//!
//! The 0.3.0 trial cycle (fixture `judge-steps-0.3.0-cand-1`, as `judge_steps.rs` describes it):
//! with one candidate step's tokens removed, the candidate's median fell from 67628 to 62554 and
//! its difference against the saved median went from +598 to -4476, a missing record read as a
//! saving.

use std::fs;
use std::path::{Path, PathBuf};

use takt::case::CASES_DIR;
use takt::scorecard::{METRICS, Scorecard};

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
        .join(format!("run-tokens-unknown-{}-{name}", std::process::id()));
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

/// Sets the tokens of the first harness step of `action` in `case`'s record under `repo`:
/// removed with its `usage` for none, else to the figure.
fn retokened(repo: &Path, case: &str, action: &str, tokens: Option<u64>) {
    let path = repo.join(CASES_DIR).join(case).join("events.jsonl");
    let text = fs::read_to_string(&path).expect("the record");
    let mut done = false;
    let lines: Vec<String> = text
        .lines()
        .map(|line| {
            let mut value: serde_json::Value = serde_json::from_str(line).expect("a line");
            if !done
                && value["event"] == "step"
                && value["by"] == "harness"
                && value["action"] == action
            {
                let object = value.as_object_mut().expect("an object");
                match tokens {
                    Some(tokens) => {
                        object.insert("tokens".to_owned(), tokens.into());
                    }
                    None => {
                        assert!(object.remove("tokens").is_some(), "the step had tokens");
                        object.remove("usage");
                    }
                }
                done = true;
            }
            value.to_string()
        })
        .collect();
    assert!(done, "{case} has a harness step of {action}");
    fs::write(&path, lines.join("\n") + "\n").expect("written");
}

/// One run of `case` under `repo`: `opened`, then each `(by, tokens)` as a step, then a stop.
fn record(repo: &Path, case: &str, steps: &[(&str, Option<u64>)]) {
    let dir = repo.join(CASES_DIR).join(case);
    fs::create_dir_all(&dir).expect("a case directory");
    let at = "2026-10-20T10:00:00.000000Z";
    let mut lines = vec![serde_json::json!({
        "at": at, "step": 0, "event": "opened", "case": case, "protocol": "story.delivery/1"
    })];
    for (n, (by, tokens)) in steps.iter().enumerate() {
        let mut line = serde_json::json!({
            "at": at, "step": n + 1, "event": "step", "by": by, "name": "x",
            "action": format!("{by}.act{}", n + 1), "summary": "s"
        });
        if let Some(tokens) = tokens {
            line["tokens"] = (*tokens).into();
        }
        lines.push(line);
    }
    lines.push(serde_json::json!({
        "at": at, "step": steps.len(), "event": "stop", "stop": "NoProgress", "steps": steps.len()
    }));
    let text: String = lines.iter().map(|line| format!("{line}\n")).collect();
    fs::write(dir.join("events.jsonl"), text).expect("the record");
}

/// Acceptance 1: a run with a harness step that recorded no tokens, or 0, has unknown run
/// tokens; a validator step, which reports none, still counts 0.
#[test]
fn a_harness_step_without_tokens_leaves_the_run_tokens_unknown_and_a_validator_counts_0() {
    let repo = scratch("read");
    record(
        &repo,
        "known",
        &[
            ("validator", None),
            ("harness", Some(100)),
            ("validator", None),
        ],
    );
    record(
        &repo,
        "missing",
        &[("harness", Some(100)), ("harness", None)],
    );
    record(
        &repo,
        "zero",
        &[("harness", Some(100)), ("harness", Some(0))],
    );
    record(&repo, "validators", &[("validator", None)]);
    let card = Scorecard::read(&repo).expect("the runs read");
    let tokens = |case: &str| {
        card.rows
            .iter()
            .find(|row| row.case == case)
            .unwrap_or_else(|| panic!("{case}: {card:?}"))
            .tokens
    };
    assert_eq!(tokens("known"), Some(100), "{card:?}");
    assert_eq!(tokens("missing"), None, "{card:?}");
    assert_eq!(
        tokens("zero"),
        None,
        "a harness step's 0 is no count: {card:?}"
    );
    assert_eq!(tokens("validators"), Some(0), "{card:?}");
    let zero = card
        .rows
        .iter()
        .find(|row| row.case == "zero")
        .expect("zero");
    assert_eq!(
        zero.step_tokens.get("harness.act2"),
        Some(&None),
        "the step's own figure is unknown too: {zero:?}"
    );
    let mut out = Vec::new();
    card.print(&mut out).expect("printed");
    let text = String::from_utf8(out).expect("utf-8");
    let line = text
        .lines()
        .find(|line| line.starts_with("missing "))
        .expect("the case's line");
    assert!(line.contains("unknown"), "{text}");
}

/// Acceptance 4: candidate 1 of the 0.3.0 cycle, one step's tokens removed, is refused, not
/// judged better: the case and the step named, nothing printed. Without the removal the same
/// records are judged as before (+598, not worse).
#[test]
fn the_0_3_0_cycle_with_one_step_untokened_is_refused_not_judged_better() {
    let (baseline, candidate) = (side("cycle", "baseline"), side("cycle", "candidate"));
    let saved = Scorecard::read(&baseline).expect("the trigger's runs read");
    let card = Scorecard::read(&candidate).expect("the candidate's runs read");
    let mut out = Vec::new();
    assert_eq!(card.against(&saved, &mut out).expect("a verdict"), 0);
    let text = String::from_utf8(out).expect("utf-8");
    assert!(text.contains("scenario 1  median tokens +598"), "{text}");

    retokened(
        &candidate,
        "scenario-1-trial-97",
        "implementation.edit",
        None,
    );
    let card = Scorecard::read(&candidate).expect("the candidate's runs read");
    let mut out = Vec::new();
    let refused = card
        .against(&saved, &mut out)
        .expect_err("refused")
        .to_string();
    assert!(
        refused.contains("scenario-1-trial-97") && refused.contains("implementation.edit"),
        "{refused}"
    );
    assert!(refused.contains("unknown"), "{refused}");
    let text = String::from_utf8(out).expect("utf-8");
    assert!(text.is_empty(), "nothing judged or printed: {text}");
}

/// Acceptance 2: the same on the saved side, and with a harness step recorded as 0.
#[test]
fn an_unknown_on_the_saved_side_or_a_harness_zero_is_refused() {
    let (baseline, candidate) = (side("theirs", "baseline"), side("theirs", "candidate"));
    retokened(&baseline, "scenario-1-trial-88", "change.handover", None);
    let saved = Scorecard::read(&baseline).expect("the trigger's runs read");
    let card = Scorecard::read(&candidate).expect("the candidate's runs read");
    let refused = card
        .against(&saved, &mut Vec::new())
        .expect_err("refused")
        .to_string();
    assert!(
        refused.contains("scenario-1-trial-88") && refused.contains("change.handover"),
        "{refused}"
    );

    let (baseline, candidate) = (side("zero", "baseline"), side("zero", "candidate"));
    retokened(&candidate, "scenario-1-trial-99", "tests.write", Some(0));
    let saved = Scorecard::read(&baseline).expect("the trigger's runs read");
    let card = Scorecard::read(&candidate).expect("the candidate's runs read");
    let refused = card
        .against(&saved, &mut Vec::new())
        .expect_err("refused")
        .to_string();
    assert!(
        refused.contains("scenario-1-trial-99") && refused.contains("tests.write"),
        "{refused}"
    );
}

/// Acceptance 3: the tokens definition moves to version 2, and a saved unknown is `null`, which
/// no reader of a count takes for one; it reads back as unknown.
#[test]
fn the_tokens_definition_moves_and_an_unknown_saves_as_null() {
    assert!(METRICS.contains(&("tokens", 2)), "{METRICS:?}");
    let candidate = side("save", "candidate");
    retokened(
        &candidate,
        "scenario-1-trial-97",
        "implementation.edit",
        None,
    );
    let card = Scorecard::read(&candidate).expect("the candidate's runs read");
    assert_eq!(card.metrics.get("tokens"), Some(&2));
    let saves = scratch("save-dir");
    let path = card
        .clone()
        .save(
            &saves,
            "0c9311cb4b3ae6a432c2847982c252382e41dd3d",
            takt::scorecard::Named::Build,
        )
        .expect("saved");
    let json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).expect("the file")).expect("json");
    let row = json["rows"]
        .as_array()
        .expect("rows")
        .iter()
        .find(|row| row["case"] == "scenario-1-trial-97")
        .expect("the case's run");
    assert!(row["tokens"].is_null(), "{row}");
    assert_eq!(Scorecard::load(&path).expect("loaded").rows, card.rows);
}

/// A saved run whose step is unknown but whose run tokens are a figure contradicts itself and
/// is not read: the run's figure would count the unknown step 0.
#[test]
fn a_saved_run_with_an_unknown_step_and_known_tokens_is_not_read() {
    let candidate = side("forged", "candidate");
    retokened(
        &candidate,
        "scenario-1-trial-97",
        "implementation.edit",
        None,
    );
    let card = Scorecard::read(&candidate).expect("the candidate's runs read");
    let saves = scratch("forged-dir");
    let path = card
        .save(
            &saves,
            "0c9311cb4b3ae6a432c2847982c252382e41dd3d",
            takt::scorecard::Named::Build,
        )
        .expect("saved");
    let mut json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).expect("the file")).expect("json");
    for row in json["rows"].as_array_mut().expect("rows") {
        if row["case"] == "scenario-1-trial-97" {
            row["tokens"] = 50_000.into();
        }
    }
    fs::write(&path, json.to_string()).expect("the forged file");
    let refused = Scorecard::load(&path).expect_err("refused").to_string();
    assert!(
        refused.contains("scenario-1-trial-97") && refused.contains("implementation.edit"),
        "{refused}"
    );
}
