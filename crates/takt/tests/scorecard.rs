//! story:scorecard-reader: the scorecard reads the runs' events under `.engineering/cases/`
//! only, one row per run, and compares each scenario run with a saved scorecard's. The case
//! directories are written through `takt::events`, the format the run loop writes, never as
//! hand-typed lines. Also the run event the scorecard and the metrics need: a step's four token
//! components beside their sum, instants below the second, the base revision on `opened` and
//! the revision a validator observed on its step.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::anyhow;
use takt::case::CASES_DIR;
use takt::events::{BY_TAKT, Event, Events, TokenUsage};
use takt::harness::{Harness, StepContext, StepReport};
use takt::scorecard::{Named, Scorecard};
use takt::validator::{TestsRun, Validator};
use takt::{Run, RunOptions, Stop};

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
        .join(format!("scorecard-{}-{name}", std::process::id()));
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
        base: Some("0123456789abcdef0123456789abcdef01234567".to_owned()),
    }
}

fn decision(seq: usize) -> Event {
    Event::Decision {
        seq,
        file: format!("decisions/{seq:04}.json"),
        frontier: "frontier: tests.run".to_owned(),
    }
}

/// A judgement step that reported `usage`, its sum as the step's tokens.
fn judged(usage: TokenUsage) -> Event {
    Event::Step {
        by: "harness".to_owned(),
        name: "claude-code".to_owned(),
        action: "implementation.edit".to_owned(),
        summary: "edited".to_owned(),
        tokens: Some(usage.total()),
        usage: Some(usage),
        revision: None,
    }
}

fn observed() -> Event {
    Event::Step {
        by: "validator".to_owned(),
        name: "takt.tests-run".to_owned(),
        action: "tests.run".to_owned(),
        summary: "pass".to_owned(),
        tokens: None,
        usage: None,
        revision: Some("89abcdef0123456789abcdef0123456789abcdef".to_owned()),
    }
}

fn usage(input: u64, output: u64, cache_read: u64, cache_creation: u64) -> TokenUsage {
    TokenUsage {
        input,
        output,
        cache_read,
        cache_creation,
    }
}

fn stop(stop: &str, steps: u64, tokens: u64) -> Event {
    Event::Stop {
        stop: stop.to_owned(),
        steps,
        tokens,
    }
}

/// Writes `runs` to the case `case`'s record under `repo`, each run's steps numbered from 1 as
/// the loop numbers them; the case directory.
fn record(repo: &Path, case: &str, runs: &[Vec<Event>]) -> PathBuf {
    let dir = repo.join(CASES_DIR).join(case);
    fs::create_dir_all(&dir).expect("a case directory");
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

/// Two scenario 1 runs as a scorecard reads them: trial 1 accepted over two judgement steps;
/// trial 2 stopped `NoProgress` after a refusal, then resumed and ended on an error. One case
/// beside them has no record at all.
fn two_scenario_one_runs(repo: &Path) {
    record(
        repo,
        "scenario-1-trial-1",
        &[vec![
            opened("scenario-1-trial-1"),
            decision(1),
            observed(),
            decision(2),
            judged(usage(1000, 150, 40, 10)),
            decision(3),
            judged(usage(600, 100, 80, 20)),
            stop("Completed (accepted)", 3, 2000),
        ]],
    );
    record(
        repo,
        "scenario-1-trial-2",
        &[
            vec![
                opened("scenario-1-trial-2"),
                decision(1),
                judged(usage(200, 60, 30, 10)),
                Event::Refusal {
                    by: BY_TAKT.to_owned(),
                    reason: "the step changed a path that is no test".to_owned(),
                },
                stop("NoProgress", 1, 300),
            ],
            vec![
                opened("scenario-1-trial-2"),
                decision(2),
                judged(usage(40, 5, 0, 0)),
                Event::Error {
                    error: "the harness failed".to_owned(),
                },
            ],
        ],
    );
    fs::create_dir_all(repo.join(CASES_DIR).join("takt-20261018T100000Z"))
        .expect("a case directory without a record");
}

/// The tokens the harness reported in each run of `dir`'s record, read back through
/// [`Events::read`] apart from the scorecard: the `tokens` of each `step` line, summed from one
/// `opened` line to the next.
fn reported(dir: &Path) -> Vec<u64> {
    let mut runs: Vec<u64> = Vec::new();
    for line in Events::new(dir).read().expect("the record reads") {
        if line["event"] == "opened" {
            runs.push(0);
        } else if line["event"] == "step"
            && let (Some(run), Some(tokens)) = (runs.last_mut(), line["tokens"].as_u64())
        {
            *run += tokens;
        }
    }
    runs
}

/// What the scorecard printed for `case`, one entry a run, split at white space.
fn printed<'a>(text: &'a str, case: &str) -> Vec<Vec<&'a str>> {
    text.lines()
        .filter(|line| line.split_whitespace().next() == Some(case))
        .map(|line| line.split_whitespace().collect())
        .collect()
}

fn print(card: &Scorecard) -> String {
    let mut out = Vec::new();
    card.print(&mut out).expect("the scorecard prints");
    String::from_utf8(out).expect("utf-8")
}

/// The acceptance: over the case directories of two scenario 1 runs, the scorecard prints each
/// run's tokens equal to the sum of the harness-reported tokens in that run's events.
#[test]
fn each_runs_tokens_are_the_sum_its_events_report() {
    let repo = scratch("acceptance");
    two_scenario_one_runs(&repo);
    let card = Scorecard::read(&repo).expect("the scorecard reads");
    let text = print(&card);

    let header: Vec<&str> = text
        .lines()
        .find(|line| line.starts_with("case "))
        .expect("a header")
        .split_whitespace()
        .collect();
    let tokens = header.iter().position(|c| *c == "tokens").expect("tokens");
    for case in ["scenario-1-trial-1", "scenario-1-trial-2"] {
        let rows = printed(&text, case);
        let sums = reported(&repo.join(CASES_DIR).join(case));
        assert_eq!(rows.len(), sums.len(), "{text}");
        for (row, sum) in rows.iter().zip(&sums) {
            assert_eq!(row[tokens], sum.to_string(), "{case}: {text}");
        }
    }
    assert_eq!(
        reported(&repo.join(CASES_DIR).join("scenario-1-trial-1")),
        [2000]
    );
    assert_eq!(
        reported(&repo.join(CASES_DIR).join("scenario-1-trial-2")),
        [300, 45]
    );
}

/// A row's case, scenario, outcome, steps, tokens, refusals, `NoProgress` and errors.
type Figures<'a> = (&'a str, Option<&'a str>, &'a str, u64, u64, u64, u64, u64);

/// Every figure of a row: case, scenario, outcome, steps, tokens, refusals, `NoProgress`,
/// errors; and a case with no record is listed as one, never guessed.
#[test]
fn a_row_per_run_with_every_figure() {
    let repo = scratch("rows");
    two_scenario_one_runs(&repo);
    let card = Scorecard::read(&repo).expect("the scorecard reads");
    let figures: Vec<Figures<'_>> = card
        .rows
        .iter()
        .map(|row| {
            (
                row.case.as_str(),
                row.scenario.as_deref(),
                row.outcome.as_str(),
                row.steps,
                row.tokens,
                row.refusals,
                row.no_progress,
                row.errors,
            )
        })
        .collect();
    assert_eq!(
        figures,
        [
            (
                "scenario-1-trial-1",
                Some("1"),
                "accepted",
                3,
                2000,
                0,
                0,
                0
            ),
            (
                "scenario-1-trial-2",
                Some("1"),
                "NoProgress",
                1,
                300,
                1,
                1,
                0
            ),
            ("scenario-1-trial-2", Some("1"), "error", 1, 45, 0, 0, 1),
        ]
    );
    assert_eq!(card.no_record, ["takt-20261018T100000Z"]);
    let text = print(&card);
    assert!(
        text.lines()
            .any(|line| line.split_whitespace().collect::<Vec<_>>()
                == ["takt-20261018T100000Z", "no", "record"]),
        "{text}"
    );
}

/// The scenario comes from a case id `scenario-<id>-trial-<n>`, and from no other shape.
#[test]
fn the_scenario_is_read_from_the_case_id_only() {
    for (case, scenario) in [
        ("scenario-1-trial-1", Some("1")),
        ("scenario-login-flow-trial-12", Some("login-flow")),
        ("scenario-1-trial-", None),
        ("scenario-1-trial-x", None),
        ("scenario--trial-1", None),
        ("story-one-20261018T100000Z", None),
        ("takt-20261018T100000Z", None),
    ] {
        assert_eq!(takt::scorecard::scenario_of(case), scenario, "{case}");
    }
}

/// Rewrites the instant of every line of `dir`'s record, in order, keeping everything else the
/// loop wrote.
fn instants(dir: &Path, at: &[&str]) {
    let lines = Events::new(dir).read().expect("the record reads");
    assert_eq!(lines.len(), at.len());
    let mut text = String::new();
    for (mut line, at) in lines.into_iter().zip(at) {
        line["at"] = (*at).into();
        text.push_str(&serde_json::to_string(&line).expect("json"));
        text.push('\n');
    }
    fs::write(dir.join(takt::events::EVENTS_FILE), text).expect("the record");
}

/// Wall time is the first to the last instant of the run, below the second where the record
/// has it, and from a record written before instants had a fraction.
#[test]
fn wall_time_spans_the_runs_first_to_last_instant() {
    let repo = scratch("wall");
    let new = record(
        &repo,
        "scenario-2-trial-1",
        &[vec![
            opened("scenario-2-trial-1"),
            judged(usage(1, 1, 1, 1)),
            stop("NoProgress", 1, 4),
        ]],
    );
    instants(
        &new,
        &[
            "2026-10-18T10:00:00.250000Z",
            "2026-10-18T10:00:01.000000Z",
            "2026-10-18T10:00:02.000000Z",
        ],
    );
    let old = record(
        &repo,
        "scenario-2-trial-2",
        &[vec![opened("scenario-2-trial-2"), stop("NoProgress", 0, 0)]],
    );
    instants(&old, &["2026-10-18T23:59:59Z", "2026-10-19T00:00:02Z"]);
    let card = Scorecard::read(&repo).expect("the scorecard reads");
    let walls: Vec<u64> = card.rows.iter().map(|row| row.wall_ms).collect();
    assert_eq!(walls, [1750, 3000]);
}

/// A run's line that predates the four components, the base and the observed revision reads
/// as it did: they are optional, and the scorecard needs none of them.
#[test]
fn a_record_without_the_new_fields_reads() {
    let repo = scratch("old");
    let dir = record(
        &repo,
        "scenario-3-trial-1",
        &[vec![
            opened("scenario-3-trial-1"),
            judged(usage(5, 6, 7, 8)),
            stop("Completed (accepted)", 1, 26),
        ]],
    );
    let lines = Events::new(&dir).read().expect("the record reads");
    let mut text = String::new();
    for mut line in lines {
        let object = line.as_object_mut().expect("an object");
        object.remove("usage");
        object.remove("base");
        object.remove("revision");
        line["at"] = "2026-10-18T10:00:00Z".into();
        text.push_str(&serde_json::to_string(&line).expect("json"));
        text.push('\n');
    }
    fs::write(dir.join(takt::events::EVENTS_FILE), text).expect("the record");
    let card = Scorecard::read(&repo).expect("the scorecard reads");
    assert_eq!(card.rows.len(), 1);
    assert_eq!(card.rows[0].tokens, 26);
    assert_eq!(card.rows[0].outcome, "accepted");
}

/// The first check: a planted `task_spend` evidence file in the candidate tree, under the case
/// and anywhere else, changes no number the scorecard prints.
#[test]
fn a_planted_task_spend_changes_no_number() {
    let repo = scratch("planted");
    two_scenario_one_runs(&repo);
    let before = print(&Scorecard::read(&repo).expect("the scorecard reads"));
    let spend = r#"{"kind":"task_spend","tokens":1,"result":"pass","steps":1,"wall_ms":1}"#;
    for dir in [
        repo.to_path_buf(),
        repo.join(".engineering"),
        repo.join(CASES_DIR).join("scenario-1-trial-1"),
        repo.join(CASES_DIR)
            .join("scenario-1-trial-1")
            .join("evidence"),
        repo.join(CASES_DIR).join("takt-20261018T100000Z"),
    ] {
        fs::create_dir_all(&dir).expect("a directory");
        fs::write(dir.join("task_spend.json"), spend).expect("a planted file");
        fs::write(dir.join("task_spend"), spend).expect("a planted file");
    }
    let after = print(&Scorecard::read(&repo).expect("the scorecard reads"));
    assert_eq!(before, after);
}

/// The second check: against a saved scorecard in which scenario 1 was `accepted`, a
/// `NoProgress` run of scenario 1 is marked worse, with its token difference.
#[test]
fn against_a_saved_accepted_scenario_a_no_progress_row_is_worse() {
    let baseline = scratch("baseline");
    record(
        &baseline,
        "scenario-1-trial-1",
        &[vec![
            opened("scenario-1-trial-1"),
            judged(usage(400, 50, 40, 10)),
            stop("Completed (accepted)", 1, 500),
        ]],
    );
    let saves = scratch("saves");
    let saved = Scorecard::read(&baseline)
        .expect("the scorecard reads")
        .save(
            &saves,
            "1111111111111111111111111111111111111111",
            Named::Build,
        )
        .expect("the scorecard is saved");
    assert_eq!(
        saved.file_name().and_then(|name| name.to_str()),
        Some("scorecard-1111111111111111111111111111111111111111.json")
    );

    let candidate = scratch("candidate");
    record(
        &candidate,
        "scenario-1-trial-2",
        &[vec![
            opened("scenario-1-trial-2"),
            judged(usage(600, 80, 10, 10)),
            stop("NoProgress", 1, 700),
        ]],
    );
    record(
        &candidate,
        "scenario-1-trial-3",
        &[vec![
            opened("scenario-1-trial-3"),
            judged(usage(300, 50, 0, 0)),
            stop("Completed (accepted)", 1, 350),
        ]],
    );
    let card = Scorecard::read(&candidate).expect("the scorecard reads");
    let mut out = Vec::new();
    let worse = card
        .against(
            &Scorecard::load(&saved).expect("the saved scorecard reads"),
            &mut out,
        )
        .expect("the comparison prints");
    let text = String::from_utf8(out).expect("utf-8");
    assert_eq!(worse, 1, "{text}");
    let row = |case: &str| {
        text.lines()
            .find(|line| line.split_whitespace().next() == Some(case))
            .unwrap_or_else(|| panic!("{case} in {text}"))
            .to_owned()
    };
    let no_progress = row("scenario-1-trial-2");
    assert!(no_progress.ends_with(" worse"), "{no_progress}");
    assert!(!no_progress.contains("not worse"), "{no_progress}");
    let accepted = row("scenario-1-trial-3");
    assert!(accepted.ends_with("not worse"), "{accepted}");
    assert!(
        text.contains("scenario 1  saved      n 1  tokens min 500 median 500 max 500"),
        "{text}"
    );
    assert!(
        text.contains("scenario 1  candidate  n 2  tokens min 350 median 525 max 700"),
        "{text}"
    );
    assert!(text.contains("scenario 1  median tokens +25"), "{text}");
}

/// Token totals of one scenario vary from run to run with no change, so with several runs a
/// side the comparison prints, per scenario and side, n and the minimum, median and maximum of
/// tokens, and the difference of the medians; tokens never mark a row worse, only its outcome
/// does. The saved scorecard keeps every run's figures, so the spread can be read again.
#[test]
fn several_runs_a_side_print_the_token_spread_and_flag_no_row_on_tokens() {
    let accepted = |repo: &Path, trial: u64, tokens: u64| {
        let case = format!("scenario-1-trial-{trial}");
        record(
            repo,
            &case,
            &[vec![
                opened(&case),
                judged(usage(tokens, 0, 0, 0)),
                stop("Completed (accepted)", 1, tokens),
            ]],
        );
    };
    let baseline = scratch("spread-baseline");
    for (trial, tokens) in [(1, 106_003), (2, 167_249), (3, 131_500)] {
        accepted(&baseline, trial, tokens);
    }
    let saves = scratch("spread-saves");
    let saved = Scorecard::read(&baseline)
        .expect("the scorecard reads")
        .save(
            &saves,
            "5555555555555555555555555555555555555555",
            Named::Head,
        )
        .expect("saved");
    let file: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&saved).expect("the file")).expect("json");
    let kept: Vec<u64> = file["rows"]
        .as_array()
        .expect("rows")
        .iter()
        .map(|row| row["tokens"].as_u64().expect("tokens"))
        .collect();
    assert_eq!(kept, [106_003, 167_249, 131_500]);

    let candidate = scratch("spread-candidate");
    for (trial, tokens) in [(4, 160_000), (5, 120_000), (6, 140_250)] {
        accepted(&candidate, trial, tokens);
    }
    let mut out = Vec::new();
    let worse = Scorecard::read(&candidate)
        .expect("the scorecard reads")
        .against(&Scorecard::load(&saved).expect("reads"), &mut out)
        .expect("the comparison prints");
    let text = String::from_utf8(out).expect("utf-8");
    assert_eq!(worse, 0, "{text}");
    for trial in 4..=6 {
        let case = format!("scenario-1-trial-{trial}");
        let line = text
            .lines()
            .find(|line| line.split_whitespace().next() == Some(case.as_str()))
            .unwrap_or_else(|| panic!("{case} in {text}"));
        assert!(line.ends_with("not worse"), "{line}");
    }
    for line in [
        "scenario 1  saved      n 3  tokens min 106003 median 131500 max 167249",
        "scenario 1  candidate  n 3  tokens min 120000 median 140250 max 160000",
        "scenario 1  median tokens +8750",
    ] {
        assert!(text.contains(line), "{line} in {text}");
    }
}

/// Outcomes rank `accepted` above an approval or a wait, above a budget stop, above
/// `NoProgress`, above an error.
#[test]
fn outcomes_rank_in_the_order_the_story_gives() {
    use takt::scorecard::rank;
    let order = [
        ["accepted", "accepted"],
        ["ApprovalRequired", "Waiting"],
        ["Iterations", "Tokens"],
        ["Hours", "Hours"],
        ["NoProgress", "NoProgress"],
        ["error", "error"],
    ];
    assert_eq!(rank("ApprovalRequired"), rank("Waiting"));
    assert_eq!(rank("Iterations"), rank("Hours"));
    assert_eq!(rank("Hours"), rank("Tokens"));
    for pair in order.windows(2) {
        for high in pair[0] {
            for low in pair[1] {
                if rank(high) != rank(low) {
                    assert!(rank(high) > rank(low), "{high} above {low}");
                }
            }
        }
    }
    assert!(rank("accepted") > rank("ApprovalRequired"));
    assert!(rank("Waiting") > rank("Iterations"));
    assert!(rank("Tokens") > rank("NoProgress"));
    assert!(rank("NoProgress") > rank("error"));
}

/// The third check: against a saved scorecard whose token definition is another version, both
/// figures are printed, each with its version, and no difference; once the candidate is saved
/// under the current definitions, the next comparison prints one figure again: one cycle.
#[test]
fn a_changed_metric_definition_prints_both_figures_for_one_cycle() {
    let repo = scratch("definitions");
    record(
        &repo,
        "scenario-1-trial-1",
        &[vec![
            opened("scenario-1-trial-1"),
            judged(usage(400, 50, 40, 10)),
            stop("Completed (accepted)", 1, 500),
        ]],
    );
    let saves = scratch("definition-saves");
    let card = Scorecard::read(&repo).expect("the scorecard reads");
    let current = card.metrics["tokens"];
    let saved = card
        .clone()
        .save(
            &saves,
            "2222222222222222222222222222222222222222",
            Named::Head,
        )
        .expect("saved");
    let mut previous: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&saved).expect("the file")).expect("json");
    previous["metrics"]["tokens"] = (current + 1).into();
    previous["rows"][0]["tokens"] = 450.into();
    fs::write(&saved, serde_json::to_string(&previous).expect("json")).expect("the file");

    let mut out = Vec::new();
    card.against(&Scorecard::load(&saved).expect("reads"), &mut out)
        .expect("the comparison prints");
    let text = String::from_utf8(out).expect("utf-8");
    let both = format!("median tokens v{} 450 v{current} 500", current + 1);
    assert!(text.contains(&both), "{both} in {text}");
    assert!(!text.contains("tokens +"), "{text}");
    assert!(!text.contains("tokens -"), "{text}");

    let next = card
        .clone()
        .save(
            &saves,
            "3333333333333333333333333333333333333333",
            Named::Head,
        )
        .expect("saved");
    let mut out = Vec::new();
    card.against(&Scorecard::load(&next).expect("reads"), &mut out)
        .expect("the comparison prints");
    let text = String::from_utf8(out).expect("utf-8");
    assert!(text.contains("median tokens +0"), "{text}");
    assert!(!text.contains(&format!("v{current}")), "{text}");
}

/// The saved scorecard carries its metric definitions' versions, the revision that names it
/// and what named it.
#[test]
fn a_saved_scorecard_names_its_revision_and_definitions() {
    let repo = scratch("saved");
    two_scenario_one_runs(&repo);
    let saves = scratch("saved-dir");
    let card = Scorecard::read(&repo).expect("the scorecard reads");
    let path = card
        .clone()
        .save(
            &saves,
            "4444444444444444444444444444444444444444",
            Named::Head,
        )
        .expect("saved");
    let loaded = Scorecard::load(&path).expect("reads");
    assert_eq!(
        loaded.revision.as_deref(),
        Some("4444444444444444444444444444444444444444")
    );
    assert_eq!(loaded.named_by, Some(Named::Head));
    assert_eq!(loaded.rows, card.rows);
    assert_eq!(loaded.no_record, card.no_record);
    for metric in [
        "outcome",
        "steps",
        "tokens",
        "refusals",
        "no_progress",
        "errors",
        "wall",
    ] {
        assert!(loaded.metrics.contains_key(metric), "{metric}");
    }
}

/// Runs `git <args>` in `tree`; its trimmed standard output.
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

/// One validator-owned action, `tests.run`; accepted once the tests pass.
const TESTS_ONLY: &str = "format: protocol/1

protocol:
  id: tests.only
  revision: 1
  description: >-
    Run the tests against the implementation at its current revision; accepted once they pass.

artifacts:
  implementation:
    description: a revision of the code

evidence_kinds:
  test_result:
    description: the result of running tests against an implementation revision

claims:
  tests.pass:
    description: the tests pass for the current implementation revision
    true_when:
      evidence:
        kind: test_result
        result: pass
        subject: implementation

obligations:
  tests.required:
    description: the tests pass for the current implementation revision
    discharged_when:
      claim: tests.pass

actions:
  tests.run:
    description: run the tests against an implementation revision
    effect: read
    may_produce:
      - evidence: test_result

outcomes:
  accepted:
    description: the tests pass for the current implementation revision
    requires:
      claim: tests.pass
";

/// A harness the run never reaches: its only step is a validator's.
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

/// The run event as the loop writes it now: instants below the second, the base revision on
/// `opened`, the revision the validator observed on its step.
#[test]
fn the_loop_records_the_base_the_observed_revision_and_sub_second_instants() {
    let repo = scratch("loop");
    fs::write(repo.join("check.txt"), "one\n").expect("a file");
    fs::write(repo.join("tests-only.yaml"), TESTS_ONLY).expect("the protocol");
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "start"]);
    let base = git(&repo, &["rev-parse", "HEAD"]);
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(TestsRun::new("true").expect("a command"))];
    let options = RunOptions {
        protocol: repo.join("tests-only.yaml").display().to_string(),
        intent: "check the file".to_owned(),
        inputs: Vec::new(),
        iterations: Some(4),
        hours: None,
        tokens: None,
        case_id: Some("scenario-1-trial-9".to_owned()),
        gate: Some("repository.merge".to_owned()),
    };
    let mut run = Run::open(&repo, options, &NoHarness, &validators).expect("the case opens");
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let text = String::from_utf8_lossy(&out).into_owned();
    assert!(
        matches!(&stop, Ok(Stop::Completed(outcome)) if outcome == "accepted"),
        "{stop:?}: {text}"
    );
    let lines = Events::new(&run.case.dir).read().expect("the record reads");
    let opened = lines
        .iter()
        .find(|line| line["event"] == "opened")
        .expect("an opened line");
    assert_eq!(opened["base"], base.as_str(), "{opened}");
    let step = lines
        .iter()
        .find(|line| line["event"] == "step" && line["by"] == "validator")
        .expect("a validator's step");
    assert_eq!(step["revision"], base.as_str(), "{step}");
    for line in &lines {
        let at = line["at"].as_str().expect("an instant");
        let (seconds, fraction) = at
            .strip_suffix('Z')
            .and_then(|at| at.split_once('.'))
            .unwrap_or_else(|| panic!("{at} has a fraction"));
        assert_eq!(seconds.len(), 19, "{at}");
        assert!(
            fraction.len() == 6 && fraction.bytes().all(|b| b.is_ascii_digit()),
            "{at}"
        );
    }
}

/// A step event carries the four components the harness reports beside their sum, and they
/// read back as written.
#[test]
fn a_step_carries_the_four_token_components_beside_their_sum() {
    let repo = scratch("usage");
    let dir = record(
        &repo,
        "scenario-1-trial-1",
        &[vec![
            opened("scenario-1-trial-1"),
            judged(usage(11, 22, 33, 44)),
        ]],
    );
    let lines = Events::new(&dir).read().expect("the record reads");
    let step = &lines[1];
    assert_eq!(step["tokens"], 110, "{step}");
    assert_eq!(
        step["usage"],
        serde_json::json!({"input": 11, "output": 22, "cache_read": 33, "cache_creation": 44}),
        "{step}"
    );
}

/// A `store_evidence` line keeps its own instant, below the second, beside the instant given
/// to the store: one key each, so neither shadows the other on read.
#[test]
fn a_store_evidence_line_keeps_its_own_instant_beside_the_records() {
    let repo = scratch("store-instant");
    let dir = record(
        &repo,
        "scenario-1-trial-1",
        &[vec![
            opened("scenario-1-trial-1"),
            Event::StoreEvidence {
                story: "story:scratch-one".to_owned(),
                kind: "test_result".to_owned(),
                reference: ".engineering/cases/scenario-1-trial-1/evidence/0001.json".to_owned(),
                observed_at: "2026-10-18T10:00:00Z".to_owned(),
                commit: None,
            },
        ]],
    );
    let text = fs::read_to_string(dir.join(takt::events::EVENTS_FILE)).expect("the record");
    let line = text.lines().nth(1).expect("the store_evidence line");
    assert_eq!(line.matches("\"at\":").count(), 1, "{line}");
    let read = &Events::new(&dir).read().expect("the record reads")[1];
    assert_eq!(read["observed_at"], "2026-10-18T10:00:00Z", "{read}");
    assert!(
        read["at"].as_str().is_some_and(|at| at.len() == 27),
        "{read}"
    );
}
