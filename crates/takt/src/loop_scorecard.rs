//! The loop scorecard (design:recursive-self-improvement § Improving the loop): one row per
//! improvement turn, a `self.improvement` case under `.engineering/cases/`, read from the
//! case's own records, and figures computed from the rows each time they are asked for, never
//! kept apart from them. The metric definitions are data, [`DEFINITIONS`], read at a revision
//! through git and never from the working tree; the baseline's are snapshotted into the loop
//! case when it opens ([`snapshot_baseline`]), so a definition is judged as it stood at the
//! base. Three validators drive a `loop.improvement/1` case: [`LoopMeasure`]
//! (`loop.measure`), [`LoopDefinitionCheck`] (`loop.definition.check`) and [`LoopCompare`]
//! (`loop.compare`).
//!
//! A row's figures come from the turn's records only: its verdict, whether it was promoted, and
//! when its finding was verified from the decisions Canon wrote for it; its tokens and wall
//! time from its run record ([`crate::scorecard`]); its revisions from its snapshot; its
//! finding from the story it was opened for, and when that was filed from the commit that
//! added the finding to the store, read through git at the row's own baseline. What a record
//! does not hold is absent, never guessed.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context as _, Result, anyhow, bail};
use serde::{Deserialize, Serialize};

use crate::case::{CASES_DIR, CaseDir};
use crate::decision::Decision;
use crate::evidence::Record;
use crate::validator::{Artifact, Observation, Validator};

/// The metric definitions of the loop scorecard, a path of the repository, read at a revision.
pub const DEFINITIONS: &str = "protocols/loop-improvement/metrics.yaml";

/// The format of [`DEFINITIONS`].
pub const DEFINITIONS_FORMAT: &str = "takt-loop-metrics/1";

/// The format of the measurement and comparison `loop.measure` and `loop.compare` retain.
pub const MEASUREMENT_FORMAT: &str = "takt-loop-measurement/1";

/// The protocol of an improvement turn's case, as its `case.yaml` names it.
pub const TURN_PROTOCOL: &str = "self.improvement";

/// The turns a measurement or a side of a comparison needs at least.
pub const MIN_TURNS: usize = 3;

/// One improvement turn's row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LoopScorecardRow {
    /// The turn: its case id.
    pub turn: String,
    /// The finding's key: the store artifact the case was opened for, `story:<key>` or
    /// `decision-blocker:<key>` as `scripts/file-finding` files it; absent when it records none.
    pub finding: Option<String>,
    /// The `baseline` artifact's revision in the case's snapshot.
    pub baseline: String,
    /// The `candidate` artifact's revision in the case's snapshot.
    pub candidate: String,
    /// The outcome legitimate at the case's latest decision; absent while the turn is open.
    pub verdict: Option<String>,
    /// Whether the case's latest decision holds the claim `promoted`.
    pub promoted: bool,
    /// The tokens of the case's runs; absent unless every harness step of its run record
    /// reports a count, and one does.
    pub tokens: Option<u64>,
    /// The wall time of the case's runs, in milliseconds; absent when it has no run record.
    pub wall_ms: Option<u64>,
    /// When the finding was filed: the time of the commit that added its artifact to the store,
    /// read through git at the row's own `baseline` revision; absent when that cannot be read,
    /// or when it comes after `verified_at`.
    pub filed_at: Option<String>,
    /// When the finding was verified gone: the instant of the first decision of the last
    /// unbroken run of decisions holding `scenario.passes_after`, when the latest holds it;
    /// absent until then.
    pub verified_at: Option<String>,
}

/// The loop scorecard: one row per recorded improvement turn, in case order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LoopScorecard {
    /// The rows.
    pub rows: Vec<LoopScorecardRow>,
}

/// A target: the figure at least, or at most, this value.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Target {
    /// The figure meets the target at this value or above.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at_least: Option<f64>,
    /// The figure meets the target at this value or below.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at_most: Option<f64>,
}

impl Target {
    /// Whether `value` misses the target.
    #[must_use]
    pub fn missed_by(&self, value: f64) -> bool {
        self.at_least.is_some_and(|least| value < least)
            || self.at_most.is_some_and(|most| value > most)
    }

    /// Whether `after` is better than `before` in the target's direction.
    #[must_use]
    pub fn improves(&self, before: f64, after: f64) -> bool {
        if self.at_least.is_some() {
            after > before
        } else {
            after < before
        }
    }
}

/// One definition of a metric: the version the code computes it under, what it is, its target.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Definition {
    /// The version; a changed definition takes another.
    pub version: u32,
    /// What the figure is, in words.
    pub definition: String,
    /// The target; a metric without one is never below it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<Target>,
}

/// A metric: its current definition and the previous ones whose figures print beside it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Metric {
    /// The current definition's version.
    pub version: u32,
    /// The current definition, in words.
    pub definition: String,
    /// The current definition's target.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<Target>,
    /// Previous definitions, each computed and printed beside the current one.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub previous: Vec<Definition>,
}

impl Metric {
    /// The current definition.
    #[must_use]
    pub fn current(&self) -> Definition {
        Definition {
            version: self.version,
            definition: self.definition.clone(),
            target: self.target,
        }
    }
}

/// The metric definitions, [`DEFINITIONS`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Definitions {
    /// [`DEFINITIONS_FORMAT`].
    pub format: String,
    /// Each metric by id.
    pub metrics: BTreeMap<String, Metric>,
}

/// What [`Definitions::at`] found at a revision.
#[derive(Debug, Clone, PartialEq)]
pub enum Found {
    /// The definitions.
    Definitions(Definitions),
    /// The revision holds no file at [`DEFINITIONS`].
    Absent,
    /// The file does not read as definitions, for the reason given.
    Malformed(String),
    /// The revision is no commit of the repository (an artifact not yet set, `r0`).
    NoCommit,
}

/// Whether `revision` can be read through git as a commit id: 7 to 64 hexadecimal digits, so
/// it is never taken for an option.
fn is_commit_id(revision: &str) -> bool {
    (7..=64).contains(&revision.len()) && revision.bytes().all(|b| b.is_ascii_hexdigit())
}

impl Definitions {
    /// Reads definitions from `text`: [`DEFINITIONS_FORMAT`], each target with exactly one
    /// bound, and no version of a metric defined twice.
    pub fn parse(text: &str) -> Result<Self> {
        let definitions: Self =
            serde_yaml_ng::from_str(text).context("read the loop metric definitions")?;
        if definitions.format != DEFINITIONS_FORMAT {
            bail!(
                "the loop metric definitions are {}, not {DEFINITIONS_FORMAT}",
                definitions.format
            );
        }
        for (id, metric) in &definitions.metrics {
            let mut versions = vec![metric.version];
            for definition in std::iter::once(metric.current()).chain(metric.previous.clone()) {
                if let Some(target) = definition.target
                    && target.at_least.is_some() == target.at_most.is_some()
                {
                    bail!(
                        "{id} v{}: a target is at_least or at_most",
                        definition.version
                    );
                }
            }
            for previous in &metric.previous {
                if versions.contains(&previous.version) {
                    bail!("{id} v{} is defined twice", previous.version);
                }
                versions.push(previous.version);
            }
        }
        Ok(definitions)
    }

    /// The definitions at `revision` of `tree`, read through git, never from the working tree.
    pub fn at(tree: &Path, revision: &str) -> Result<Found> {
        if !is_commit_id(revision)
            || crate::git::run(tree, &["cat-file", "-e", &format!("{revision}^{{commit}}")])
                .is_err()
        {
            return Ok(Found::NoCommit);
        }
        let object = format!("{revision}:{DEFINITIONS}");
        if crate::git::run(tree, &["cat-file", "-e", &object]).is_err() {
            return Ok(Found::Absent);
        }
        let bytes = crate::git::blob(tree, &object)?;
        let Ok(text) = String::from_utf8(bytes) else {
            return Ok(Found::Malformed("not UTF-8".to_owned()));
        };
        Ok(match Self::parse(&text) {
            Ok(definitions) => Found::Definitions(definitions),
            Err(error) => Found::Malformed(format!("{error:#}")),
        })
    }
}

/// One figure of the loop scorecard, computed from the rows it names.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Figure {
    /// The metric.
    pub metric: String,
    /// The version of the definition it was computed under.
    pub version: u32,
    /// Whether that definition is the metric's current one; a previous one prints beside it.
    pub current: bool,
    /// The figure; absent when no row counts towards it.
    pub value: Option<f64>,
    /// The figure as printed: a share as `<accepted>/<all>`.
    pub shown: String,
    /// The definition's target.
    pub target: Option<Target>,
    /// Whether the figure misses the target, or has a target and no value (no data fails
    /// closed).
    pub below_target: bool,
    /// The turns it was computed from, by case id.
    pub cases: Vec<String>,
}

/// The median of `values`, none for none.
fn median(values: &[f64]) -> Option<f64> {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let n = sorted.len();
    match n {
        0 => None,
        _ if n % 2 == 1 => Some(sorted[n / 2]),
        _ => Some(f64::midpoint(sorted[n / 2 - 1], sorted[n / 2])),
    }
}

/// A figure as printed: whole numbers without a fraction, others to three places.
fn shown(value: Option<f64>) -> String {
    match value {
        None => "-".to_owned(),
        Some(value) if value.fract() == 0.0 => format!("{value:.0}"),
        Some(value) => format!("{value:.3}"),
    }
}

/// An instant as Canon writes one, `YYYY-MM-DDTHH:MM:SS[.f...]Z`, in nanoseconds since the
/// epoch; none for any other text.
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

/// The figure of `metric` under `version` over `rows`: its value, as printed, and the turns
/// it was computed from. A version the code does not compute is an error naming it.
fn compute(
    metric: &str,
    version: u32,
    rows: &[&LoopScorecardRow],
) -> Result<(Option<f64>, String, Vec<String>)> {
    let all: Vec<String> = rows.iter().map(|row| row.turn.clone()).collect();
    let share = |counted: usize| {
        let value = (!rows.is_empty()).then(|| counted as f64 / rows.len() as f64);
        (value, format!("{counted}/{}", rows.len()), all.clone())
    };
    Ok(match (metric, version) {
        ("turns", 1) => (Some(rows.len() as f64), rows.len().to_string(), all),
        ("accepted_share", 1) => share(
            rows.iter()
                .filter(|row| row.verdict.as_deref() == Some("accepted"))
                .count(),
        ),
        ("accepted_share", 2) => {
            let closed: Vec<&&LoopScorecardRow> =
                rows.iter().filter(|row| row.verdict.is_some()).collect();
            let counted = closed
                .iter()
                .filter(|row| row.verdict.as_deref() == Some("accepted"))
                .count();
            (
                (!closed.is_empty()).then(|| counted as f64 / closed.len() as f64),
                format!("{counted}/{}", closed.len()),
                closed.iter().map(|row| row.turn.clone()).collect(),
            )
        }
        ("promoted_share", 1) => share(rows.iter().filter(|row| row.promoted).count()),
        ("median_tokens_per_accepted_turn", 1) => {
            let counted: Vec<&&LoopScorecardRow> = rows
                .iter()
                .filter(|row| row.verdict.as_deref() == Some("accepted") && row.tokens.is_some())
                .collect();
            let values: Vec<f64> = counted
                .iter()
                .filter_map(|row| row.tokens.map(|tokens| tokens as f64))
                .collect();
            let value = median(&values);
            (
                value,
                shown(value),
                counted.iter().map(|row| row.turn.clone()).collect(),
            )
        }
        ("finding_to_verified", 1) => {
            let mut values = Vec::new();
            let mut cases = Vec::new();
            for row in rows {
                if let (Some(filed), Some(verified)) = (
                    row.filed_at.as_deref().and_then(instant),
                    row.verified_at.as_deref().and_then(instant),
                ) {
                    values.push((verified - filed) as f64 / 1e9);
                    cases.push(row.turn.clone());
                }
            }
            let value = median(&values);
            (value, shown(value), cases)
        }
        _ => bail!("the loop scorecard computes no {metric} v{version}"),
    })
}

/// The decisions under `case_dir`, `decisions/<n>.json` in the order of `n`, each with the
/// instant it was computed at, when it names one.
fn decisions(case_dir: &Path) -> Result<Vec<(Decision, Option<String>)>> {
    let dir = case_dir.join("decisions");
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut numbered: Vec<(u64, std::path::PathBuf)> = Vec::new();
    for entry in std::fs::read_dir(&dir).with_context(|| format!("read {}", dir.display()))? {
        let path = entry?.path();
        if path.extension().is_none_or(|extension| extension != "json") {
            continue;
        }
        if let Some(n) = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .and_then(|stem| stem.parse::<u64>().ok())
        {
            numbered.push((n, path));
        }
    }
    numbered.sort();
    numbered
        .into_iter()
        .map(|(_, path)| {
            let json = std::fs::read_to_string(&path)
                .with_context(|| format!("read {}", path.display()))?;
            let at = serde_json::from_str::<serde_json::Value>(&json)
                .ok()
                .and_then(|doc| {
                    doc["explanation"]["computed_from"]["at"]
                        .as_str()
                        .map(str::to_owned)
                });
            let decision =
                Decision::from_json(&json).with_context(|| format!("read {}", path.display()))?;
            Ok((decision, at))
        })
        .collect()
}

/// The revision of `artifact` in the snapshot `doc`.
fn revision_in(doc: &serde_yaml_ng::Value, artifact: &str) -> Option<String> {
    doc["artifacts"][artifact]["revision"]
        .as_str()
        .map(str::to_owned)
}

/// The snapshot of the case at `case_dir`.
fn snapshot(case_dir: &Path) -> Result<serde_yaml_ng::Value> {
    let path = case_dir.join("case.yaml");
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    serde_yaml_ng::from_str(&text).with_context(|| format!("read {}", path.display()))
}

/// The finding's key and its file in the planning store, for the store artifact `artifact` a
/// case was opened for: `story:<key>` or `decision-blocker:<key>` (`blocker:<key>` as
/// `scripts/file-finding` names the kind), the key lower-case letters and digits in words joined
/// by `-`; none for anything else.
fn store_path(artifact: &str) -> Option<(String, String)> {
    let (kind, key) = artifact.split_once(':')?;
    let dir = match kind {
        "story" => "story",
        "blocker" | "decision-blocker" => "decision-blocker",
        _ => return None,
    };
    let is_key = key.split('-').all(|word| {
        !word.is_empty()
            && word
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
    });
    is_key.then(|| {
        (
            key.to_owned(),
            format!(".engineering/planning/{dir}/{key}.md"),
        )
    })
}

/// When the commit that added `path` to the history of `base` was made, its committer time as
/// Canon writes an instant; the earliest when it was added more than once. None when `base` is
/// no commit id or git cannot say.
fn added_at(repo: &Path, base: &str, path: &str) -> Option<String> {
    if !is_commit_id(base) {
        return None;
    }
    let listed = crate::git::run(
        repo,
        &["log", "--diff-filter=A", "--format=%ct", base, "--", path],
    )
    .ok()?;
    let seconds: i64 = listed.lines().last()?.trim().parse().ok()?;
    time::OffsetDateTime::from_unix_timestamp(seconds)
        .ok()?
        .format(&time::macros::format_description!(
            "[year]-[month]-[day]T[hour]:[minute]:[second]Z"
        ))
        .ok()
}

impl LoopScorecard {
    /// The rows as [`Self::read_at`] reads them with no base: no `filed_at`.
    pub fn read(repo: &Path) -> Result<Self> {
        Self::read_at(repo, None)
    }

    /// The rows of the `self.improvement` cases under `repo`'s cases directory, in name order;
    /// none when the directory is absent. A case of another protocol is no turn. With `base`
    /// given (the revision the caller works at), each row's `filed_at` is read through git at
    /// that row's own `baseline` revision, never at `base`; with none, no row has one. A
    /// snapshot, a decision or a run record that does not read is an error naming it.
    pub fn read_at(repo: &Path, base: Option<&str>) -> Result<Self> {
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
            Err(error) => return Err(error).with_context(|| format!("read {}", cases.display())),
        }
        names.sort();
        let spent: BTreeMap<String, crate::scorecard::Case> =
            crate::scorecard::Scorecard::read(repo)?
                .cases()
                .into_iter()
                .map(|case| (case.case.clone(), case))
                .collect();
        let mut rows = Vec::new();
        for name in names {
            let dir = cases.join(&name);
            if !dir.join("case.yaml").is_file() {
                continue;
            }
            let doc = snapshot(&dir)?;
            if doc["protocol"].as_str() != Some(TURN_PROTOCOL) {
                continue;
            }
            let revision = |artifact: &str| {
                revision_in(&doc, artifact).ok_or_else(|| {
                    anyhow!(
                        "{}: the turn names no {artifact} revision",
                        dir.join("case.yaml").display()
                    )
                })
            };
            let decided = decisions(&dir)?;
            let latest = decided.last().map(|(decision, _)| decision);
            let verified_at = decided
                .iter()
                .rev()
                .take_while(|(decision, _)| decision.holds("scenario.passes_after"))
                .last()
                .and_then(|(_, at)| at.clone());
            let artifact = crate::case::story_of(&dir)?;
            let stored = artifact.as_deref().and_then(store_path);
            let baseline = revision("baseline")?;
            // A filing later than the verification is no filing time: the store was committed
            // after the run, and the time from finding to verified would come out negative.
            let filed_at = match (base, &stored) {
                (Some(_), Some((_, path))) => added_at(repo, &baseline, path),
                _ => None,
            }
            .filter(|filed| {
                match (instant(filed), verified_at.as_deref().and_then(instant)) {
                    (Some(filed), Some(verified)) => filed <= verified,
                    _ => true,
                }
            });
            // A step reports its tokens only when its harness counted them: the turn's tokens
            // are known only when every harness step of its record (every step but a
            // validator's) reported a count, and at least one did; a partial sum, or a record
            // with no count at all, is no count.
            let lines = crate::events::Events::new(&dir).read()?;
            let steps: Vec<&serde_json::Value> = lines
                .iter()
                .filter(|line| line["event"] == "step" && line["by"] != "validator")
                .collect();
            let counted =
                !steps.is_empty() && steps.iter().all(|line| line["tokens"].as_u64().is_some());
            let spend = spent.get(&name);
            rows.push(LoopScorecardRow {
                turn: name.clone(),
                finding: stored.map(|(key, _)| key),
                baseline,
                candidate: revision("candidate")?,
                verdict: latest.and_then(|decision| decision.legitimate().into_iter().next()),
                promoted: latest.is_some_and(|decision| decision.holds("promoted")),
                tokens: spend.filter(|_| counted).map(|case| case.tokens),
                wall_ms: spend.map(|case| case.wall_ms),
                filed_at,
                verified_at,
            });
        }
        Ok(Self { rows })
    }

    /// The figures of every metric `definitions` defines over every row: the current
    /// definition's, then each previous one's, metrics in id order.
    pub fn figures(&self, definitions: &Definitions) -> Result<Vec<Figure>> {
        figures_of(&self.rows.iter().collect::<Vec<_>>(), definitions)
    }
}

/// The figures of every metric `definitions` defines over `rows`.
fn figures_of(rows: &[&LoopScorecardRow], definitions: &Definitions) -> Result<Vec<Figure>> {
    let mut figures = Vec::new();
    for (id, metric) in &definitions.metrics {
        let current = std::iter::once((metric.current(), true));
        let previous = metric.previous.iter().cloned().map(|d| (d, false));
        for (definition, is_current) in current.chain(previous) {
            let (value, shown, cases) = compute(id, definition.version, rows)?;
            figures.push(Figure {
                metric: id.clone(),
                version: definition.version,
                current: is_current,
                // A target with no figure to hold to it fails closed: no data is below target.
                below_target: match (value, definition.target) {
                    (Some(value), Some(target)) => target.missed_by(value),
                    (None, Some(_)) => true,
                    (_, None) => false,
                },
                value,
                shown,
                target: definition.target,
                cases,
            });
        }
    }
    Ok(figures)
}

/// Each figure as `<metric> v<n> <shown>`, a missed target marked.
fn line(figures: &[Figure]) -> String {
    figures
        .iter()
        .map(|figure| {
            format!(
                "{} v{}{} {}{}",
                figure.metric,
                figure.version,
                if figure.current { "" } else { " (previous)" },
                figure.shown,
                if figure.below_target {
                    " below target"
                } else {
                    ""
                }
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// The first eight characters of a revision.
fn short(revision: &str) -> &str {
    revision.get(..8).unwrap_or(revision)
}

/// An observation that recorded nothing.
fn nothing(summary: String) -> Observation {
    Observation {
        summary,
        records: Vec::new(),
        artifact: None,
    }
}

/// The definitions at `revision`, or what to say when there are none to read.
fn definitions_at(tree: &Path, revision: &str) -> Result<Result<Definitions, String>> {
    Ok(match Definitions::at(tree, revision)? {
        Found::Definitions(definitions) => Ok(definitions),
        Found::Absent => Err(format!("{} holds no {DEFINITIONS}", short(revision))),
        Found::Malformed(why) => Err(format!("{DEFINITIONS} at {}: {why}", short(revision))),
        Found::NoCommit => Err(format!("{revision} is no commit")),
    })
}

/// The revision of `artifact` in the case whose branch `tree` is on.
fn case_revision(tree: &Path, artifact: &str) -> Result<String> {
    let dir = crate::case::current(tree)?;
    revision_in(&snapshot(&dir)?, artifact)
        .ok_or_else(|| anyhow!("the case at {} names no {artifact} revision", dir.display()))
}

/// The file under a loop case's directory holding the baseline's metric definitions as they
/// were read when the case opened ([`snapshot_baseline`]).
pub const BASELINE_SNAPSHOT: &str = "loop-baseline.json";

/// The format of [`BASELINE_SNAPSHOT`].
pub const BASELINE_SNAPSHOT_FORMAT: &str = "takt-loop-baseline/1";

/// Snapshots the metric definitions at the loop case `case`'s `loop_baseline` revision into
/// the case's directory as [`BASELINE_SNAPSHOT`]: the revision as the case names it, the
/// commit it is, and the bytes of [`DEFINITIONS`] there (none when it holds no such file).
/// Called when the case opens, before any step: a read of the base after a step ran trusts an
/// object store the step could write, while the case directory is guarded around every step.
/// Answers whether it wrote one: nothing is written while `loop_baseline` names no commit.
pub fn snapshot_baseline(tree: &Path, case: &CaseDir) -> Result<bool> {
    let Some(revision) = case.revision("loop_baseline") else {
        return Ok(false);
    };
    if !is_commit_id(&revision) {
        return Ok(false);
    }
    let Ok(commit) = crate::git::run(
        tree,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{revision}^{{commit}}"),
        ],
    ) else {
        return Ok(false);
    };
    let object = format!("{commit}:{DEFINITIONS}");
    let definitions = if crate::git::run(tree, &["cat-file", "-e", &object]).is_ok() {
        Some(
            String::from_utf8(crate::git::blob(tree, &object)?)
                .map_err(|_| anyhow!("{DEFINITIONS} at {} is not UTF-8", short(&commit)))?,
        )
    } else {
        None
    };
    let snapshot = serde_json::json!({
        "format": BASELINE_SNAPSHOT_FORMAT,
        "revision": revision,
        "commit": commit,
        "definitions": definitions,
    });
    let path = case.dir.join(BASELINE_SNAPSHOT);
    std::fs::write(
        &path,
        serde_json::to_vec_pretty(&snapshot).context("serialize the baseline snapshot")?,
    )
    .with_context(|| format!("write {}", path.display()))?;
    Ok(true)
}

/// The baseline's definitions from the snapshot in the case whose branch `tree` is on, taken
/// when it opened ([`snapshot_baseline`]), never read from the object store again; or what to
/// say when there is no snapshot of `base`, or no definitions in it.
fn snapshotted(tree: &Path, base: &str) -> Result<Result<Definitions, String>> {
    let path = crate::case::current(tree)?.join(BASELINE_SNAPSHOT);
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Ok(Err(format!(
            "no snapshot of its definitions was taken when the case opened ({BASELINE_SNAPSHOT})"
        )));
    };
    let snapshot: serde_json::Value =
        serde_json::from_str(&text).with_context(|| format!("read {}", path.display()))?;
    if snapshot["format"] != BASELINE_SNAPSHOT_FORMAT || snapshot["revision"] != base {
        return Ok(Err(format!(
            "the snapshot {BASELINE_SNAPSHOT} is not of {}",
            short(base)
        )));
    }
    let Some(definitions) = snapshot["definitions"].as_str() else {
        return Ok(Err(format!("{} holds no {DEFINITIONS}", short(base))));
    };
    Ok(Definitions::parse(definitions)
        .map_err(|error| format!("{DEFINITIONS} at {}: {error:#}", short(base))))
}

/// `loop.measure`: the loop scorecard over the recorded turns (at least [`MIN_TURNS`]), its
/// figures under the definitions at the baseline revision, writes one `loop_measurement`
/// record about `loop_baseline`: `below_target` when a current figure misses its target, else
/// `at_target`. The figures, each naming the turns it was computed from, are retained as
/// `loop-measurement-<seq>.json`: an observation; the rows stay the source.
#[derive(Debug, Clone, Copy, Default)]
pub struct LoopMeasure;

impl Validator for LoopMeasure {
    fn name(&self) -> &str {
        "takt.loop-measure"
    }

    fn action(&self) -> &str {
        "loop.measure"
    }

    fn kind(&self) -> &str {
        "loop_measurement"
    }

    fn subject(&self) -> &str {
        "loop_baseline"
    }

    fn observe(&self, tree: &Path, revision: &str, seq: usize) -> Result<Observation> {
        let definitions = match definitions_at(tree, revision)? {
            Ok(definitions) => definitions,
            Err(why) => return Ok(nothing(format!("{why}; nothing recorded"))),
        };
        let card = LoopScorecard::read_at(tree, Some(revision))?;
        if card.rows.len() < MIN_TURNS {
            return Ok(nothing(format!(
                "{} turns recorded under {CASES_DIR}, at least {MIN_TURNS} needed; nothing recorded",
                card.rows.len()
            )));
        }
        let figures = match card.figures(&definitions) {
            Ok(figures) => figures,
            Err(error) => return Ok(nothing(format!("{error:#}; nothing recorded"))),
        };
        let result = if figures
            .iter()
            .any(|figure| figure.current && figure.below_target)
        {
            "below_target"
        } else {
            "at_target"
        };
        let turns: Vec<String> = card.rows.iter().map(|row| row.turn.clone()).collect();
        let artifact = format!("loop-measurement-{seq}.json");
        let snapshot = serde_json::json!({
            "format": MEASUREMENT_FORMAT,
            "definitions_revision": revision,
            "turns": turns,
            "result": result,
            "figures": figures,
        });
        Ok(Observation {
            summary: format!(
                "the loop scorecard over {} turns ({}) under the definitions at {}: {result}; {}; retained as {artifact}",
                turns.len(),
                turns.join(", "),
                short(revision),
                line(&figures)
            ),
            records: vec![Record::new(
                format!("loop-measurement-{seq}"),
                self.kind(),
                Some(result.to_owned()),
                self.subject(),
                revision,
                self.name(),
            )],
            artifact: Some(Artifact {
                name: artifact,
                bytes: serde_json::to_vec_pretty(&snapshot)
                    .context("serialize the loop measurement")?,
            }),
        })
    }
}

/// How the candidate's metric definitions stand against the baseline's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DefinitionCheck {
    /// Every definition of the baseline is the candidate's current one.
    Unchanged,
    /// A definition changed, took another version, kept its target, and the baseline's is kept
    /// as a previous one beside every previous one the baseline prints, so the figures of both
    /// print.
    DualReported,
    /// A definition changed and only the new one prints: it kept its version, dropped the
    /// baseline's or a previous one the baseline prints, or the metric is gone; or its target
    /// changed.
    Redefined,
}

impl DefinitionCheck {
    /// The result as the protocol names it.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unchanged => "unchanged",
            Self::DualReported => "dual_reported",
            Self::Redefined => "redefined",
        }
    }
}

/// Each metric of `base` with how `candidate` stands against it, and the result over all: the
/// worst of them. No candidate definitions (absent or unreadable) redefines every metric. A
/// metric whose target changed (its value or its direction) is redefined whatever its version
/// and its previous definitions say: targets are the owner's, and no candidate changes one
/// through this check. So is one that drops or changes a previous definition the base prints.
#[must_use]
pub fn check(
    base: &Definitions,
    candidate: Option<&Definitions>,
) -> (DefinitionCheck, Vec<(String, DefinitionCheck)>) {
    let each: Vec<(String, DefinitionCheck)> = base
        .metrics
        .iter()
        .map(|(id, metric)| {
            let was = metric.current();
            let stands = match candidate.and_then(|c| c.metrics.get(id)) {
                None => DefinitionCheck::Redefined,
                Some(now) if now.target != was.target => DefinitionCheck::Redefined,
                Some(now)
                    if metric
                        .previous
                        .iter()
                        .any(|printed| !now.previous.contains(printed)) =>
                {
                    DefinitionCheck::Redefined
                }
                Some(now) if now.current() == was => DefinitionCheck::Unchanged,
                Some(now) if now.version != was.version && now.previous.contains(&was) => {
                    DefinitionCheck::DualReported
                }
                Some(_) => DefinitionCheck::Redefined,
            };
            (id.clone(), stands)
        })
        .collect();
    let overall = each
        .iter()
        .map(|(_, stands)| *stands)
        .max()
        .unwrap_or(DefinitionCheck::Unchanged);
    (overall, each)
}

/// The metrics of `definitions` some figure of which the code does not compute over the rows of
/// `card`, current or previous, each with why.
fn uncomputed(card: &LoopScorecard, definitions: &Definitions) -> Vec<(String, String)> {
    let rows: Vec<&LoopScorecardRow> = card.rows.iter().collect();
    definitions
        .metrics
        .iter()
        .filter_map(|(id, metric)| {
            std::iter::once(metric.current())
                .chain(metric.previous.iter().cloned())
                .find_map(|definition| compute(id, definition.version, &rows).err())
                .map(|error| (id.clone(), format!("{error:#}")))
        })
        .collect()
}

/// `loop.definition.check`: the metric definitions at the candidate revision, read through git,
/// against the baseline's as the case snapshotted them when it opened ([`snapshot_baseline`]),
/// never read from the object store after a step ran; writes one `definition_check` record
/// about `loop_candidate`, its result [`DefinitionCheck`]. The figures the candidate's
/// definitions name are then computed over the recorded turns: a metric any of whose figures
/// the code does not compute is `redefined`, whatever [`check`] found, since its figure cannot
/// print; so is every metric when the turns do not read. No snapshot of the case's current
/// `loop_baseline`, or no definitions in it, records nothing.
#[derive(Debug, Clone, Copy, Default)]
pub struct LoopDefinitionCheck;

impl Validator for LoopDefinitionCheck {
    fn name(&self) -> &str {
        "takt.loop-definition-check"
    }

    fn action(&self) -> &str {
        "loop.definition.check"
    }

    fn kind(&self) -> &str {
        "definition_check"
    }

    fn subject(&self) -> &str {
        "loop_candidate"
    }

    fn observe(&self, tree: &Path, revision: &str, seq: usize) -> Result<Observation> {
        let base = case_revision(tree, "loop_baseline")?;
        let base_definitions = match snapshotted(tree, &base)? {
            Ok(definitions) => definitions,
            Err(why) => return Ok(nothing(format!("the baseline: {why}; nothing recorded"))),
        };
        let (candidate, why) = match Definitions::at(tree, revision)? {
            Found::Definitions(definitions) => (Some(definitions), String::new()),
            Found::Absent => (
                None,
                format!("; {} holds no {DEFINITIONS}", short(revision)),
            ),
            Found::Malformed(why) => (None, format!("; {DEFINITIONS}: {why}")),
            Found::NoCommit => {
                return Ok(nothing(format!(
                    "the candidate {revision} is no commit; nothing recorded"
                )));
            }
        };
        let (mut result, mut each) = check(&base_definitions, candidate.as_ref());
        let mut why = why;
        if let Some(candidate) = &candidate {
            let failed: Vec<(String, String)> = match LoopScorecard::read_at(tree, Some(&base)) {
                Ok(card) => uncomputed(&card, candidate),
                Err(error) => candidate
                    .metrics
                    .keys()
                    .map(|id| (id.clone(), format!("{error:#}")))
                    .collect(),
            };
            for (id, reason) in &failed {
                match each.iter_mut().find(|(each_id, _)| each_id == id) {
                    Some((_, stands)) => *stands = DefinitionCheck::Redefined,
                    None => each.push((id.clone(), DefinitionCheck::Redefined)),
                }
                why.push_str(&format!("; {id} does not print: {reason}"));
            }
            if !failed.is_empty() {
                result = DefinitionCheck::Redefined;
            }
        }
        let changed: Vec<String> = each
            .iter()
            .filter(|(_, stands)| *stands != DefinitionCheck::Unchanged)
            .map(|(id, stands)| format!("{id} {}", stands.as_str()))
            .collect();
        Ok(Observation {
            summary: format!(
                "the metric definitions at {} against the baseline's at {}: {}{}{why}",
                short(revision),
                short(&base),
                result.as_str(),
                if changed.is_empty() {
                    String::new()
                } else {
                    format!(" ({})", changed.join(", "))
                }
            ),
            records: vec![Record::new(
                format!("definition-check-{seq}"),
                self.kind(),
                Some(result.as_str().to_owned()),
                self.subject(),
                revision,
                self.name(),
            )],
            artifact: None,
        })
    }
}

/// The evidence records of the case whose branch `tree` is on, in name order (the order they
/// were written), each read as YAML, of which a JSON record is one.
fn case_records(tree: &Path) -> Result<Vec<serde_yaml_ng::Value>> {
    let dir = crate::case::current(tree)?.join("evidence");
    let mut paths: Vec<std::path::PathBuf> = Vec::new();
    for entry in std::fs::read_dir(&dir).with_context(|| format!("read {}", dir.display()))? {
        let path = entry?.path();
        if path
            .extension()
            .is_some_and(|extension| extension == "json" || extension == "yaml")
        {
            paths.push(path);
        }
    }
    paths.sort();
    paths
        .iter()
        .map(|path| {
            let text = std::fs::read_to_string(path)
                .with_context(|| format!("read {}", path.display()))?;
            serde_yaml_ng::from_str(&text).with_context(|| format!("read {}", path.display()))
        })
        .collect()
}

/// Whether `record` is of `kind` with `result` (any when none) about `subject` at `revision`.
fn is_record(
    record: &serde_yaml_ng::Value,
    kind: &str,
    result: Option<&str>,
    subject: &str,
    revision: &str,
) -> bool {
    record["kind"].as_str() == Some(kind)
        && result.is_none_or(|result| record["result"].as_str() == Some(result))
        && record["subject"].as_str() == Some(subject)
        && record["subject_revision"].as_str() == Some(revision)
}

/// The turns the case's latest `loop_measurement` about `loop_baseline` at `base` read, as the
/// measurement it retained names them; none without such a measurement or its retained file.
fn measured_turns(tree: &Path, base: &str) -> Result<Option<Vec<String>>> {
    let Some(id) = case_records(tree)?
        .iter()
        .rev()
        .find(|record| is_record(record, "loop_measurement", None, "loop_baseline", base))
        .and_then(|record| record["id"].as_str().map(str::to_owned))
    else {
        return Ok(None);
    };
    let path = crate::case::current(tree)?
        .join("artifacts")
        .join(format!("{id}.json"));
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Ok(None);
    };
    let retained: serde_json::Value =
        serde_json::from_str(&text).with_context(|| format!("read {}", path.display()))?;
    Ok(retained["turns"].as_array().map(|turns| {
        turns
            .iter()
            .filter_map(|turn| turn.as_str().map(str::to_owned))
            .collect()
    }))
}

/// Whether the commit `revision` descends from the commit `ancestor`, or is it; never for a
/// revision that is no commit id.
fn descends(tree: &Path, ancestor: &str, revision: &str) -> bool {
    is_commit_id(ancestor)
        && is_commit_id(revision)
        && crate::git::run(tree, &["merge-base", "--is-ancestor", ancestor, revision]).is_ok()
}

/// `loop.compare`: due once the case holds a `merge_observation` of the loop candidate (the
/// protocol's precondition; nothing is recorded without one). The turns whose baseline revision
/// descends from the merged candidate, at least [`MIN_TURNS`] of them with a verdict, against
/// the turns the case's `loop_measurement` of the baseline read, under the definitions at the
/// case's `loop_baseline` revision. The metrics compared are those whose current figure missed
/// its target before; `improved` when each of them is better after, in its target's direction,
/// else `not_improved`. Writes one `loop_comparison` record about `loop_candidate` and retains
/// both sides' figures as `loop-comparison-<seq>.json`.
#[derive(Debug, Clone, Copy, Default)]
pub struct LoopCompare;

impl Validator for LoopCompare {
    fn name(&self) -> &str {
        "takt.loop-compare"
    }

    fn action(&self) -> &str {
        "loop.compare"
    }

    fn kind(&self) -> &str {
        "loop_comparison"
    }

    fn subject(&self) -> &str {
        "loop_candidate"
    }

    fn observe(&self, tree: &Path, revision: &str, seq: usize) -> Result<Observation> {
        let base = case_revision(tree, "loop_baseline")?;
        let definitions = match definitions_at(tree, &base)? {
            Ok(definitions) => definitions,
            Err(why) => return Ok(nothing(format!("the baseline: {why}; nothing recorded"))),
        };
        let records = case_records(tree)?;
        if !records.iter().any(|record| {
            is_record(
                record,
                "merge_observation",
                Some("merged"),
                "loop_candidate",
                revision,
            )
        }) {
            return Ok(nothing(format!(
                "no merge of the loop candidate {} is recorded; nothing recorded",
                short(revision)
            )));
        }
        let Some(measured) = measured_turns(tree, &base)? else {
            return Ok(nothing(format!(
                "no loop measurement of the baseline {} is retained in the case; nothing recorded",
                short(&base)
            )));
        };
        let card = LoopScorecard::read_at(tree, Some(&base))?;
        let before: Vec<&LoopScorecardRow> = card
            .rows
            .iter()
            .filter(|row| measured.contains(&row.turn))
            .collect();
        let after: Vec<&LoopScorecardRow> = card
            .rows
            .iter()
            .filter(|row| descends(tree, revision, &row.baseline))
            .collect();
        let ran = after.iter().filter(|row| row.verdict.is_some()).count();
        if ran < MIN_TURNS {
            return Ok(nothing(format!(
                "{ran} turns with a verdict on a baseline that holds the candidate {}, at least {MIN_TURNS} needed; nothing recorded",
                short(revision)
            )));
        }
        let (was, now) = match (
            figures_of(&before, &definitions),
            figures_of(&after, &definitions),
        ) {
            (Ok(was), Ok(now)) => (was, now),
            (Err(error), _) | (_, Err(error)) => {
                return Ok(nothing(format!("{error:#}; nothing recorded")));
            }
        };
        let mut compared = Vec::new();
        let mut improved = true;
        for figure in was.iter().filter(|f| f.current && f.below_target) {
            compared.push(figure.metric.clone());
            let later = now
                .iter()
                .find(|f| f.current && f.metric == figure.metric)
                .and_then(|f| f.value);
            // A metric below target for want of data before improved once it has a figure
            // after that meets its target.
            improved &= match (figure.value, later, figure.target) {
                (Some(before), Some(after), Some(target)) => target.improves(before, after),
                (None, Some(after), Some(target)) => !target.missed_by(after),
                _ => false,
            };
        }
        let result = if improved && !compared.is_empty() {
            "improved"
        } else {
            "not_improved"
        };
        let artifact = format!("loop-comparison-{seq}.json");
        let retained = serde_json::json!({
            "format": MEASUREMENT_FORMAT,
            "definitions_revision": base,
            "candidate": revision,
            "compared": compared,
            "result": result,
            "before": { "turns": before.iter().map(|row| &row.turn).collect::<Vec<_>>(), "figures": was },
            "after": { "turns": after.iter().map(|row| &row.turn).collect::<Vec<_>>(), "figures": now },
        });
        Ok(Observation {
            summary: format!(
                "{} turns before the merge of {} against {} after, on {}: {result}; before: {}; after: {}; retained as {artifact}",
                before.len(),
                short(revision),
                after.len(),
                if compared.is_empty() {
                    "no metric below its target before".to_owned()
                } else {
                    compared.join(", ")
                },
                line(&was),
                line(&now)
            ),
            records: vec![Record::new(
                format!("loop-comparison-{seq}"),
                self.kind(),
                Some(result.to_owned()),
                self.subject(),
                revision,
                self.name(),
            )],
            artifact: Some(Artifact {
                name: artifact,
                bytes: serde_json::to_vec_pretty(&retained)
                    .context("serialize the loop comparison")?,
            }),
        })
    }
}
