//! The scorecard: per run of a case under `.engineering/cases/`, its outcome, steps, tokens,
//! refusals, `NoProgress` and errors, and wall time, read from the case's run record
//! (`events.jsonl`, [`crate::events`]) and from nothing else. A number a candidate could plant
//! in its tree (a `task_spend` file, an evidence record) is never read. A case without a record
//! is listed as such, never guessed.
//!
//! A run is the span from one `opened` line to its `stop`, or to the next `opened` or the end
//! of the record when it has none (a run that ended on an error). Saved, the scorecard is
//! `scorecard-<revision>.json` with every run's figures and the version of each metric's
//! definition; compared with a saved one, a run is worse only when its outcome ranks below the
//! best outcome the saved scorecard holds for the same scenario. Tokens are printed per
//! scenario and side as n, minimum, median and maximum with the difference of the medians, and
//! never make a run worse. A metric whose definition changed between the two is printed with
//! both figures, each with its version, and not compared.

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

/// The revision this takt was built from, when the build was given one (`TAKT_REVISION`).
pub const BUILT_FROM: Option<&str> = option_env!("TAKT_REVISION");

/// What named a saved scorecard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Named {
    /// The revision takt was built from.
    Build,
    /// `HEAD` of the repository the runs were read from; the binary knew no revision.
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

/// An event line's instant as nanoseconds since the epoch: `YYYY-MM-DDTHH:MM:SS[.f...]Z`,
/// whole seconds as a run wrote them before instants had a fraction.
fn instant(at: &str) -> Option<i128> {
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
    stop: Option<String>,
}

impl Span {
    fn row(self, case: &str) -> Row {
        let outcome = match &self.stop {
            Some(stop) => match stop.strip_prefix("Completed (") {
                Some(rest) => rest.strip_suffix(')').unwrap_or(rest).to_owned(),
                None => stop.split([' ', '(']).next().unwrap_or(stop).to_owned(),
            },
            None if self.errors > 0 => "error".to_owned(),
            None => "unfinished".to_owned(),
        };
        let wall = match (self.first, self.last) {
            (Some(first), Some(last)) => u64::try_from((last - first).max(0) / 1_000_000).ok(),
            _ => None,
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
            wall_ms: wall.unwrap_or(0),
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
        if span.stop.is_some() {
            continue;
        }
        if let Some(at) = line["at"].as_str().and_then(instant) {
            span.first = Some(span.first.map_or(at, |first| first.min(at)));
            span.last = Some(span.last.map_or(at, |last| last.max(at)));
        }
        span.steps = span.steps.max(line["step"].as_u64().unwrap_or(0));
        match line["event"].as_str() {
            Some("step") => span.tokens += line["tokens"].as_u64().unwrap_or(0),
            Some("refusal") => span.refusals += 1,
            Some("error") => span.errors += 1,
            Some("stop") => span.stop = Some(line["stop"].as_str().unwrap_or("").to_owned()),
            _ => {}
        }
    }
    rows.extend(open.map(|span| span.row(case)));
    rows
}

/// A metric's figure of `row`, as printed.
fn figure(row: &Row, metric: &str) -> String {
    match metric {
        "outcome" => row.outcome.clone(),
        "steps" => row.steps.to_string(),
        "tokens" => row.tokens.to_string(),
        "refusals" => row.refusals.to_string(),
        "no_progress" => row.no_progress.to_string(),
        "errors" => row.errors.to_string(),
        "wall" => format!("{}.{:03}", row.wall_ms / 1000, row.wall_ms % 1000),
        _ => "-".to_owned(),
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

    /// Prints the definitions' versions, then a row a run, then each case with no record.
    pub fn print(&self, out: &mut dyn Write) -> std::io::Result<()> {
        let definitions: Vec<String> = self
            .metrics
            .iter()
            .map(|(metric, version)| format!("{metric} v{version}"))
            .collect();
        writeln!(
            out,
            "{} runs under {CASES_DIR}; definitions: {}",
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
            ]
            .map(str::to_owned)
            .to_vec(),
        ];
        for row in &self.rows {
            table.push(vec![
                row.case.clone(),
                row.scenario.clone().unwrap_or_else(|| "-".to_owned()),
                figure(row, "outcome"),
                figure(row, "steps"),
                figure(row, "tokens"),
                figure(row, "refusals"),
                figure(row, "no_progress"),
                figure(row, "errors"),
                figure(row, "wall"),
            ]);
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

    /// Saves the scorecard as `<dir>/scorecard-<revision>.json`, named by `revision`, which
    /// `named` says the source of; the path.
    pub fn save(mut self, dir: &Path, revision: &str, named: Named) -> Result<PathBuf> {
        if revision.is_empty() || !revision.bytes().all(|b| b.is_ascii_alphanumeric()) {
            bail!("the revision {revision:?} cannot name a scorecard file");
        }
        self.revision = Some(revision.to_owned());
        self.named_by = Some(named);
        std::fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
        let path = dir.join(format!("scorecard-{revision}.json"));
        let mut text = serde_json::to_string_pretty(&self).context("write the scorecard")?;
        text.push('\n');
        std::fs::write(&path, text).with_context(|| format!("write {}", path.display()))?;
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

    /// Compares each run with the same scenario in `saved`, scenario by scenario: a run is
    /// worse when its outcome ranks below the best saved run's; then each side's n and the
    /// minimum, median and maximum of tokens, and the difference of the medians. A metric whose
    /// definition's version differs is printed with both figures, each with its version, and
    /// not compared. The number of runs marked worse.
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
                Some(Named::Build) => "the revision takt was built from",
                Some(Named::Head) => "HEAD of the repository",
                None => "unnamed",
            }
        )?;
        let scenarios: BTreeSet<&str> = self
            .rows
            .iter()
            .chain(&saved.rows)
            .filter_map(|row| row.scenario.as_deref())
            .collect();
        let mut worse = 0;
        for scenario in scenarios {
            let of = |card: &'_ Scorecard| -> Vec<Row> {
                card.rows
                    .iter()
                    .filter(|row| row.scenario.as_deref() == Some(scenario))
                    .cloned()
                    .collect()
            };
            let (candidate, baseline) = (of(self), of(saved));
            let best = baseline
                .iter()
                .max_by_key(|row| (rank(&row.outcome), std::cmp::Reverse(row.tokens)));
            for row in &candidate {
                let mut parts = vec![row.case.clone(), format!("scenario {scenario}")];
                let Some(best) = best else {
                    parts.push("no baseline".to_owned());
                    writeln!(out, "{}", parts.join("  "))?;
                    continue;
                };
                for (metric, _) in METRICS {
                    if metric != "tokens" && changed(metric) {
                        parts.push(format!(
                            "{metric} {} {} {} {}",
                            version(saved, metric),
                            figure(best, metric),
                            version(self, metric),
                            figure(row, metric)
                        ));
                    }
                }
                if changed("outcome") {
                    parts.push("not compared".to_owned());
                } else {
                    parts.insert(2, format!("{} against {}", row.outcome, best.outcome));
                    if rank(&row.outcome) < rank(&best.outcome) {
                        worse += 1;
                        parts.push("worse".to_owned());
                    } else {
                        parts.push("not worse".to_owned());
                    }
                }
                writeln!(out, "{}", parts.join("  "))?;
            }
            let mut medians = Vec::new();
            for (side, rows) in [("saved", &baseline), ("candidate", &candidate)] {
                let tokens: Vec<u64> = rows.iter().map(|row| row.tokens).collect();
                let (Some(min), Some(max)) = (tokens.iter().min(), tokens.iter().max()) else {
                    continue;
                };
                let median = median2(&tokens);
                medians.push(median);
                writeln!(
                    out,
                    "scenario {scenario}  {side:<9}  n {}  tokens min {min} median {} max {max}",
                    tokens.len(),
                    half(median)
                )?;
            }
            if let [saved_median, median] = medians[..] {
                if changed("tokens") {
                    writeln!(
                        out,
                        "scenario {scenario}  median tokens {} {} {} {}",
                        version(saved, "tokens"),
                        half(saved_median),
                        version(self, "tokens"),
                        half(median)
                    )?;
                } else {
                    let difference = median - saved_median;
                    writeln!(
                        out,
                        "scenario {scenario}  median tokens {}{}",
                        if difference >= 0 { "+" } else { "" },
                        half(difference)
                    )?;
                }
            }
        }
        for row in self.rows.iter().filter(|row| row.scenario.is_none()) {
            writeln!(out, "{}  no scenario  not compared", row.case)?;
        }
        Ok(worse)
    }
}

/// `takt scorecard`: prints the scorecard of `repo`'s runs; with `save`, saves it there, named
/// by [`BUILT_FROM`] when the build knows it, else by `HEAD` of `repo`, and says which; with
/// `against`, compares it with that saved scorecard. The number of runs marked worse.
pub fn report(
    repo: &Path,
    save: Option<&Path>,
    against: Option<&Path>,
    out: &mut dyn Write,
) -> Result<usize> {
    let saved = against.map(Scorecard::load).transpose()?;
    let card = Scorecard::read(repo)?;
    card.print(out)?;
    if let Some(dir) = save {
        let (revision, named) = match BUILT_FROM {
            Some(revision) => (revision.to_owned(), Named::Build),
            None => (crate::git::head(repo)?, Named::Head),
        };
        let path = card.clone().save(dir, &revision, named)?;
        match named {
            Named::Build => writeln!(
                out,
                "saved: {}, named by the revision this takt was built from",
                path.display()
            )?,
            Named::Head => writeln!(
                out,
                "saved: {}, named by HEAD of {}: this takt knows no revision it was built from",
                path.display(),
                repo.display()
            )?,
        }
    }
    match saved {
        Some(saved) => card.against(&saved, out),
        None => Ok(0),
    }
}
