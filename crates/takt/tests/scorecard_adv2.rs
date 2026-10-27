//! Adversary pass 2 on story:scorecard-reader, after correction 1, held to the rule
//! story:promotion decided: the case is the unit of comparison and a scenario is worse when,
//! at some outcome rank, its cases at that rank or better fall short of the saved share by more
//! than a margin of one case, or when its median of tokens exceeds the saved maximum. Two
//! properties are asked of that rule: a scorecard compared with itself is never worse, and a
//! candidate worse by two cases of one scenario (none better) always is; one case worse is the
//! margin. Records are written through `takt::events`, as the run loop writes them.

use std::fs;
use std::path::{Path, PathBuf};

use takt::case::CASES_DIR;
use takt::events::{EVENTS_FILE, Event, Events};
use takt::scorecard::{Named, Row, Scorecard, rank};

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
        .join(format!("scorecard-adv2-{}-{name}", std::process::id()));
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
        layers: None,
        confinement: None,
        endpoint: None,
        records: None,
        scan: None,
        protocol_source: None,
        input_sources: None,
        harness_program: None,
        preflight: None,
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
        version: None,
        cost_usd: None,
        duration_ms: None,
        api_ms: None,
        turns: None,
        started_at: None,
        transcript: None,
        transcript_refused: None,
    }
}

/// One run that ends as `outcome` says: a stop as the loop prints it, or `error`.
fn run(case: &str, outcome: &str) -> Vec<Event> {
    let last = if outcome == "error" {
        Event::Error {
            error: "the harness failed".to_owned(),
        }
    } else {
        Event::Stop {
            stop: outcome.to_owned(),
            steps: 1,
            tokens: 100,
            completed: None,
        }
    };
    vec![opened(case), judged(100), last]
}

/// Writes `runs` to the case `case`'s record under `repo` through `takt::events`, and for a
/// scenario case a held-out result that passed beside it.
fn record(repo: &Path, case: &str, runs: &[Vec<Event>]) {
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
}

/// A repository whose scenario-1 trials ended as `outcomes`, one run each.
fn trials(name: &str, outcomes: &[&str]) -> Scratch {
    let repo = scratch(name);
    for (n, outcome) in outcomes.iter().enumerate() {
        let case = format!("scenario-1-trial-{}", n + 1);
        record(&repo, &case, &[run(&case, outcome)]);
    }
    repo
}

const ACCEPTED: &str = "Completed (accepted)";
const NO_PROGRESS: &str = "NoProgress";

/// `candidate` compared with `saved`, saved first as `takt scorecard --save` saves it, under a
/// rule of one trial: these cases are about the ranks and the margin at any n, and five trials
/// are story:scorecard-against-needs-the-trials'.
fn compare(candidate: &Path, saved: &Path, name: &str) -> (usize, String) {
    let (worse, text) = compared(candidate, saved, name);
    (worse.expect("the comparison prints"), text)
}

/// As [`compare`], the comparison's result as it is, a refusal included.
fn compared(candidate: &Path, saved: &Path, name: &str) -> (anyhow::Result<usize>, String) {
    let saves = scratch(&format!("{name}-saves"));
    let file = Scorecard::read(saved)
        .expect("the saved side reads")
        .save(
            &saves,
            "abcdef1234567890abcdef1234567890abcdef12",
            Named::Build,
        )
        .expect("saved");
    let mut card = Scorecard::read(candidate).expect("the candidate reads");
    card.rule.trials = 1;
    let mut out = Vec::new();
    let worse = card.against(&Scorecard::load(&file).expect("loads"), &mut out);
    (worse, String::from_utf8(out).expect("utf-8"))
}

/// Five trials of scenario 1, all accepted; the candidate keeps three and ends two on an error.
/// No case is better and two are worse, and the median of five is the third: not worse.
#[test]
fn a_candidate_that_regresses_two_of_five_trials_is_worse() {
    let saved = trials("two-of-five-saved", &[ACCEPTED; 5]);
    let candidate = trials(
        "two-of-five-candidate",
        &[ACCEPTED, ACCEPTED, ACCEPTED, "error", "error"],
    );
    let (worse, text) = compare(&candidate, &saved, "two-of-five");
    assert!(
        worse > 0,
        "two of five trials went from accepted to error, and the scenario is not worse:\n{text}"
    );
}

/// One saved case, accepted; five candidate cases, two of them `NoProgress`.
#[test]
fn one_saved_case_against_five_candidate_cases_two_regressed_is_worse() {
    let saved = trials("one-five-saved", &[ACCEPTED]);
    let candidate = trials(
        "one-five-candidate",
        &[ACCEPTED, NO_PROGRESS, ACCEPTED, NO_PROGRESS, ACCEPTED],
    );
    let (worse, text) = compare(&candidate, &saved, "one-five");
    assert!(
        worse > 0,
        "two of five candidate cases stopped NoProgress against one accepted saved case:\n{text}"
    );
}

/// Saved: three accepted and one error. One accepted trial turned into `NoProgress` is within
/// the rule's margin of one case (a flaky run); two turned are not.
#[test]
fn one_regressed_trial_is_within_the_margin_and_two_are_not() {
    let saved = trials("even-saved", &[ACCEPTED, ACCEPTED, ACCEPTED, "error"]);
    let one = trials(
        "even-candidate-one",
        &[ACCEPTED, ACCEPTED, NO_PROGRESS, "error"],
    );
    let (worse, text) = compare(&one, &saved, "even-one");
    assert_eq!(
        worse, 0,
        "one trial went from accepted to NoProgress, within the margin of one:\n{text}"
    );
    let two = trials(
        "even-candidate-two",
        &[ACCEPTED, NO_PROGRESS, NO_PROGRESS, "error"],
    );
    let (worse, text) = compare(&two, &saved, "even-two");
    assert!(
        worse > 0,
        "two trials went from accepted to NoProgress, past the margin:\n{text}"
    );
}

/// A case accepted, then resumed (`takt run --case`; nothing refuses a resume of an accepted
/// case) by a run that ended on an error before any stop. The case was accepted; the case-level
/// outcome takes the last run's and reads `error`.
#[test]
fn an_accepted_case_whose_later_run_errored_stays_accepted() {
    let repo = scratch("accepted-then-error");
    let case = "scenario-1-trial-1";
    record(&repo, case, &[run(case, ACCEPTED), run(case, "error")]);
    let card = Scorecard::read(&repo).expect("the scorecard reads");
    let cases = card.cases();
    assert_eq!(
        cases[0].outcome, "accepted",
        "an accepted case reads as {:?} after a later run failed",
        cases[0].outcome
    );
}

/// story:the-judge-requires-the-whole-corpus: a scenario only the candidate has is a scenario of
/// the corpus the saved side lacks, so the comparison is refused naming it; it is never `not
/// worse` and never `no baseline`.
#[test]
fn a_scenario_only_the_candidate_has_is_refused_naming_it() {
    let saved = trials("new-scenario-saved", &[ACCEPTED]);
    let candidate = trials("new-scenario-candidate", &[ACCEPTED]);
    record(
        &candidate,
        "scenario-2-trial-1",
        &[run("scenario-2-trial-1", "error")],
    );
    let (worse, text) = compared(&candidate, &saved, "new-scenario");
    let error = format!(
        "{:#}",
        worse.expect_err(&format!(
            "scenario 2 is the candidate's only, and the comparison gave a verdict:\n{text}"
        ))
    );
    assert!(
        error.contains("scenario 2") && error.contains("not in the saved scorecard"),
        "{error}"
    );
    assert!(
        text.contains("scenario 2  not in the saved scorecard"),
        "{text}"
    );
    assert!(!text.contains("no baseline"), "{text}");
}

/// story:the-judge-requires-the-whole-corpus: two scorecards with no scenario case between them
/// are no corpus, so their comparison is refused, never zero scenarios worse.
#[test]
fn empty_against_empty_is_refused() {
    let saved = scratch("empty-saved");
    let candidate = scratch("empty-candidate");
    let (worse, text) = compared(&candidate, &saved, "empty");
    let error = format!(
        "{:#}",
        worse.expect_err(&format!(
            "no scenario on either side, and the comparison gave a verdict:\n{text}"
        ))
    );
    assert!(error.contains("no scenario case"), "{error}");
}

/// A line holding three `at` keys: its instant is the first, its `observed_at` the last, and
/// the run's wall time spans only the lines' first instants.
#[test]
fn a_line_with_three_instants_reads_its_first() {
    let repo = scratch("three-ats");
    let case = "scenario-1-trial-1";
    record(&repo, case, &[run(case, ACCEPTED)]);
    let path = repo.join(CASES_DIR).join(case).join(EVENTS_FILE);
    let text = fs::read_to_string(&path).expect("the record");
    let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
    let first: serde_json::Value = serde_json::from_str(&lines[1]).expect("json");
    let at = first["at"].as_str().expect("an instant").to_owned();
    lines[1] = lines[1].replacen(
        "\"step\":",
        "\"at\":\"2001-01-01T00:00:00Z\",\"at\":\"2099-01-01T00:00:00Z\",\"step\":",
        1,
    );
    fs::write(&path, format!("{}\n", lines.join("\n"))).expect("the record");
    let read = Events::new(&repo.join(CASES_DIR).join(case))
        .read()
        .expect("reads");
    assert_eq!(read[1]["at"], at.as_str(), "{}", read[1]);
    assert_eq!(
        read[1]["observed_at"], "2099-01-01T00:00:00Z",
        "{}",
        read[1]
    );
    let card = Scorecard::read(&repo).expect("the scorecard reads");
    assert!(card.rows[0].wall_ms < 60_000, "{:?}", card.rows[0]);
}

/// A save is refused onto a symbolic link at the file's name, a dangling one too, and onto a
/// revision that is a path; the link's target is not written.
#[test]
fn a_save_does_not_follow_a_link_or_a_path_revision() {
    let repo = trials("save-link", &[ACCEPTED]);
    let saves = scratch("save-link-saves");
    let elsewhere = scratch("save-link-elsewhere");
    let target = elsewhere.join("planted.json");
    std::os::unix::fs::symlink(
        &target,
        saves.join("scorecard-abcdef1234567890abcdef1234567890abcdef12.json"),
    )
    .expect("a link");
    let card = Scorecard::read(&repo).expect("reads");
    let refused = card
        .clone()
        .save(
            &saves,
            "abcdef1234567890abcdef1234567890abcdef12",
            Named::Build,
        )
        .expect_err("a link at the file's name is refused");
    assert!(format!("{refused:#}").contains("exists"), "{refused:#}");
    assert!(!target.exists());
    for revision in ["../abcdef1", "ABCDEF1", "abcdef", &"a".repeat(65), ""] {
        assert!(
            card.clone().save(&saves, revision, Named::Build).is_err(),
            "{revision:?}"
        );
    }
    assert_eq!(fs::read_dir(&*elsewhere).expect("dir").count(), 0);
}

/// A small xorshift, so the property runs the same on every machine.
struct Seeded(u64);

impl Seeded {
    fn next(&mut self, below: u64) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0 % below
    }
}

const OUTCOMES: [&str; 5] = [
    "accepted",
    "ApprovalRequired",
    "Iterations",
    "NoProgress",
    "error",
];

fn row(case: &str, outcome: &str) -> Row {
    Row {
        case: case.to_owned(),
        scenario: takt::scorecard::scenario_of(case).map(str::to_owned),
        outcome: outcome.to_owned(),
        steps: 1,
        tokens: Some(100),
        refusals: 0,
        no_progress: u64::from(outcome == "NoProgress"),
        errors: u64::from(outcome == "error"),
        wall_ms: 1000,
        saturated: false,
        binary: None,
        stop: outcome.to_owned(),
        held_out: None,
        opened_binary: None,
        completed: None,
        step_tokens: Default::default(),
        layers: None,
    }
}

/// Property, seed 0x5eed: for random scenario sets (1 to 3 scenarios, 1 to 6 cases each, 1 to
/// 3 runs a case), the scorecard saved and compared with itself is never worse; lowering two
/// cases of one scenario to `error` (every run of each: a case that completed `accepted` in an
/// earlier run stays `accepted` whatever its last run ended on), nothing else changed, always
/// is, since the rule's margin is one case.
#[test]
fn self_is_never_worse_and_two_lowered_cases_always_are() {
    let empty = scratch("property-empty");
    let base = Scorecard::read(&empty).expect("an empty scorecard");
    let saves = scratch("property-saves");
    let mut rng = Seeded(0x5eed);
    let mut missed: Vec<String> = Vec::new();
    for trial in 0..200 {
        let mut card = base.clone();
        // One to six cases a scenario: compared under a rule of one trial, since the property
        // is the margin's at any n, and five trials are story:scorecard-against-needs-the-trials'.
        card.rule.trials = 1;
        for scenario in 1..=1 + rng.next(3) {
            for case in 1..=1 + rng.next(6) {
                let id = format!("scenario-{scenario}-trial-{case}");
                for _ in 0..=rng.next(3) {
                    let outcome = OUTCOMES[usize::try_from(rng.next(5)).expect("small")];
                    card.rows.push(row(&id, outcome));
                }
            }
        }
        let file = card
            .clone()
            .save(&saves, &format!("{trial:040x}"), Named::Build)
            .expect("saved");
        let saved = Scorecard::load(&file).expect("loads");
        let mut out = Vec::new();
        let worse = card.against(&saved, &mut out).expect("prints");
        assert_eq!(worse, 0, "{}", String::from_utf8_lossy(&out));

        // The last row of each case whose outcome ranks above an error, grouped by scenario.
        let lowerable: Vec<usize> = (0..card.rows.len())
            .filter(|&n| {
                let last = n + 1 == card.rows.len() || card.rows[n + 1].case != card.rows[n].case;
                last && rank(&card.rows[n].outcome) > 0
            })
            .collect();
        let scenario_of = |n: usize| card.rows[n].scenario.clone();
        let pairs: Vec<(usize, usize)> = lowerable
            .iter()
            .flat_map(|&a| {
                lowerable
                    .iter()
                    .filter(move |&&b| b > a && scenario_of(a) == scenario_of(b))
                    .map(move |&b| (a, b))
            })
            .collect();
        if pairs.is_empty() {
            continue;
        }
        let (first, second) = pairs[usize::try_from(rng.next(pairs.len() as u64)).expect("small")];
        let mut candidate = card.clone();
        let lowered = [
            candidate.rows[first].case.clone(),
            candidate.rows[second].case.clone(),
        ];
        for run in candidate
            .rows
            .iter_mut()
            .filter(|run| lowered.contains(&run.case))
        {
            *run = row(&run.case.clone(), "error");
        }
        let mut out = Vec::new();
        if candidate.against(&saved, &mut out).expect("prints") == 0 {
            missed.push(format!(
                "trial {trial}: {} and {} lowered to error\n{}",
                lowered[0],
                lowered[1],
                String::from_utf8_lossy(&out)
            ));
        }
    }
    assert!(
        missed.is_empty(),
        "{} candidates strictly worse by outcome were not worse; the first:\n{}",
        missed.len(),
        missed[0]
    );
}
