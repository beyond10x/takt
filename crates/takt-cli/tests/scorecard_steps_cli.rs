//! story:the-judge-compares-the-steps-a-candidate-targets through the binary: `takt scorecard
//! --against <saved> --steps <action>[,<action>...]` re-judges candidate 1 of the 0.3.0 trial
//! cycle from its recorded runs (the fixture `judge-steps-0.3.0-cand-1` of the library crate),
//! exit 0 better on `change.handover`; exit 4 on a step it does not make better; `--steps`
//! without `--against` does not parse; and a saved scorecard without the `step_tokens`
//! definition is refused, exit 1.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use takt::case::CASES_DIR;

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
        .join(format!("scorecard-steps-cli-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(&dir).expect("a scratch tree");
    Scratch(dir)
}

fn takt(args: &[&str]) -> (i32, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args(args)
        .output()
        .expect("takt runs");
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// The revision the trigger's runs were made with, as their `opened` lines name it.
const TRIGGER: &str = "bee77822aa9ac86df153de9871b962b419d195f3";

const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../takt/tests/fixtures/judge-steps-0.3.0-cand-1"
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

/// The trigger's scorecard, saved under `saves`.
fn saved_trigger(name: &str, saves: &Path) -> PathBuf {
    let baseline = side(name, "baseline");
    let (code, text, stderr) = takt(&[
        "scorecard",
        "--repo",
        &baseline.display().to_string(),
        "--save",
        &saves.display().to_string(),
        "--revision",
        TRIGGER,
    ]);
    assert_eq!(code, 0, "{text}{stderr}");
    saves.join(format!("scorecard-{TRIGGER}.json"))
}

fn judged(name: &str, steps: &str) -> (i32, String, String) {
    let saves = scratch(&format!("{name}-saves"));
    let saved = saved_trigger(name, &saves);
    let candidate = side(name, "candidate");
    takt(&[
        "scorecard",
        "--repo",
        &candidate.display().to_string(),
        "--against",
        &saved.display().to_string(),
        "--steps",
        steps,
    ])
}

/// Acceptances 1 and 3: better on `change.handover`, each case's figure printed beside the
/// saved side's cheapest, and not worse on the run: exit 0.
#[test]
fn candidate_1_is_better_on_change_handover_exit_0() {
    let (code, text, stderr) = judged("better", "change.handover");
    assert_eq!(code, 0, "{text}{stderr}");
    assert!(
        text.contains(
            "scenario 1  step change.handover  candidate 8405 8588 8528 8562 8481  saved 15884 15922 15875 15962 15877  saved cheapest 15875"
        ),
        "{text}"
    );
    assert!(
        text.contains("verdict  better on its steps (change.handover), not worse on the run"),
        "{text}"
    );
}

/// Acceptance 1: one of two targeted steps not better is exit 4, the step named.
#[test]
fn a_step_not_better_is_exit_4() {
    let (code, text, stderr) = judged("not-better", "change.handover,tests.write");
    assert_eq!(code, 4, "{text}{stderr}");
    assert!(
        text.contains(
            "not better on its steps (change.handover, tests.write; not better: tests.write)"
        ),
        "{text}"
    );
}

/// `--steps` judges against a saved side: without `--against` the arguments do not parse.
#[test]
fn steps_without_against_does_not_parse() {
    let candidate = side("no-against", "candidate");
    let (code, text, stderr) = takt(&[
        "scorecard",
        "--repo",
        &candidate.display().to_string(),
        "--steps",
        "change.handover",
    ]);
    assert_eq!(code, 2, "{text}{stderr}");
}

/// Acceptance 4: a saved scorecard from before the `step_tokens` definition has no tokens by
/// step: refused, exit 1, the metric named.
#[test]
fn a_saved_scorecard_without_step_tokens_is_refused() {
    let saves = scratch("old-saves");
    let saved = saved_trigger("old", &saves);
    let mut file: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&saved).expect("the saved scorecard"))
            .expect("json");
    file["metrics"]
        .as_object_mut()
        .expect("metrics")
        .remove("step_tokens");
    for row in file["rows"].as_array_mut().expect("rows") {
        row.as_object_mut().expect("a row").remove("step_tokens");
    }
    fs::write(&saved, serde_json::to_string_pretty(&file).expect("json")).expect("written");
    let candidate = side("old", "candidate");
    let (code, text, stderr) = takt(&[
        "scorecard",
        "--repo",
        &candidate.display().to_string(),
        "--against",
        &saved.display().to_string(),
        "--steps",
        "change.handover",
    ]);
    assert_eq!(code, 1, "{text}{stderr}");
    assert!(stderr.contains("step_tokens"), "{stderr}");
}

/// Removes `tokens` and `usage` from the first harness step of `action` in `case`'s record
/// under `repo`: a step the harness reported no tokens for.
fn untokened(repo: &Path, case: &str, action: &str) {
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
                assert!(object.remove("tokens").is_some(), "the step had tokens");
                object.remove("usage");
                done = true;
            }
            value.to_string()
        })
        .collect();
    assert!(done, "{case} has a harness step of {action}");
    fs::write(&path, lines.join("\n") + "\n").expect("written");
}

/// C1: a candidate case whose targeted step recorded no tokens is refused, exit 1, the case and
/// the step named: a missing record is never read as a saving.
#[test]
fn a_candidate_step_without_tokens_is_refused_exit_1() {
    let saves = scratch("unknown-mine-saves");
    let saved = saved_trigger("unknown-mine", &saves);
    let candidate = side("unknown-mine", "candidate");
    untokened(&candidate, "scenario-1-trial-97", "change.handover");
    let (code, text, stderr) = takt(&[
        "scorecard",
        "--repo",
        &candidate.display().to_string(),
        "--against",
        &saved.display().to_string(),
        "--steps",
        "change.handover",
    ]);
    assert_eq!(code, 1, "{text}{stderr}");
    assert!(
        stderr.contains("scenario-1-trial-97") && stderr.contains("change.handover"),
        "{stderr}"
    );
    assert!(!text.contains("verdict"), "{text}");
}

/// C1: the same on the saved side, its unknown saved with the scorecard and read back.
#[test]
fn a_saved_step_without_tokens_is_refused_exit_1() {
    let saves = scratch("unknown-theirs-saves");
    let baseline = side("unknown-theirs", "baseline");
    untokened(&baseline, "scenario-1-trial-88", "change.handover");
    let (code, text, stderr) = takt(&[
        "scorecard",
        "--repo",
        &baseline.display().to_string(),
        "--save",
        &saves.display().to_string(),
        "--revision",
        TRIGGER,
    ]);
    assert_eq!(code, 0, "{text}{stderr}");
    let saved = saves.join(format!("scorecard-{TRIGGER}.json"));
    let candidate = side("unknown-theirs", "candidate");
    let (code, text, stderr) = takt(&[
        "scorecard",
        "--repo",
        &candidate.display().to_string(),
        "--against",
        &saved.display().to_string(),
        "--steps",
        "change.handover",
    ]);
    assert_eq!(code, 1, "{text}{stderr}");
    assert!(
        stderr.contains("scenario-1-trial-88") && stderr.contains("change.handover"),
        "{stderr}"
    );
}
