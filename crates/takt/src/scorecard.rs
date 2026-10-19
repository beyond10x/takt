//! The scorecard: per case under `.engineering/cases/`, and per run of it, its outcome, steps,
//! tokens, refusals, `NoProgress` and errors, and wall time, read from the case's run record
//! (`events.jsonl`, [`crate::events`]) and from nothing else. A number a candidate could plant
//! in its tree (a `task_spend` file, an evidence record) is never read. A case without a record
//! is listed as such, never guessed.
//!
//! A run is the span from one `opened` line to its `stop`, or to the next `opened` or the end
//! of the record when it has none (a run that ended on an error); a `skipped` line is no
//! instant of any run. A case is the unit of comparison: its outcome is the best a run of it
//! completed with, else its last run's (a completed case is final); its other figures are its
//! runs' summed, at most `u64::MAX` and flagged when a sum stopped there.
//! Saved, the scorecard is `scorecard-<revision>.json`, the revision of takt under test, with
//! every run's figures, the version of each metric's definition and the rule it is compared
//! under ([`Rule`]: the trials a scenario needs and the margin of cases). Compared with a saved
//! one, a scenario is worse when, for some outcome rank, the candidate's cases at that rank or
//! better fall short of the saved share of its own n by more than the margin (at equal n: more
//! than `margin` cases fewer), when its median of tokens exceeds the saved maximum, or when the
//! saved scorecard has the scenario and the candidate has no run of it. Tokens are printed per
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

/// Each metric with the version of its definition. A changed definition takes the next
/// version, so a comparison with a scorecard saved under the previous one prints both figures.
pub const METRICS: [(&str, u32); 7] = [
    ("outcome", 1),
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
    /// The stop: a `Completed` stop's outcome (`accepted`), else the stop's kind
    /// (`ApprovalRequired`, `NoProgress`, ...); `error` for a run that ended on an error,
    /// `unfinished` for one with neither.
    pub outcome: String,
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
}

/// One case's figures: its outcome, the best a run of it completed with, else its last run's;
/// its runs' other figures summed. A case that completed is final (a merged case does not
/// reopen): a run on it after it completed changes nothing it reached, so an `accepted` case
/// stays `accepted` whatever a later run ended on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Case {
    /// The case.
    pub case: String,
    /// Its scenario, as [`Row::scenario`].
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
            .filter(|(_, run)| is_completed(&run.outcome))
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
        Some(Self {
            case: last.case.clone(),
            scenario: last.scenario.clone(),
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
    /// The cases that have no record.
    pub no_record: Vec<String>,
}

/// The scenario of a case id `scenario-<id>-trial-<n>`, `<n>` digits; none for any other id.
#[must_use]
pub fn scenario_of(case: &str) -> Option<&str> {
    let (id, trial) = case.strip_prefix("scenario-")?.rsplit_once("-trial-")?;
    (!id.is_empty() && !trial.is_empty() && trial.bytes().all(|b| b.is_ascii_digit())).then_some(id)
}

/// An outcome's rank, higher better: `accepted`; an approval or a wait (`ApprovalRequired`,
/// `Waiting`); a budget stop (`Iterations`, `Hours`, `Tokens`); `NoProgress`, and with it every
/// other stop (`NothingAdmissible`, `StepRefused`, another outcome); an error, or no stop.
#[must_use]
pub fn rank(outcome: &str) -> u8 {
    match outcome {
        "accepted" => 4,
        "ApprovalRequired" | "Waiting" => 3,
        "Iterations" | "Hours" | "Tokens" => 2,
        "error" | "unfinished" => 0,
        _ => 1,
    }
}

/// The outcomes that are no completion: a stop's kind, an error, no stop. Any other outcome
/// is one a `Completed` stop named.
const NOT_COMPLETED: [&str; 10] = [
    "ApprovalRequired",
    "Waiting",
    "Iterations",
    "Hours",
    "Tokens",
    "NothingAdmissible",
    "StepRefused",
    "NoProgress",
    "error",
    "unfinished",
];

/// Whether `outcome` is one a `Completed` stop named: the case completed with it.
fn is_completed(outcome: &str) -> bool {
    !NOT_COMPLETED.contains(&outcome)
}

/// The ranks [`rank`] gives, best first, each with the outcomes it covers.
const RANKS: [(u8, &str); 4] = [
    (4, "accepted"),
    (3, "ApprovalRequired or Waiting"),
    (2, "a budget stop"),
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
            outcome,
            steps: self.steps,
            tokens: self.tokens,
            refusals: self.refusals,
            errors: self.errors,
            wall_ms,
            saturated: self.saturated || too_long,
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
            Some("stop") => span.stop = Some(line["stop"].as_str().unwrap_or("").to_owned()),
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

/// Whether `revision` can name a scorecard: a commit id, 7 to 64 lower-case hex digits.
fn is_revision(revision: &str) -> bool {
    (7..=64).contains(&revision.len())
        && revision
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

impl Scorecard {
    /// The scorecard of the runs recorded under `repo`'s cases directory, cases in name order;
    /// none when the directory is absent. A record that does not read is an error naming it.
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
        };
        for name in names {
            let dir = cases.join(&name);
            let rows = if dir.join(EVENTS_FILE).is_file() {
                runs(&name, &Events::new(&dir).read()?)
            } else {
                Vec::new()
            };
            if rows.is_empty() {
                card.no_record.push(name);
            } else {
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
    /// `<case>#<n>`, then each case with no record.
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
        let line = |label: String, case: &Case| {
            let mut cells = vec![
                label,
                case.scenario.clone().unwrap_or_else(|| "-".to_owned()),
            ];
            for metric in [
                "outcome",
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
        for case in &cases {
            table.push(line(case.case.clone(), case));
            let runs = self.rows.iter().filter(|row| row.case == case.case);
            for (n, run) in runs.enumerate() {
                if let Some(single) = Case::of(&[run]) {
                    table.push(line(format!("  {}#{}", case.case, n + 1), &single));
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
        for case in &self.no_record {
            writeln!(out, "{case}  no record")?;
        }
        Ok(())
    }

    /// Saves the scorecard as `<dir>/scorecard-<revision>.json`, named by `revision`, a commit
    /// id, which `named` says the source of; the path. A file there already is not
    /// overwritten: the save is refused.
    pub fn save(mut self, dir: &Path, revision: &str, named: Named) -> Result<PathBuf> {
        if !is_revision(revision) {
            bail!(
                "the revision {revision:?} cannot name a scorecard file: give a commit id, 7 to 64 lower-case hex digits"
            );
        }
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

    /// A saved scorecard; one that does not read, or of another format, is an error naming it.
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
    /// token definition unchanged; or when the saved scorecard has the scenario and the
    /// candidate has no run of it (`no run`). A scorecard is never worse than itself; one case
    /// lowered is within a margin of one, two cases lowered in one scenario never are. The
    /// number of scenarios marked worse.
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
                writeln!(out, "{}  scenario {scenario}  {}", case.case, case.outcome)?;
            }
            if baseline.is_empty() {
                writeln!(out, "scenario {scenario}  no baseline")?;
                continue;
            }
            if candidate.is_empty() {
                worse += 1;
                writeln!(out, "scenario {scenario}  no run  worse")?;
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
