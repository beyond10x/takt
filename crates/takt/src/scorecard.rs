//! The scorecard: per case under `.engineering/cases/`, and per run of it, its outcome, steps,
//! tokens, refusals, `NoProgress` and errors, and wall time, read from the case's run record
//! (`events.jsonl`, [`crate::events`]) and from nothing else. A number a candidate could plant
//! in its tree (a `task_spend` file, an evidence record) is never read. A case without a record
//! is listed as such, never guessed. The binary a case's runs were made with is read from a
//! [`BINARY_FILE`] beside the record when the operator wrote one: its sha256 and a note, a
//! statement marked as the operator's, never a figure; a promotion checks it against the binary
//! it hashes. A file that does not read leaves its case's runs stating no binary and lists the
//! case with the reason; it never stops the scorecard. Each run also carries the revision its
//! own `opened` line names (`binary`), which a promotion checks against the revision promoted.
//!
//! A scenario run's outcome comes from the held-out check, never from the run's own record: the
//! runner writes the check's result in a [`HELD_OUT_FILE`] beside the case's record, and a
//! scenario case with runs and none, or one that does not read, refuses the scorecard naming the
//! case. A result that is no pass scores every run of the case `failed`; a pass leaves the
//! outcome the run's stop gives. The run's own stop is kept beside the outcome, and a run whose
//! stop claims a completion while the held-out check failed is named as a disagreement.
//!
//! A run is the span from one `opened` line to its `stop`, or to the next `opened` or the end
//! of the record when it has none (a run that ended on an error); a `skipped` line is no
//! instant of any run. A case is the unit of comparison: its outcome is the best a run of it
//! completed with, else its last run's (a completed case is final); its other figures are its
//! runs' summed, at most `u64::MAX` and flagged when a sum stopped there.
//! Saved, the scorecard is `scorecard-<revision>.json`, the revision of takt under test, with
//! every run's figures and the binary it states, the version of each metric's definition and
//! the rule it is compared under ([`Rule`]: the trials a scenario needs and the margin of
//! cases). Compared with a saved
//! one, a scenario is worse when, for some outcome rank, the candidate's cases at that rank or
//! better fall short of the saved share of its own n by more than the margin (at equal n: more
//! than `margin` cases fewer), or when its median of tokens exceeds the saved maximum; a side
//! with fewer cases than the rule's trials, none included, gets no verdict; a trial whose last
//! run stopped `DiskLow` and that never completed is no measurement and counts on neither side
//! ([`Case::scenario`]). Tokens are printed per
//! scenario and side as n, minimum, median and maximum over cases with the difference of the
//! medians. A metric whose definition changed between the two is printed with both figures and
//! does not make a scenario worse; outcomes are compared under the current definition whatever
//! the saved one's version. The rule applied is printed with every comparison.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result, bail};
use serde::{Deserialize, Serialize};

use crate::case::CASES_DIR;
use crate::events::{EVENTS_FILE, Events};

/// The saved scorecard's format.
pub const FORMAT: &str = "takt-scorecard/1";

/// The held-out check's result for a scenario case, in the case directory beside its record,
/// written by the runner after the run and never by the takt binary under judgement: the first
/// line the check's verdict as it printed it, `PASS` for a pass and any other text (`FAIL:
/// <reason>`, `no pull request`) for a failure, that line kept verbatim; every further line a
/// note, not read.
pub const HELD_OUT_FILE: &str = "held-out.txt";

/// The outcome of a scenario run whose held-out check did not pass, whatever its stop said.
pub const FAILED: &str = "failed";

/// The stop of a run that stopped before a step for want of free disk ([`crate::Stop::DiskLow`]):
/// a case whose last run stopped so, none of its runs completed, is no measurement of takt and
/// belongs to no scenario ([`Case::scenario`]).
pub const DISK_LOW: &str = "DiskLow";

/// The operator's statement of the binary a case's runs were made with, in the case directory
/// beside its record: the first line `<path>\t<sha256>`, the path as the operator wrote it
/// (`~` and all; only the digest is read, 64 lower-case hex digits), every further line the
/// operator's note. Written for a binary whose `--version` names no revision.
pub const BINARY_FILE: &str = "binary.txt";

/// Each metric with the version of its definition. A changed definition takes the next
/// version, so a comparison with a scorecard saved under the previous one prints both figures.
pub const METRICS: [(&str, u32); 7] = [
    ("outcome", 2),
    ("steps", 1),
    ("tokens", 1),
    ("refusals", 1),
    ("no_progress", 1),
    ("errors", 1),
    ("wall", 1),
];

/// What named a saved scorecard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Named {
    /// The revision of takt under test, as the operator gave it (`takt scorecard --revision`).
    Build,
    /// `HEAD` of a repository, as a caller of the library gave it; `takt scorecard` never
    /// names a file by it.
    Head,
}

/// One run's figures.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Row {
    /// The case the run worked.
    pub case: String,
    /// The scenario, from a case id `scenario-<id>-trial-<n>`; none for any other id.
    pub scenario: Option<String>,
    /// The outcome: for a scenario run, [`FAILED`] when its held-out check did not pass, else
    /// its `stop`; for any other run, its `stop`.
    pub outcome: String,
    /// The run's own stop: a `Completed` stop's outcome (`accepted`), else the stop's kind
    /// (`ApprovalRequired`, `NoProgress`, ...); `error` for a run that ended on an error,
    /// `unfinished` for one with neither. Empty on a file saved before it had the field.
    #[serde(default)]
    pub stop: String,
    /// The held-out check's result for the run's case ([`HELD_OUT_FILE`]), on every run of a
    /// scenario case; none on any other run, and on a file saved before it had the field.
    #[serde(default)]
    pub held_out: Option<HeldOut>,
    /// The highest step number of the run's lines: the steps it took.
    pub steps: u64,
    /// The sum of the tokens the harness reported on the run's `step` lines.
    pub tokens: u64,
    /// The run's `refusal` lines.
    pub refusals: u64,
    /// 1 when the run stopped `NoProgress`, else 0.
    pub no_progress: u64,
    /// The run's `error` lines.
    pub errors: u64,
    /// From the run's first instant to its last, in milliseconds.
    pub wall_ms: u64,
    /// A sum stopped at `u64::MAX`: the figure is at least what it shows.
    #[serde(default)]
    pub saturated: bool,
    /// The binary the run was made with, as stated beside its record ([`BINARY_FILE`]); none
    /// when nothing states it.
    #[serde(default)]
    pub binary: Option<RunBinary>,
    /// The revision the run's binary was built from, as its `opened` line names it (`binary`,
    /// what `takt --version` prints: `<sha>`, `<sha>-dirty` or `unknown revision`); none when
    /// the line names none.
    #[serde(default)]
    pub opened_binary: Option<String>,
    /// Whether the run's stop is a completion, as its stop line records it, written by the
    /// binary that stopped; none for a line from before it had the field, a run with no stop,
    /// and a file saved before it had the field, which are scored by the stops this binary
    /// knows ([`NOT_COMPLETED`]).
    #[serde(default)]
    pub completed: Option<bool>,
}

impl Row {
    /// Whether the run's own stop claims a completion while its held-out check did not pass:
    /// its stop line says it completed, or its stop is one a `Completed` stop named. Either
    /// claim is enough: a run's `completed: false` beside `Completed (accepted)` does not take
    /// the claim back.
    #[must_use]
    pub fn disagrees(&self) -> bool {
        self.held_out.as_ref().is_some_and(|held| !held.passed)
            && (self.completed == Some(true) || is_completed(&self.stop))
    }

    /// Whether the run's outcome is a completion: its stop line's own word when the outcome is
    /// its stop, else (a held-out check that did not pass, an error, no stop) by the outcomes
    /// this binary knows.
    fn completes(&self) -> bool {
        match self.completed {
            Some(completed) if self.outcome == self.stop => completed,
            _ => is_completed(&self.outcome),
        }
    }
}

/// The held-out check's result for a scenario case, as the runner wrote it beside the record
/// ([`HELD_OUT_FILE`]): whether it passed, and its first line verbatim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeldOut {
    pub passed: bool,
    pub result: String,
}

/// The held-out check's result in the case directory `dir`; none without the file. A file that
/// does not read, or whose first line is empty, is the reason, starting with [`HELD_OUT_FILE`].
fn held_out(dir: &Path) -> std::result::Result<Option<HeldOut>, String> {
    let path = dir.join(HELD_OUT_FILE);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("{HELD_OUT_FILE} does not read: {error}")),
    };
    let result = text.lines().next().unwrap_or("").trim_end_matches('\r');
    if result.trim().is_empty() {
        return Err(format!(
            "{HELD_OUT_FILE} states no result: its first line is the held-out check's verdict, `PASS` or the failure"
        ));
    }
    Ok(Some(HeldOut {
        passed: result == "PASS",
        result: result.to_owned(),
    }))
}

/// Who stated a run's binary: the operator, in a [`BINARY_FILE`] beside the run's record, for
/// a binary whose `--version` names no revision; or the run's record itself, which no record
/// does today.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StatedBy {
    Operator,
    Record,
}

/// The binary a run was made with, as stated rather than measured: its sha256, lower-case hex,
/// who stated it, and the operator's note, every line of the [`BINARY_FILE`] after the first,
/// verbatim, empty without one. `takt promote --baseline-only --binary-sha256` checks the
/// digest against the binary it hashes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunBinary {
    pub sha256: String,
    pub stated_by: StatedBy,
    pub note: String,
}

/// Whether `text` is a sha256 as takt writes one: 64 lower-case hex digits.
#[must_use]
pub fn is_sha256(text: &str) -> bool {
    text.len() == 64
        && text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// The binary a case's runs were made with, as the [`BINARY_FILE`] in its directory `dir`
/// states it; none without the file. A file that does not read, or whose first line is no
/// `<path>\t<sha256>`, is the reason, starting with [`BINARY_FILE`]: its case's runs state no
/// binary, and the case is listed with the reason, never the repository refused.
fn stated_binary(dir: &Path) -> std::result::Result<Option<RunBinary>, String> {
    let path = dir.join(BINARY_FILE);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("{BINARY_FILE} does not read: {error}")),
    };
    let (first, note) = text.split_once('\n').unwrap_or((text.as_str(), ""));
    let first = first.trim_end_matches('\r');
    let Some((_, sha256)) = first
        .rsplit_once('\t')
        .filter(|(binary, sha256)| !binary.is_empty() && is_sha256(sha256))
    else {
        return Err(format!(
            "{BINARY_FILE} states no binary: the first line is `<path>\\t<sha256>`, the sha256 64 lower-case hex digits; got {first:?}"
        ));
    };
    Ok(Some(RunBinary {
        sha256: sha256.to_owned(),
        stated_by: StatedBy::Operator,
        note: note.to_owned(),
    }))
}

/// One case's figures: its outcome, the best a run of it completed with, else its last run's;
/// its runs' other figures summed. A case that completed is final (a merged case does not
/// reopen): a run on it after it completed changes nothing it reached, so an `accepted` case
/// stays `accepted` whatever a later run ended on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Case {
    /// The case.
    pub case: String,
    /// Its scenario, as [`Row::scenario`]; none for a trial that is no measurement
    /// ([`DISK_LOW`]), so neither side of a comparison nor a promotion counts it.
    pub scenario: Option<String>,
    /// The best outcome a run of it completed with, else its last run's.
    pub outcome: String,
    /// Its runs.
    pub runs: u64,
    /// Its runs' steps, summed.
    pub steps: u64,
    /// Its runs' tokens, summed.
    pub tokens: u64,
    /// Its runs' refusals, summed.
    pub refusals: u64,
    /// 1 when its outcome is `NoProgress`, else 0.
    pub no_progress: u64,
    /// Its runs' errors, summed.
    pub errors: u64,
    /// Its runs' wall times, summed.
    pub wall_ms: u64,
    /// A sum, the case's or a run's, stopped at `u64::MAX`.
    pub saturated: bool,
}

impl Case {
    /// The case of `runs`, all of one case, in the order they ran; none for no run.
    fn of(runs: &[&Row]) -> Option<Self> {
        let last = runs.last()?;
        let outcome = runs
            .iter()
            .enumerate()
            .filter(|(_, run)| run.completes())
            .max_by_key(|(n, run)| (rank(&run.outcome), *n))
            .map_or(&last.outcome, |(_, run)| &run.outcome)
            .clone();
        let mut saturated = runs.iter().any(|run| run.saturated);
        let mut sum = |figure: fn(&Row) -> u64| {
            runs.iter().fold(0_u64, |sum, run| {
                sum.checked_add(figure(run)).unwrap_or_else(|| {
                    saturated = true;
                    u64::MAX
                })
            })
        };
        let (steps, tokens, refusals, errors, wall_ms) = (
            sum(|run| run.steps),
            sum(|run| run.tokens),
            sum(|run| run.refusals),
            sum(|run| run.errors),
            sum(|run| run.wall_ms),
        );
        // A trial whose last run stopped `DiskLow`, none of its runs completed, measured the
        // machine's disk, not takt: it is left out of its scenario, on either side, so a scenario
        // it leaves short of the rule's trials gets no verdict (the planner's ruling).
        let measured = last.stop != DISK_LOW || runs.iter().any(|run| run.completes());
        Some(Self {
            case: last.case.clone(),
            scenario: last.scenario.clone().filter(|_| measured),
            no_progress: u64::from(outcome == "NoProgress"),
            outcome,
            runs: runs.len() as u64,
            steps,
            tokens,
            refusals,
            errors,
            wall_ms,
            saturated,
        })
    }
}

/// The rule a comparison applies, recorded with the scorecard so that a saved file says what
/// it was judged under and a change to the rule is a change to a judge surface
/// (design:recursive-self-improvement § Scorecard, decided 2026-10-18).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rule {
    /// The cases a scenario needs on the candidate's side for a promotion to read it; fewer is
    /// a refusal, not a verdict.
    pub trials: u32,
    /// The cases a scenario may fall short by at any outcome rank before it is worse: one
    /// flaky case passes, two cases worse at a rank fail.
    pub margin: u32,
}

impl Default for Rule {
    fn default() -> Self {
        Self {
            trials: 5,
            margin: 1,
        }
    }
}

/// The scorecard of a repository's runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Scorecard {
    /// [`FORMAT`].
    pub format: String,
    /// The revision a saved scorecard is named by; none before it is saved.
    pub revision: Option<String>,
    /// What named it.
    pub named_by: Option<Named>,
    /// The version of each metric's definition the figures were taken under.
    pub metrics: BTreeMap<String, u32>,
    /// The rule a comparison with this scorecard applies; a file saved before the rule had
    /// fields reads with the default.
    #[serde(default)]
    pub rule: Rule,
    /// One row a run, in case order, each case's runs in the order they ran.
    pub rows: Vec<Row>,
    /// The cases that have no record; and each case with runs whose [`BINARY_FILE`] does not
    /// read, its reason in `unstated`.
    pub no_record: Vec<String>,
    /// Each case listed in `no_record` for a [`BINARY_FILE`] that does not read, with the
    /// reason; its runs count and state no binary. Empty on a file saved before it had the field.
    #[serde(default)]
    pub unstated: BTreeMap<String, String>,
}

/// The scenario of a case id `scenario-<id>-trial-<n>`, `<n>` digits; none for any other id.
#[must_use]
pub fn scenario_of(case: &str) -> Option<&str> {
    let (id, trial) = case.strip_prefix("scenario-")?.rsplit_once("-trial-")?;
    (!id.is_empty() && !trial.is_empty() && trial.bytes().all(|b| b.is_ascii_digit())).then_some(id)
}

/// An outcome's rank, higher better: `accepted`; an approval or a wait (`ApprovalRequired`,
/// `Waiting`); a budget stop (`Iterations`, `Hours`, `Tokens`) and `Unperformable`;
/// `NoProgress`, and with it every other stop (`NothingAdmissible`, `StepRefused`, another
/// outcome); an error, no stop, or a held-out check that did not pass ([`FAILED`]).
///
/// The planner's ruling: `Unperformable`, a stop that only names why the run could not go on,
/// ranks with the budget stops. A run that used to end on its budget ranks where it did, so
/// naming its stop moves no comparison against an older baseline, which below the budget stops
/// would read as worse and above them would flatter the candidate. Not every such run ranked
/// there: a `--forge none` run that used to stop `NoProgress` (rank 1) now ranks one higher
/// (`Unperformable`, 2) against an older baseline. Real promotions measure with the GitHub forge
/// and a gates document, where `Unperformable` does not fire, so that lift affects only
/// `--forge none` trial comparisons. `DiskLow` takes no rank in a comparison: a trial that ended
/// on it measured the machine's disk, not takt, and belongs to no scenario ([`Case::scenario`]).
#[must_use]
pub fn rank(outcome: &str) -> u8 {
    match outcome {
        "accepted" => 4,
        "ApprovalRequired" | "Waiting" => 3,
        "Iterations" | "Hours" | "Tokens" | "Unperformable" => 2,
        "error" | "unfinished" | FAILED => 0,
        _ => 1,
    }
}

/// The outcomes that are no completion: every stop's kind but `Completed`'s
/// ([`crate::run::STOP_KINDS`], which lists every variant), an error, no stop, a held-out check
/// that did not pass. Any other outcome is one a `Completed` stop named.
const NOT_COMPLETED: [&str; crate::run::STOP_KINDS.len() + 3] = {
    let mut all = [""; crate::run::STOP_KINDS.len() + 3];
    let mut at = 0;
    while at < crate::run::STOP_KINDS.len() {
        all[at] = crate::run::STOP_KINDS[at];
        at += 1;
    }
    all[at] = "error";
    all[at + 1] = "unfinished";
    all[at + 2] = FAILED;
    all
};

/// Whether `outcome` is one a `Completed` stop named: the case completed with it. Read for a run
/// whose stop line does not say ([`Row::completed`]): a kind not in [`NOT_COMPLETED`] counts as
/// a completion there.
fn is_completed(outcome: &str) -> bool {
    !NOT_COMPLETED.contains(&outcome)
}

/// The ranks [`rank`] gives, best first, each with the outcomes it covers.
const RANKS: [(u8, &str); 4] = [
    (4, "accepted"),
    (3, "ApprovalRequired or Waiting"),
    (2, "a budget stop or Unperformable"),
    (1, "NoProgress or another stop"),
];

/// An event line's instant as nanoseconds since the epoch: `YYYY-MM-DDTHH:MM:SS[.f...]Z`,
/// whole seconds as a run wrote them before instants had a fraction.
pub(crate) fn instant(at: &str) -> Option<i128> {
    let at = at.strip_suffix('Z')?;
    let (whole, fraction) = at.split_once('.').unwrap_or((at, ""));
    let bytes = whole.as_bytes();
    if bytes.len() != 19
        || [(4, b'-'), (7, b'-'), (10, b'T'), (13, b':'), (16, b':')]
            .iter()
            .any(|(at, byte)| bytes[*at] != *byte)
        || !fraction.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    let number = |from: usize, to: usize| whole.get(from..to)?.parse::<u32>().ok();
    let date = time::Date::from_calendar_date(
        i32::try_from(number(0, 4)?).ok()?,
        time::Month::try_from(u8::try_from(number(5, 7)?).ok()?).ok()?,
        u8::try_from(number(8, 10)?).ok()?,
    )
    .ok()?;
    let time = time::Time::from_hms(
        u8::try_from(number(11, 13)?).ok()?,
        u8::try_from(number(14, 16)?).ok()?,
        u8::try_from(number(17, 19)?).ok()?,
    )
    .ok()?;
    let nanos: String = fraction
        .chars()
        .chain("000000000".chars())
        .take(9)
        .collect();
    Some(
        time::PrimitiveDateTime::new(date, time)
            .assume_utc()
            .unix_timestamp_nanos()
            + nanos.parse::<i128>().ok()?,
    )
}

/// A run being read.
#[derive(Default)]
struct Span {
    first: Option<i128>,
    last: Option<i128>,
    steps: u64,
    tokens: u64,
    refusals: u64,
    errors: u64,
    saturated: bool,
    stop: Option<String>,
    completed: Option<bool>,
    binary: Option<String>,
}

/// A run's outcome from its stop: a `Completed` stop's outcome (`accepted`), else the stop's
/// kind (`ApprovalRequired`, `NoProgress`, ...); `error` for a run that ended on an error
/// without a stop, `unfinished` for one with neither.
pub(crate) fn outcome_of(stop: Option<&str>, errors: u64) -> String {
    match stop {
        Some(stop) => match stop.strip_prefix("Completed (") {
            Some(rest) => rest.strip_suffix(')').unwrap_or(rest).to_owned(),
            None => stop.split([' ', '(']).next().unwrap_or(stop).to_owned(),
        },
        None if errors > 0 => "error".to_owned(),
        None => "unfinished".to_owned(),
    }
}

impl Span {
    fn row(self, case: &str) -> Row {
        let outcome = outcome_of(self.stop.as_deref(), self.errors);
        let (wall_ms, too_long) = match (self.first, self.last) {
            (Some(first), Some(last)) => u64::try_from((last - first).max(0) / 1_000_000)
                .map_or((u64::MAX, true), |ms| (ms, false)),
            _ => (0, false),
        };
        Row {
            case: case.to_owned(),
            scenario: scenario_of(case).map(str::to_owned),
            no_progress: u64::from(outcome == "NoProgress"),
            stop: outcome.clone(),
            outcome,
            held_out: None,
            steps: self.steps,
            tokens: self.tokens,
            refusals: self.refusals,
            errors: self.errors,
            wall_ms,
            saturated: self.saturated || too_long,
            binary: None,
            opened_binary: self.binary,
            completed: self.stop.and(self.completed),
        }
    }
}

/// The runs of one case's record, in the order they ran.
fn runs(case: &str, lines: &[serde_json::Value]) -> Vec<Row> {
    let mut rows = Vec::new();
    let mut open: Option<Span> = None;
    for line in lines {
        if line["event"] == "opened"
            && let Some(span) = open.replace(Span::default())
        {
            rows.push(span.row(case));
        }
        let Some(span) = open.as_mut() else {
            continue;
        };
        if line["event"] == "opened" {
            span.binary = line["binary"].as_str().map(str::to_owned);
        }
        if span.stop.is_some() || line["event"] == "skipped" {
            continue;
        }
        if let Some(at) = line["at"].as_str().and_then(instant) {
            span.first = Some(span.first.map_or(at, |first| first.min(at)));
            span.last = Some(span.last.map_or(at, |last| last.max(at)));
        }
        span.steps = span.steps.max(line["step"].as_u64().unwrap_or(0));
        let count = |figure: &mut u64, by: u64, saturated: &mut bool| {
            *figure = figure.checked_add(by).unwrap_or_else(|| {
                *saturated = true;
                u64::MAX
            });
        };
        match line["event"].as_str() {
            Some("step") => count(
                &mut span.tokens,
                line["tokens"].as_u64().unwrap_or(0),
                &mut span.saturated,
            ),
            Some("refusal") => count(&mut span.refusals, 1, &mut span.saturated),
            Some("error") => count(&mut span.errors, 1, &mut span.saturated),
            Some("stop") => {
                span.stop = Some(line["stop"].as_str().unwrap_or("").to_owned());
                span.completed = line["completed"].as_bool();
            }
            _ => {}
        }
    }
    rows.extend(open.map(|span| span.row(case)));
    rows
}

/// A metric's figure of `case`, as printed.
fn figure(case: &Case, metric: &str) -> String {
    match metric {
        "outcome" => case.outcome.clone(),
        "steps" => case.steps.to_string(),
        "tokens" => case.tokens.to_string(),
        "refusals" => case.refusals.to_string(),
        "no_progress" => case.no_progress.to_string(),
        "errors" => case.errors.to_string(),
        "wall" => format!("{}.{:03}", case.wall_ms / 1000, case.wall_ms % 1000),
        _ => "-".to_owned(),
    }
}

/// A metric's figure of `case` as a number, a scenario's median is taken over: an outcome by
/// its rank, wall time in milliseconds.
fn number(case: &Case, metric: &str) -> u64 {
    match metric {
        "outcome" => u64::from(rank(&case.outcome)),
        "steps" => case.steps,
        "tokens" => case.tokens,
        "refusals" => case.refusals,
        "no_progress" => case.no_progress,
        "errors" => case.errors,
        _ => case.wall_ms,
    }
}

/// Twice the median of `values`, which are not empty: an integer either way.
fn median2(values: &[u64]) -> i128 {
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let n = sorted.len();
    if n % 2 == 1 {
        2 * i128::from(sorted[n / 2])
    } else {
        i128::from(sorted[n / 2 - 1]) + i128::from(sorted[n / 2])
    }
}

/// Half of `twice`, exactly: `12`, `12.5`, `-0.5`.
fn half(twice: i128) -> String {
    let sign = if twice < 0 { "-" } else { "" };
    let abs = twice.abs();
    format!("{sign}{}{}", abs / 2, if abs % 2 == 1 { ".5" } else { "" })
}

/// Whether `revision` can name a scorecard: the full commit id, 40 lower-case hex digits.
fn is_full_revision(revision: &str) -> bool {
    revision.len() == 40
        && revision
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// What a run whose own stop and held-out check disagree says on each side; none for a run
/// that has no disagreement.
fn disagreement(run: &Row) -> Option<String> {
    let held = run.held_out.as_ref().filter(|_| run.disagrees())?;
    Some(format!(
        "its own stop says {}, the held-out check says {}; scored {}",
        run.stop, held.result, run.outcome
    ))
}

/// What `takt --version` prints, and so a run's `opened` line names, for a binary built where
/// no git checkout held it: a binary that names no revision.
pub const UNKNOWN_REVISION: &str = "unknown revision";

/// Whether an `opened` line's `binary` names no revision: none, as a binary built before
/// `--version` named its revision left it; [`UNKNOWN_REVISION`]; or empty. The one predicate a
/// save ([`Scorecard::save`]), a stated baseline (`takt promote --binary-sha256`) and a trace's
/// revision read it with.
#[must_use]
pub fn names_no_revision(binary: Option<&str>) -> bool {
    binary.is_none_or(|binary| binary.is_empty() || binary == UNKNOWN_REVISION)
}

/// The mix rule: the first run of `rows` whose `opened` line names no revision
/// ([`names_no_revision`]) while another run's names one, as `<case>#<n>` with its row; none
/// when every run names one or none does. A save and a stated baseline refuse such a run.
#[must_use]
pub fn unnamed_in_a_mix(rows: &[Row]) -> Option<(String, &Row)> {
    let unnamed = |row: &Row| names_no_revision(row.opened_binary.as_deref());
    if rows.iter().all(unnamed) || !rows.iter().any(unnamed) {
        return None;
    }
    let mut counted: BTreeMap<&str, u64> = BTreeMap::new();
    for row in rows {
        let n = counted.entry(row.case.as_str()).or_insert(0);
        *n += 1;
        if unnamed(row) {
            return Some((format!("{}#{n}", row.case), row));
        }
    }
    None
}

impl Scorecard {
    /// The scorecard of the runs recorded under `repo`'s cases directory, cases in name order;
    /// none when the directory is absent. A record that does not read is an error naming it;
    /// each run of a case carries the binary its [`BINARY_FILE`] states, and a file that does
    /// not read leaves that case's runs stating none and lists the case in `no_record`, the
    /// reason in `unstated`, every other case read as without it. Each run of a scenario case
    /// carries the held-out result its [`HELD_OUT_FILE`] states, and takes its outcome from it;
    /// a scenario case with runs and no such file, or one that does not read, is an error
    /// naming the case.
    pub fn read(repo: &Path) -> Result<Self> {
        let cases = repo.join(CASES_DIR);
        let mut names: Vec<String> = Vec::new();
        match std::fs::read_dir(&cases) {
            Ok(entries) => {
                for entry in entries {
                    let entry = entry.with_context(|| format!("read {}", cases.display()))?;
                    if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                        names.push(entry.file_name().to_string_lossy().into_owned());
                    }
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(error).with_context(|| format!("read {}", cases.display()));
            }
        }
        names.sort();
        let mut card = Self {
            format: FORMAT.to_owned(),
            revision: None,
            named_by: None,
            metrics: METRICS
                .iter()
                .map(|(metric, version)| ((*metric).to_owned(), *version))
                .collect(),
            rule: Rule::default(),
            rows: Vec::new(),
            no_record: Vec::new(),
            unstated: BTreeMap::new(),
        };
        for name in names {
            let dir = cases.join(&name);
            let mut rows = if dir.join(EVENTS_FILE).is_file() {
                runs(&name, &Events::new(&dir).read()?)
            } else {
                Vec::new()
            };
            let (binary, unstated) = match stated_binary(&dir) {
                Ok(binary) => (binary, None),
                Err(reason) => (None, Some(reason)),
            };
            for row in &mut rows {
                row.binary = binary.clone();
            }
            if !rows.is_empty() && scenario_of(&name).is_some() {
                let held = match held_out(&dir) {
                    Ok(Some(held)) => held,
                    Ok(None) => bail!(
                        "{name}: a scenario case with no held-out result ({} is missing); its runs' outcome comes from the held-out check the runner writes beside the record, never from the runs' own stop, so the scorecard is not read",
                        dir.join(HELD_OUT_FILE).display()
                    ),
                    Err(reason) => bail!(
                        "{name}: {reason} ({}); its runs' outcome comes from the held-out check, so the scorecard is not read",
                        dir.join(HELD_OUT_FILE).display()
                    ),
                };
                for row in &mut rows {
                    if !held.passed {
                        FAILED.clone_into(&mut row.outcome);
                    }
                    row.held_out = Some(held.clone());
                }
            }
            if rows.is_empty() {
                card.no_record.push(name);
            } else {
                if let Some(reason) = unstated {
                    card.no_record.push(name.clone());
                    card.unstated.insert(name, reason);
                }
                card.rows.extend(rows);
            }
        }
        Ok(card)
    }

    /// One [`Case`] a case with a record, in case order.
    #[must_use]
    pub fn cases(&self) -> Vec<Case> {
        let mut cases: Vec<Case> = Vec::new();
        let mut runs: Vec<&Row> = Vec::new();
        for row in &self.rows {
            if runs.last().is_some_and(|last| last.case != row.case) {
                cases.extend(Case::of(&runs));
                runs.clear();
            }
            runs.push(row);
        }
        cases.extend(Case::of(&runs));
        cases
    }

    /// Prints the definitions' versions, then a row a case, its runs listed under it as
    /// `<case>#<n>`, each with its own stop and the held-out result beside the outcome, then
    /// each run whose stop and held-out check disagree, each case with no record, and each case
    /// whose [`BINARY_FILE`] does not read with the reason.
    pub fn print(&self, out: &mut dyn Write) -> std::io::Result<()> {
        let definitions: Vec<String> = self
            .metrics
            .iter()
            .map(|(metric, version)| format!("{metric} v{version}"))
            .collect();
        let cases = self.cases();
        writeln!(
            out,
            "{} cases, {} runs under {CASES_DIR}; definitions: {}",
            cases.len(),
            self.rows.len(),
            definitions.join(", ")
        )?;
        let mut table: Vec<Vec<String>> = vec![
            [
                "case",
                "scenario",
                "outcome",
                "stop",
                "held_out",
                "steps",
                "tokens",
                "refusals",
                "no_progress",
                "errors",
                "wall_s",
                "notes",
            ]
            .map(str::to_owned)
            .to_vec(),
        ];
        let line = |label: String, case: &Case, run: Option<&Row>| {
            let mut cells = vec![
                label,
                case.scenario.clone().unwrap_or_else(|| "-".to_owned()),
                figure(case, "outcome"),
                run.map_or_else(|| "-".to_owned(), |run| run.stop.clone()),
                match run.and_then(|run| run.held_out.as_ref()) {
                    Some(held) if held.passed => "pass".to_owned(),
                    Some(_) => "fail".to_owned(),
                    None => "-".to_owned(),
                },
            ];
            for metric in [
                "steps",
                "tokens",
                "refusals",
                "no_progress",
                "errors",
                "wall",
            ] {
                cells.push(figure(case, metric));
            }
            cells.push(if case.saturated { "saturated" } else { "-" }.to_owned());
            cells
        };
        let mut disagreements = Vec::new();
        for case in &cases {
            table.push(line(case.case.clone(), case, None));
            let runs = self.rows.iter().filter(|row| row.case == case.case);
            for (n, run) in runs.enumerate() {
                let label = format!("{}#{}", case.case, n + 1);
                if let Some(single) = Case::of(&[run]) {
                    table.push(line(format!("  {label}"), &single, Some(run)));
                }
                if let Some(text) = disagreement(run) {
                    disagreements.push(format!("disagreement  {label}  {text}"));
                }
            }
        }
        let widths: Vec<usize> = (0..table[0].len())
            .map(|column| table.iter().map(|row| row[column].len()).max().unwrap_or(0))
            .collect();
        for row in &table {
            let cells: Vec<String> = row
                .iter()
                .zip(&widths)
                .map(|(cell, width)| format!("{cell:<width$}"))
                .collect();
            writeln!(out, "{}", cells.join("  ").trim_end())?;
        }
        for line in disagreements {
            writeln!(out, "{line}")?;
        }
        for case in &self.no_record {
            match self.unstated.get(case) {
                Some(reason) => writeln!(out, "{case}  {reason}; its runs state no binary")?,
                None => writeln!(out, "{case}  no record")?,
            }
        }
        Ok(())
    }

    /// Saves the scorecard as `<dir>/scorecard-<revision>.json`, named by `revision`, the full
    /// commit id, checked before anything else, which `named` says the source of; the path. A
    /// file there already is not overwritten: the save is refused. So is a scorecard holding a
    /// run of another binary than `revision`'s ([`Scorecard::made_with`]): nothing is written.
    pub fn save(mut self, dir: &Path, revision: &str, named: Named) -> Result<PathBuf> {
        if !is_full_revision(revision) {
            bail!(
                "the revision {revision:?} cannot name a scorecard file: give the full commit id, 40 lower-case hex digits"
            );
        }
        self.made_with(revision)?;
        self.revision = Some(revision.to_owned());
        self.named_by = Some(named);
        std::fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
        let path = dir.join(format!("scorecard-{revision}.json"));
        let mut text = serde_json::to_string_pretty(&self).context("write the scorecard")?;
        text.push('\n');
        let mut file = match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                bail!(
                    "{} exists: a saved scorecard is not overwritten",
                    path.display()
                );
            }
            Err(error) => {
                return Err(error).with_context(|| format!("write {}", path.display()));
            }
        };
        file.write_all(text.as_bytes())
            .with_context(|| format!("write {}", path.display()))?;
        Ok(path)
    }

    /// Every run was made with the binary of `revision`, as its `opened` line names it,
    /// compared exactly (`<sha>-dirty` is another binary); or no run's line names a revision
    /// ([`names_no_revision`]). A run that names none beside one that names one
    /// ([`unnamed_in_a_mix`]) is the refusal, else the first run that names another revision,
    /// named `<case>#<n>` with both revisions.
    fn made_with(&self, revision: &str) -> Result<()> {
        if let Some((run, row)) = unnamed_in_a_mix(&self.rows) {
            bail!(
                "run {run} names no revision ({}), beside runs that name one; saved under {revision}, a scorecard's runs all name {revision} or all name none, so nothing is saved",
                row.opened_binary.as_deref().map_or_else(
                    || "its `opened` line has no `binary`".to_owned(),
                    |binary| format!("its `opened` line says {binary:?}")
                )
            );
        }
        let mut counted: BTreeMap<&str, u64> = BTreeMap::new();
        for row in &self.rows {
            let n = counted.entry(row.case.as_str()).or_insert(0);
            *n += 1;
            if let Some(named) = row.opened_binary.as_deref()
                && !names_no_revision(Some(named))
                && named != revision
            {
                bail!(
                    "run {}#{n} was made with the binary of {named}, not with the binary of {revision}; a scorecard is saved under the full revision every run's `opened` line names, so nothing is saved",
                    row.case
                );
            }
        }
        Ok(())
    }

    /// A saved scorecard; one that does not read, or of another format, is an error naming it,
    /// and so is one with a run whose scenario is not the one its case id names
    /// ([`scenario_of`]), or whose outcome is not what its held-out result makes it ([`FAILED`]
    /// unless it passed, else its stop).
    pub fn load(path: &Path) -> Result<Self> {
        let text =
            std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
        let card: Self = serde_json::from_str(&text)
            .with_context(|| format!("{} is not a saved scorecard", path.display()))?;
        if card.format != FORMAT {
            bail!(
                "{} is a scorecard of format {}, not {FORMAT}",
                path.display(),
                card.format
            );
        }
        let mut counted: BTreeMap<&str, u64> = BTreeMap::new();
        for row in &card.rows {
            let n = counted.entry(row.case.as_str()).or_insert(0);
            *n += 1;
            if row.scenario.as_deref() != scenario_of(&row.case) {
                bail!(
                    "{}: run {}#{n} gives the scenario {}, but its case id names {}; a scenario is read from the case id, so the scorecard is not read",
                    path.display(),
                    row.case,
                    row.scenario.as_deref().unwrap_or("none"),
                    scenario_of(&row.case).unwrap_or("none")
                );
            }
            let Some(held) = &row.held_out else {
                continue;
            };
            if held.passed != (held.result == "PASS") {
                bail!(
                    "{}: run {}#{n} has the held-out result {:?}, which {}, but the file says passed {}; whether a run passed is read from its result, so the scorecard is not read",
                    path.display(),
                    row.case,
                    held.result,
                    if held.result == "PASS" {
                        "passed"
                    } else {
                        "did not pass"
                    },
                    held.passed
                );
            }
            let expected = if held.passed {
                row.stop.as_str()
            } else {
                FAILED
            };
            if row.outcome != expected {
                bail!(
                    "{}: run {}#{n} gives the outcome {}, but its held-out result {:?} makes it {expected:?}; a saved scorecard whose outcomes disagree with their held-out results is not read",
                    path.display(),
                    row.case,
                    row.outcome,
                    held.result
                );
            }
        }
        Ok(card)
    }

    /// Compares this scorecard with `saved`, scenario by scenario, over cases, under this
    /// scorecard's [`Rule`], printed first: each case and its outcome; each side's n and the
    /// minimum, median and maximum of tokens, and the difference of the medians; both figures
    /// of every metric whose definition's version differs; each outcome rank at which the
    /// candidate falls short, with both counts; then the verdict. A scenario is worse when, for
    /// some rank, the candidate's cases at that rank or better fall short of the saved share of
    /// the candidate's n by more than the margin (at equal n: by more than `margin` cases),
    /// under the current definition; when its median of tokens exceeds the saved maximum, the
    /// token definition unchanged. A scorecard is never worse than itself; one case
    /// lowered is within a margin of one, two cases lowered in one scenario never are. The
    /// trials apply as a promotion applies them: a scenario the saved scorecard has with fewer
    /// cases than the rule's trials (`too few cases on the saved side (<n> of <trials>)`), and
    /// any scenario with fewer on the candidate's side, none included (`too few cases (<n> of
    /// <trials>)`), gets no verdict, and once every scenario is printed the comparison is an
    /// error naming each such scenario, as a promotion refuses one. A case a run of which disagrees with its held-out check is named so beside its
    /// outcome. The number of scenarios marked worse.
    pub fn against(&self, saved: &Scorecard, out: &mut dyn Write) -> Result<usize> {
        let version = |card: &Scorecard, metric: &str| {
            card.metrics
                .get(metric)
                .map_or("v-".to_owned(), |version| format!("v{version}"))
        };
        let changed = |metric: &str| saved.metrics.get(metric) != self.metrics.get(metric);
        writeln!(
            out,
            "against {} ({})",
            saved.revision.as_deref().unwrap_or("-"),
            match saved.named_by {
                Some(Named::Build) => "the revision of takt under test",
                Some(Named::Head) => "HEAD of a repository",
                None => "unnamed",
            }
        )?;
        let Rule { trials, margin } = self.rule;
        writeln!(
            out,
            "rule  trials {trials} a scenario  margin {margin} case{}  tokens worse when the median exceeds the saved maximum",
            if margin == 1 { "" } else { "s" }
        )?;
        let (candidates, baselines) = (self.cases(), saved.cases());
        let scenarios: BTreeSet<&str> = candidates
            .iter()
            .chain(&baselines)
            .filter_map(|case| case.scenario.as_deref())
            .collect();
        let mut worse = 0;
        let mut too_few: Vec<String> = Vec::new();
        for scenario in scenarios {
            let of = |cases: &[Case]| -> Vec<Case> {
                cases
                    .iter()
                    .filter(|case| case.scenario.as_deref() == Some(scenario))
                    .cloned()
                    .collect()
            };
            let (candidate, baseline) = (of(&candidates), of(&baselines));
            for case in &candidate {
                let disagrees = self
                    .rows
                    .iter()
                    .filter(|row| row.case == case.case)
                    .find_map(disagreement);
                match disagrees {
                    Some(text) => writeln!(
                        out,
                        "{}  scenario {scenario}  {}  disagreement: {text}",
                        case.case, case.outcome
                    )?,
                    None => writeln!(out, "{}  scenario {scenario}  {}", case.case, case.outcome)?,
                }
            }
            // A rule of no trials still needs one case a side: there is no verdict on none.
            let needed = usize::try_from(trials).unwrap_or(usize::MAX).max(1);
            if !baseline.is_empty() && baseline.len() < needed {
                writeln!(
                    out,
                    "scenario {scenario}  too few cases on the saved side ({} of {trials})",
                    baseline.len()
                )?;
                too_few.push(format!(
                    "scenario {scenario} (the saved side, {} of {trials})",
                    baseline.len()
                ));
                continue;
            }
            if candidate.len() < needed {
                writeln!(
                    out,
                    "scenario {scenario}  too few cases ({} of {trials})",
                    candidate.len()
                )?;
                too_few.push(format!(
                    "scenario {scenario} ({} of {trials})",
                    candidate.len()
                ));
                continue;
            }
            if baseline.is_empty() {
                writeln!(out, "scenario {scenario}  no baseline")?;
                continue;
            }
            let median = |cases: &[Case], metric: &str| {
                median2(
                    &cases
                        .iter()
                        .map(|case| number(case, metric))
                        .collect::<Vec<_>>(),
                )
            };
            for (side, cases) in [("saved", &baseline), ("candidate", &candidate)] {
                let tokens: Vec<u64> = cases.iter().map(|case| case.tokens).collect();
                let min = tokens.iter().min().copied().unwrap_or(0);
                let max = tokens.iter().max().copied().unwrap_or(0);
                writeln!(
                    out,
                    "scenario {scenario}  {side:<9}  n {}  tokens min {min} median {} max {max}",
                    tokens.len(),
                    half(median2(&tokens))
                )?;
            }
            let (saved_tokens, tokens) =
                (median(&baseline, "tokens"), median(&candidate, "tokens"));
            let mut over_tokens = false;
            if changed("tokens") {
                writeln!(
                    out,
                    "scenario {scenario}  median tokens {} {} {} {}",
                    version(saved, "tokens"),
                    half(saved_tokens),
                    version(self, "tokens"),
                    half(tokens)
                )?;
            } else {
                let difference = tokens - saved_tokens;
                writeln!(
                    out,
                    "scenario {scenario}  median tokens {}{}",
                    if difference >= 0 { "+" } else { "" },
                    half(difference)
                )?;
                let saved_max = baseline.iter().map(|case| case.tokens).max().unwrap_or(0);
                // The median is held doubled; so is the maximum for the comparison.
                if tokens > 2 * i128::from(saved_max) {
                    over_tokens = true;
                    writeln!(
                        out,
                        "scenario {scenario}  median tokens {} exceeds the saved maximum {saved_max}",
                        half(tokens)
                    )?;
                }
            }
            for (metric, _) in METRICS {
                if metric != "tokens" && changed(metric) {
                    writeln!(
                        out,
                        "scenario {scenario}  median {metric} {} {} {} {}",
                        version(saved, metric),
                        half(median(&baseline, metric)),
                        version(self, metric),
                        half(median(&candidate, metric))
                    )?;
                }
            }
            let at_least = |cases: &[Case], at: u8| {
                cases
                    .iter()
                    .filter(|case| rank(&case.outcome) >= at)
                    .count() as u64
            };
            let (n, saved_n) = (candidate.len() as u64, baseline.len() as u64);
            let mut short = false;
            for (at, covers) in RANKS {
                let (count, saved_count) = (at_least(&candidate, at), at_least(&baseline, at));
                // Short by more than the margin, the saved share scaled to the candidate's n
                // and cross-multiplied so nothing is rounded: saved_count/saved_n * n exceeds
                // count + margin.
                if u128::from(saved_count) * u128::from(n)
                    > (u128::from(count) + u128::from(margin)) * u128::from(saved_n)
                {
                    short = true;
                    writeln!(
                        out,
                        "scenario {scenario}  rank {at} or better ({covers}): candidate {count}/{n} against saved {saved_count}/{saved_n}, short by more than {margin}"
                    )?;
                }
            }
            let verdict = if short || over_tokens {
                worse += 1;
                "worse"
            } else {
                "not worse"
            };
            writeln!(
                out,
                "scenario {scenario}  outcomes by rank {}  tokens {}  {verdict}",
                if short { "short" } else { "held" },
                if over_tokens { "over" } else { "held" }
            )?;
        }
        for case in candidates.iter().filter(|case| case.scenario.is_none()) {
            writeln!(out, "{}  no scenario  not compared", case.case)?;
        }
        if !too_few.is_empty() {
            bail!(
                "too few cases for a verdict: {}; the rule needs {trials} cases a scenario on each side, so run the short side again before comparing",
                too_few.join(", ")
            );
        }
        Ok(worse)
    }
}

/// `takt scorecard`: prints the scorecard of `repo`'s runs; with `save`, saves it there, named
/// by `revision`, the revision of takt under test, which a save needs; with `against`,
/// compares it with that saved scorecard. The number of scenarios marked worse.
pub fn report(
    repo: &Path,
    save: Option<&Path>,
    revision: Option<&str>,
    against: Option<&Path>,
    out: &mut dyn Write,
) -> Result<usize> {
    let saved = against.map(Scorecard::load).transpose()?;
    let card = Scorecard::read(repo)?;
    let named = match (save, revision) {
        (Some(_), None) => bail!(
            "--save needs --revision <sha>, the revision of takt under test; a scorecard is never named by the scenario repository's HEAD"
        ),
        (Some(dir), Some(revision)) => Some(card.clone().save(dir, revision, Named::Build)?),
        (None, _) => None,
    };
    card.print(out)?;
    if let Some(path) = named {
        writeln!(
            out,
            "saved: {}, named by the revision of takt under test",
            path.display()
        )?;
    }
    match saved {
        Some(saved) => card.against(&saved, out),
        None => Ok(0),
    }
}
