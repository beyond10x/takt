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
//! with fewer cases than the rule's trials, none included, gets no verdict, and so does a
//! scenario with a `DiskLow` trial on either side, whatever the count ([`DISK_LOW`]). Tokens are
//! printed per
//! scenario and side as n, minimum, median and maximum over cases with the difference of the
//! medians. A metric whose definition changed between the two is printed with both figures and
//! does not make a scenario worse, but for one with no figure a case (`step_tokens`, by action),
//! which prints none; outcomes are compared under the current definition whatever
//! the saved one's version. The rule applied is printed with every comparison.
//!
//! A candidate that targets steps (the actions its finding names) is judged on them too
//! ([`Scorecard::judge_steps`], `takt scorecard --against --steps`), under the `step_tokens`
//! definition: each run keeps its tokens by action, and per scenario and action a step is
//! better when every candidate case's tokens at it are below the saved side's cheapest, or a
//! one-sided exact Mann-Whitney U test at five cases a side or more rejects equality at 1 in 20
//! ([`rank_test`]). The whole run is still judged as above, and a candidate better on its steps
//! but worse on the run is refused, naming both. A harness step that recorded no tokens leaves
//! its action's tokens unknown for the run, never 0, and a targeted step unknown in a case of
//! either side refuses the judgement, naming the case and the step.
//!
//! A harness step (every step but a validator's) that recorded no tokens, or 0, which takt never
//! writes, leaves the run's tokens unknown too, never a sum that counts it 0, which would read
//! as a saving; a validator step reports none and counts 0. A comparison with a scenario case of
//! unknown run tokens on either side is refused before anything is judged or printed, naming
//! the case and the step ([`refuse_unknown_tokens`]). The record cannot tell a harness step
//! relabelled `validator` from a validator's: it is read as the validator's 0.

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

/// The stop of a run that stopped before a step for want of free disk
/// ([`crate::Stop::DiskLow`]). A case any run of which stopped so is a `DiskLow` trial, whatever
/// its other runs did, so a resume cannot launder an earlier failure; disk fullness measures the
/// machine, not takt, and a scenario with such a trial on either side gets no verdict, whatever
/// its count, until a fresh set without one is saved: topping it up does not clear it
/// ([`Scorecard::disk_low`], [`Scorecard::against`]; the planner's ruling).
pub const DISK_LOW: &str = "DiskLow";

/// The operator's statement of the binary a case's runs were made with, in the case directory
/// beside its record: the first line `<path>\t<sha256>`, the path as the operator wrote it
/// (`~` and all; only the digest is read, 64 lower-case hex digits), every further line the
/// operator's note. Written for a binary whose `--version` names no revision.
pub const BINARY_FILE: &str = "binary.txt";

/// Each metric with the version of its definition. A changed definition takes the next
/// version, so a comparison with a scorecard saved under the previous one prints both figures.
/// The outcome is at version 3: `Unperformable` ranks with the budget stops and a [`DISK_LOW`]
/// trial takes no rank, so a takt that knows version 2 refuses a scorecard saved under it.
/// `tokens` is at version 2: a run whose harness step recorded no tokens, or 0, has unknown
/// tokens (`null`), where version 1 summed the step as 0; a takt that knows version 1 refuses
/// a scorecard saved under it, and a baseline saved under it is compared only after
/// `takt promote --rebaseline`. `step_tokens` is a run's tokens by the action of its `step`
/// lines ([`Row::step_tokens`], an action unknown when a harness step of it recorded no tokens)
/// and the rule that judges the steps a candidate targets with them
/// ([`Scorecard::judge_steps`]); a takt that does not know it refuses a scorecard saved under
/// it. At version 2 a harness step that recorded 0 leaves its action unknown too, where version
/// 1 counted the 0.
pub const METRICS: [(&str, u32); 8] = [
    ("outcome", 3),
    ("steps", 1),
    ("tokens", 2),
    ("refusals", 1),
    ("no_progress", 1),
    ("errors", 1),
    ("wall", 1),
    ("step_tokens", 2),
];

/// The cases a side needs for the rank test of a targeted step ([`rank_test`]): five, the
/// rule's trials, at which the one-sided exact test rejects equality at [`RANK_TEST_ALPHA`] for
/// U of at most 4 (12 of the 252 arrangements).
pub const RANK_TEST_CASES: usize = 5;

/// The significance level of the rank test, as a fraction: 1 in 20, one-sided.
pub const RANK_TEST_ALPHA: (u128, u128) = (1, 20);

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
    /// The sum of the tokens reported on the run's `step` lines, a validator's that reports
    /// none counting 0; unknown (none, saved as `null`) when a harness step of the run recorded
    /// no tokens, or 0, which takt never writes: a missing record is never read as 0, which
    /// would read as a saving (metric `tokens` version 2).
    pub tokens: Option<u64>,
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
    /// The tokens of the run's `step` lines by their action, each summed over the run's steps
    /// of that action (at most `u64::MAX`, the run then `saturated`); an action with no
    /// harness step reads 0, a validator reporting none. Unknown (none, saved as `null`) when a
    /// harness step of the action recorded no `tokens`, or 0: a missing record is never read as 0,
    /// which would read as a saving ([`Scorecard::judge_steps`] refuses it). Empty on a file
    /// saved before it had the field (metric `step_tokens`).
    #[serde(default)]
    pub step_tokens: BTreeMap<String, Option<u64>>,
    /// The run's layers (ADR 0101), layer name to revision: its `opened` line's `layers`, the
    /// harness layer joined from the version its harness steps' lines state and the model
    /// layer from the model they state (each several, distinct, in name order, joined by `+`);
    /// none when the line has no `layers`, a record written before the field, which is "layers
    /// not recorded", never a revision, whatever its steps say.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layers: Option<BTreeMap<String, String>>,
}

/// The layers of the stack a run ran on (ADR 0101), in the order the scorecard and the report
/// list them: takt, the harness session layer, the model layer, the runtime; any other layer
/// a record names follows in name order.
pub const LAYERS: [&str; 4] = ["takt", "harness", "model", "runtime"];

/// `layers` as the scorecard and the report print them: `<name> <revision>` per layer, in
/// [`LAYERS`] order and then name order, joined by `; `; `not recorded` for none; `none
/// readable` for an empty map.
#[must_use]
pub fn layers_text(layers: Option<&BTreeMap<String, String>>) -> String {
    match layers {
        None => "not recorded".to_owned(),
        Some(layers) if layers.is_empty() => "none readable".to_owned(),
        Some(layers) => {
            let rank = |name: &str| LAYERS.iter().position(|known| *known == name);
            let mut names: Vec<&String> = layers.keys().collect();
            names.sort_by_key(|name| (rank(name).unwrap_or(LAYERS.len()), (*name).clone()));
            names
                .into_iter()
                .map(|name| format!("{name} {}", layers[name]))
                .collect::<Vec<_>>()
                .join("; ")
        }
    }
}

/// The first difference between the layers of `rows`, as the refusal names it: the two runs
/// (`<case>#<n>`), the layer, and each run's revision of it or that it recorded none. A run
/// that recorded no layers beside one that did differs; so does a layer one run states and
/// another does not. None when every run's layers are the same, or no run recorded any.
#[must_use]
pub fn layers_differ(rows: &[Row]) -> Option<String> {
    let mut counted: BTreeMap<&str, u64> = BTreeMap::new();
    let mut first: Option<(String, &Row)> = None;
    for row in rows {
        let n = counted.entry(row.case.as_str()).or_insert(0);
        *n += 1;
        let label = format!("{}#{n}", row.case);
        let Some((label_a, a)) = &first else {
            first = Some((label, row));
            continue;
        };
        match (&a.layers, &row.layers) {
            (None, None) => {}
            (None, Some(_)) => {
                return Some(format!(
                    "run {label_a} recorded no layers (its `opened` line has no `layers`, so its layers are not recorded), run {label} recorded {}: the runs' layers differ; a scorecard scores runs of one stack, so nothing is scored (pass --across-layers to compare across them)",
                    layers_text(row.layers.as_ref())
                ));
            }
            (Some(_), None) => {
                return Some(format!(
                    "run {label} recorded no layers (its `opened` line has no `layers`, so its layers are not recorded), run {label_a} recorded {}: the runs' layers differ; a scorecard scores runs of one stack, so nothing is scored (pass --across-layers to compare across them)",
                    layers_text(a.layers.as_ref())
                ));
            }
            (Some(first_layers), Some(layers)) => {
                let names: BTreeSet<&str> = first_layers
                    .keys()
                    .chain(layers.keys())
                    .map(String::as_str)
                    .collect();
                for name in names {
                    let (x, y) = (first_layers.get(name), layers.get(name));
                    if x == y {
                        continue;
                    }
                    let state = |revision: Option<&String>| {
                        revision
                            .map_or_else(|| "none".to_owned(), |revision| format!("`{revision}`"))
                    };
                    return Some(format!(
                        "layer `{name}`: run {label_a} records {}, run {label} records {}: the runs' layers differ; a scorecard scores runs of one stack, so nothing is scored (pass --across-layers to compare across them)",
                        state(x),
                        state(y)
                    ));
                }
            }
        }
    }
    None
}

impl Row {
    /// The steps that leave the run's tokens unknown, each named by its action, in action
    /// order: every action whose tokens are unknown ([`Row::step_tokens`]), or, for a run of
    /// unknown tokens with none such (a step line without an action), `a step without an
    /// action`; empty when the run's tokens are known.
    #[must_use]
    pub fn unknown_steps(&self) -> Vec<String> {
        let unknown: Vec<String> = self
            .step_tokens
            .iter()
            .filter(|(_, tokens)| tokens.is_none())
            .map(|(action, _)| action.clone())
            .collect();
        if unknown.is_empty() && self.tokens.is_none() {
            return vec!["a step without an action".to_owned()];
        }
        unknown
    }

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
    /// Its scenario, as [`Row::scenario`]. A `DiskLow` trial keeps its scenario, which then gets
    /// no verdict ([`DISK_LOW`]).
    pub scenario: Option<String>,
    /// The best outcome a run of it completed with, else its last run's.
    pub outcome: String,
    /// Its runs.
    pub runs: u64,
    /// Its runs' steps, summed.
    pub steps: u64,
    /// Its runs' tokens, summed over the runs whose tokens are known; no figure when
    /// `unknown_steps` names a step, which a comparison refuses before reading it
    /// ([`refuse_unknown_tokens`]).
    pub tokens: u64,
    /// The steps, by action, that leave a run's tokens unknown ([`Row::unknown_steps`]), over
    /// its runs; empty when every run's tokens are known.
    pub unknown_steps: BTreeSet<String>,
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
    /// Its runs' tokens by action ([`Row::step_tokens`]), summed; unknown when any run's is.
    pub step_tokens: BTreeMap<String, Option<u64>>,
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
            sum(|run| run.tokens.unwrap_or(0)),
            sum(|run| run.refusals),
            sum(|run| run.errors),
            sum(|run| run.wall_ms),
        );
        let mut step_tokens: BTreeMap<String, Option<u64>> = BTreeMap::new();
        for (action, tokens) in runs.iter().flat_map(|run| &run.step_tokens) {
            let sum = step_tokens.entry(action.clone()).or_insert(Some(0));
            *sum = match (*sum, *tokens) {
                (Some(sum), Some(tokens)) => Some(sum.checked_add(tokens).unwrap_or_else(|| {
                    saturated = true;
                    u64::MAX
                })),
                _ => None,
            };
        }
        Some(Self {
            case: last.case.clone(),
            scenario: last.scenario.clone(),
            no_progress: u64::from(outcome == "NoProgress"),
            outcome,
            runs: runs.len() as u64,
            steps,
            tokens,
            unknown_steps: runs.iter().flat_map(|run| run.unknown_steps()).collect(),
            refusals,
            errors,
            wall_ms,
            saturated,
            step_tokens,
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
/// `--forge none` trial comparisons. `DiskLow` takes no rank in a comparison: a scenario with a
/// `DiskLow` trial on either side gets no verdict ([`DISK_LOW`]).
#[must_use]
pub fn rank(outcome: &str) -> u8 {
    match outcome {
        "accepted" => 4,
        "ApprovalRequired" | "Waiting" => 3,
        "Iterations" | "Hours" | "Tokens" | "Unperformable" => 2,
        "error" | "unfinished" | FAILED => 0,
        "NoProgress" | "ActiveTime" => 1,
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
    (1, "NoProgress, ActiveTime or another stop"),
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
    /// A harness step recorded no tokens, or 0: the run's tokens are unknown ([`Row::tokens`]).
    tokens_unknown: bool,
    refusals: u64,
    errors: u64,
    saturated: bool,
    stop: Option<String>,
    completed: Option<bool>,
    binary: Option<String>,
    step_tokens: BTreeMap<String, Option<u64>>,
    /// The `opened` line's `layers`; none when it has none.
    layers: Option<BTreeMap<String, String>>,
    /// The models the harness steps reported, distinct.
    models: BTreeSet<String>,
    /// The harness versions the harness steps reported, distinct.
    versions: BTreeSet<String>,
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
            tokens: (!self.tokens_unknown).then_some(self.tokens),
            refusals: self.refusals,
            errors: self.errors,
            wall_ms,
            saturated: self.saturated || too_long,
            binary: None,
            opened_binary: self.binary,
            completed: self.stop.and(self.completed),
            step_tokens: self.step_tokens,
            layers: self.layers.map(|mut layers| {
                if !self.versions.is_empty() {
                    layers.entry("harness".to_owned()).or_insert_with(|| {
                        self.versions.iter().cloned().collect::<Vec<_>>().join("+")
                    });
                }
                if !self.models.is_empty() {
                    layers.entry("model".to_owned()).or_insert_with(|| {
                        self.models.iter().cloned().collect::<Vec<_>>().join("+")
                    });
                }
                layers
            }),
        }
    }
}

/// The `layers` of an `opened` line: its map's entries whose value is a text; none when the
/// line has no `layers` object.
pub(crate) fn opened_layers(line: &serde_json::Value) -> Option<BTreeMap<String, String>> {
    line["layers"].as_object().map(|layers| {
        layers
            .iter()
            .filter_map(|(name, revision)| Some((name.clone(), revision.as_str()?.to_owned())))
            .collect()
    })
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
            span.layers = opened_layers(line);
        }
        if span.stop.is_some() || line["event"] == "skipped" {
            continue;
        }
        if line["event"] == "step" && line["by"] == "harness" {
            if let Some(model) = line["model"].as_str() {
                span.models.insert(model.to_owned());
            }
            if let Some(version) = line["version"].as_str() {
                span.versions.insert(version.to_owned());
            }
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
            Some("step") => {
                let reported = line["tokens"].as_u64();
                let tokens = reported.unwrap_or(0);
                // A validator reports no tokens and spends none; a harness step (every step but
                // a validator's) without a count, or with 0, which takt never writes, leaves the
                // run's tokens and its action's unknown, never 0.
                let unknown = line["by"] != "validator" && reported.is_none_or(|n| n == 0);
                span.tokens_unknown |= unknown;
                count(&mut span.tokens, tokens, &mut span.saturated);
                if let Some(action) = line["action"].as_str() {
                    let at = span.step_tokens.entry(action.to_owned()).or_insert(Some(0));
                    match at {
                        Some(_) if unknown => *at = None,
                        Some(sum) => count(sum, tokens, &mut span.saturated),
                        None => {}
                    }
                }
            }
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
        "tokens" if !case.unknown_steps.is_empty() => "unknown".to_owned(),
        "tokens" => case.tokens.to_string(),
        "refusals" => case.refusals.to_string(),
        "no_progress" => case.no_progress.to_string(),
        "errors" => case.errors.to_string(),
        "wall" => format!("{}.{:03}", case.wall_ms / 1000, case.wall_ms % 1000),
        _ => "-".to_owned(),
    }
}

/// A metric's figure of `case` as a number, a scenario's median is taken over: an outcome by
/// its rank, wall time in milliseconds; none for a metric with no figure a case, `step_tokens`
/// (by action) or one this function does not name, never another metric's.
fn number(case: &Case, metric: &str) -> Option<u64> {
    match metric {
        "outcome" => Some(u64::from(rank(&case.outcome))),
        "steps" => Some(case.steps),
        "tokens" => Some(case.tokens),
        "refusals" => Some(case.refusals),
        "no_progress" => Some(case.no_progress),
        "errors" => Some(case.errors),
        "wall" => Some(case.wall_ms),
        _ => None,
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

/// Refuses a comparison of `sides`, each a scorecard with the name a refusal gives it, when a
/// scenario case of any of them has unknown run tokens ([`Row::tokens`]), naming each such case
/// and step as `<case> step <action> (<side>)`: a harness step that recorded no tokens is no
/// saving, and nothing is judged on it. A case of no scenario is compared with nothing and
/// refuses nothing.
pub fn refuse_unknown_tokens(sides: &[(&Scorecard, &str)]) -> Result<()> {
    let mut unknown: Vec<String> = Vec::new();
    for (card, side) in sides {
        for case in card.cases() {
            if case.scenario.is_some() {
                for step in &case.unknown_steps {
                    unknown.push(format!("{} step {step} ({side})", case.case));
                }
            }
        }
    }
    if !unknown.is_empty() {
        bail!(
            "the run tokens are unknown in {}: a harness step of it recorded no tokens, or 0, and a missing record is never read as 0, which would read as a saving, so nothing is judged",
            unknown.join(", ")
        );
    }
    Ok(())
}

/// The one-sided exact Mann-Whitney U test of a targeted step: whether the candidate's tokens at
/// the step are stochastically below the saved side's. `u` counts the pairs of a candidate case
/// and a saved case in which the candidate's is not below (a tie counts against the candidate,
/// so ties never help it); `at_most` is the number of the `total` equally likely arrangements of
/// the two sides, under equality and without ties, whose U is at most `u`; and `rejects` is
/// whether `at_most / total` is at most [`RANK_TEST_ALPHA`], compared in integers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RankTest {
    pub u: u64,
    pub at_most: u128,
    pub total: u128,
    pub rejects: bool,
}

/// The rank test of `candidate` against `saved`, each a side's tokens at one step, a figure a
/// case; none when a side has fewer than [`RANK_TEST_CASES`] cases, or so many that the count of
/// arrangements does not fit 128 bits.
///
/// Why this test: the tokens of a step are not normal (a step takes one request or three, so the
/// figures cluster with gaps), the sides are small and independent, and the question is one-sided
/// (cheaper, not different). The exact Mann-Whitney U test assumes nothing of the distribution,
/// is exact at five a side where a normal approximation is not, and counts in integers, so its
/// verdict does not depend on rounding. At five a side it rejects at U of at most 4 (12 of 252
/// arrangements, 0.048), not at 5 (19 of 252, 0.075).
#[must_use]
pub fn rank_test(candidate: &[u64], saved: &[u64]) -> Option<RankTest> {
    let (n, m) = (candidate.len(), saved.len());
    if n < RANK_TEST_CASES || m < RANK_TEST_CASES {
        return None;
    }
    let u = candidate
        .iter()
        .map(|c| saved.iter().filter(|s| c >= s).count() as u64)
        .sum::<u64>();
    // ways[i][j][k]: the arrangements of i candidate and j saved cases with U = k. The largest
    // of them is a candidate case, which is above all j saved ones, or a saved case, which adds
    // nothing.
    let mut ways: Vec<Vec<Vec<u128>>> = vec![vec![Vec::new(); m + 1]; n + 1];
    for i in 0..=n {
        for j in 0..=m {
            let mut row = vec![0_u128; i * j + 1];
            if i == 0 || j == 0 {
                row[0] = 1;
            } else {
                for (k, slot) in row.iter_mut().enumerate() {
                    let above = k
                        .checked_sub(j)
                        .and_then(|k| ways[i - 1][j].get(k))
                        .copied()
                        .unwrap_or(0);
                    let below = ways[i][j - 1].get(k).copied().unwrap_or(0);
                    *slot = above.checked_add(below)?;
                }
            }
            ways[i][j] = row;
        }
    }
    let all = &ways[n][m];
    let total = all
        .iter()
        .try_fold(0_u128, |sum, ways| sum.checked_add(*ways))?;
    let reach = usize::try_from(u).unwrap_or(usize::MAX).min(all.len() - 1);
    let at_most = all[..=reach]
        .iter()
        .try_fold(0_u128, |sum, ways| sum.checked_add(*ways))?;
    let (numerator, denominator) = RANK_TEST_ALPHA;
    let rejects = at_most.checked_mul(denominator)? <= total.checked_mul(numerator)?;
    Some(RankTest {
        u,
        at_most,
        total,
        rejects,
    })
}

/// One targeted step of one scenario, as [`Scorecard::judge_steps`] judged it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetedStep {
    pub scenario: String,
    pub action: String,
    /// Each candidate case that took the step, with its tokens at it, in case order.
    pub candidate: Vec<(String, u64)>,
    /// Each saved case that took the step, with its tokens at it, in case order.
    pub saved: Vec<(String, u64)>,
    /// The saved side's cheapest case at the step; none when no saved case took it.
    pub saved_cheapest: Option<u64>,
    /// Every candidate case took the step and is below `saved_cheapest`.
    pub every_below: bool,
    /// The rank test, with five cases a side or more ([`rank_test`]).
    pub rank: Option<RankTest>,
    /// The cases, either side's, that took no step of the action, which no figure stands for.
    pub missing: Vec<String>,
    /// No case is missing, and every candidate case is below or the rank test rejects equality.
    pub better: bool,
}

/// The verdict of [`Scorecard::judge_steps`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepVerdict {
    /// Better on every targeted step in every scenario that has it, and no scenario worse on the
    /// run.
    Better,
    /// No scenario worse on the run, but some targeted step not better, or none judged.
    NotBetter,
    /// Some scenario worse on the run, whatever the steps: refused.
    WorseOnTheRun,
}

/// What [`Scorecard::judge_steps`] found: each targeted step of each scenario, the scenarios
/// worse on the run, and the verdict.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Judgement {
    pub steps: Vec<TargetedStep>,
    pub worse: Vec<String>,
    pub verdict: StepVerdict,
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

    /// Each scenario's `DiskLow` trials: the cases of it any run of which stopped `DiskLow`
    /// ([`DISK_LOW`]), by case id; a scenario with none is not listed.
    #[must_use]
    pub fn disk_low(&self) -> BTreeMap<String, BTreeSet<String>> {
        let mut low: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for row in self.rows.iter().filter(|row| row.stop == DISK_LOW) {
            if let Some(scenario) = &row.scenario {
                low.entry(scenario.clone())
                    .or_default()
                    .insert(row.case.clone());
            }
        }
        low
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
        // The layers of the runs scored (ADR 0101): each distinct set once, with its runs'
        // count, in the order the sets first appear; a run without `layers` is not recorded.
        let mut sets: Vec<(String, u64)> = Vec::new();
        for row in &self.rows {
            let text = layers_text(row.layers.as_ref());
            match sets.iter_mut().find(|(set, _)| *set == text) {
                Some((_, n)) => *n += 1,
                None => sets.push((text, 1)),
            }
        }
        for (set, n) in sets {
            writeln!(
                out,
                "layers  {set}  ({n} run{})",
                if n == 1 { "" } else { "s" }
            )?;
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
            // Under the tokens definition 1 a run summed an untokened step as 0 beside a null step:
            // that copy reads, and the definition check refuses to compare it until a rebaseline.
            if card
                .metrics
                .get("tokens")
                .is_some_and(|version| *version >= 2)
                && let Some(tokens) = row.tokens
                && let Some((action, _)) = row.step_tokens.iter().find(|(_, at)| at.is_none())
            {
                bail!(
                    "{}: run {}#{n} gives the tokens {tokens}, but its step {action} recorded none; a run's tokens are unknown when a step's are, so the scorecard is not read",
                    path.display(),
                    row.case
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
    /// of every metric whose definition's version differs, none for one with no figure a case
    /// (`step_tokens`, by action); each outcome rank at which the
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
    /// error naming each such scenario, as a promotion refuses one. The corpus is every scenario
    /// either side has: a scenario the saved scorecard lacks (`not in the saved scorecard`) gets
    /// no verdict and the comparison is an error naming it, as is one this side lacks (none of
    /// its cases, too few); with no scenario case on either side there is no corpus, and the
    /// comparison is an error before any scenario. A scenario with a `DiskLow` trial on either
    /// side ([`Scorecard::disk_low`]) gets no verdict whatever its count, printed `no verdict:
    /// DiskLow trials (...)` naming them, and the error names them too: the scenario is run
    /// again as a fresh set without them, not topped up; a side that has no case of such a
    /// scenario is named as well, as above, so neither refusal hides the other. A case a run of
    /// which disagrees with its held-out check is named so beside its outcome. A scenario case
    /// of either side whose run tokens are unknown refuses the comparison before anything is
    /// printed, naming the case and the step ([`refuse_unknown_tokens`]). The number of
    /// scenarios marked worse.
    pub fn against(&self, saved: &Scorecard, out: &mut dyn Write) -> Result<usize> {
        Ok(self.worse_against(saved, out)?.len())
    }

    /// [`Scorecard::against`], the scenarios marked worse by name, in order, rather than their
    /// number.
    pub fn worse_against(&self, saved: &Scorecard, out: &mut dyn Write) -> Result<Vec<String>> {
        refuse_unknown_tokens(&[(self, "this scorecard"), (saved, "the saved scorecard")])?;
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
        if scenarios.is_empty() {
            bail!(
                "no scenario case on either side, so there is no corpus to compare; a comparison covers every scenario either side has and judges nothing on none"
            );
        }
        let mut worse: Vec<String> = Vec::new();
        let mut too_few: Vec<String> = Vec::new();
        let mut unsaved: Vec<String> = Vec::new();
        let (low, saved_low) = (self.disk_low(), saved.disk_low());
        let mut disk_low: Vec<String> = Vec::new();
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
            // A `DiskLow` trial on either side measured the machine, not takt: no verdict,
            // whatever the count, and no top-up clears it (the planner's ruling).
            let named: Vec<String> = [
                ("the candidate's", low.get(scenario)),
                ("the saved side's", saved_low.get(scenario)),
            ]
            .into_iter()
            .filter_map(|(side, cases)| {
                cases.map(|cases| {
                    format!(
                        "{side} {}",
                        cases
                            .iter()
                            .map(String::as_str)
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                })
            })
            .collect();
            if !named.is_empty() {
                writeln!(
                    out,
                    "scenario {scenario}  no verdict: DiskLow trials ({})",
                    named.join("; ")
                )?;
                disk_low.push(format!("scenario {scenario} ({})", named.join("; ")));
                // A side with no case of the scenario is a gap in the corpus that no fresh
                // set on the other side closes, so it is named too; a side's count short of the
                // trials is not, the fresh set being run anyway.
                if baseline.is_empty() {
                    writeln!(out, "scenario {scenario}  not in the saved scorecard")?;
                    unsaved.push(format!("scenario {scenario}"));
                }
                if candidate.is_empty() {
                    writeln!(out, "scenario {scenario}  too few cases (0 of {trials})")?;
                    too_few.push(format!("scenario {scenario} (0 of {trials})"));
                }
                continue;
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
                writeln!(out, "scenario {scenario}  not in the saved scorecard")?;
                unsaved.push(format!("scenario {scenario}"));
                continue;
            }
            // A metric's median over `cases`; none for a metric with no figure a case.
            let median = |cases: &[Case], metric: &str| {
                cases
                    .iter()
                    .map(|case| number(case, metric))
                    .collect::<Option<Vec<_>>>()
                    .map(|figures| median2(&figures))
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
            let tokens_median =
                |cases: &[Case]| median2(&cases.iter().map(|case| case.tokens).collect::<Vec<_>>());
            let (saved_tokens, tokens) = (tokens_median(&baseline), tokens_median(&candidate));
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
                // A metric with no figure a case (`step_tokens`, by action) has no median and
                // prints none, never another metric's figures under its name.
                if metric != "tokens"
                    && changed(metric)
                    && let (Some(was), Some(is)) =
                        (median(&baseline, metric), median(&candidate, metric))
                {
                    writeln!(
                        out,
                        "scenario {scenario}  median {metric} {} {} {} {}",
                        version(saved, metric),
                        half(was),
                        version(self, metric),
                        half(is)
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
                worse.push(scenario.to_owned());
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
        let mut refusals: Vec<String> = Vec::new();
        if !too_few.is_empty() {
            refusals.push(format!(
                "too few cases for a verdict: {}; the rule needs {trials} cases a scenario on each side, so run the short side again before comparing",
                too_few.join(", ")
            ));
        }
        if !unsaved.is_empty() {
            refusals.push(format!(
                "not in the saved scorecard: {}; a comparison covers every scenario either side has, and a promotion's baseline gains a scenario only through `takt promote --rebaseline`",
                unsaved.join(", ")
            ));
        }
        if !disk_low.is_empty() {
            refusals.push(format!(
                "a DiskLow trial gives its scenario no verdict: {}; disk fullness measures the machine, not takt, so run each such scenario again as a fresh set without them, not topped up",
                disk_low.join(", ")
            ));
        }
        if !refusals.is_empty() {
            bail!("{}", refusals.join("; "));
        }
        Ok(worse)
    }

    /// Judges this scorecard as a candidate that targets the steps of `actions` (the actions its
    /// finding names), against `saved`, under the `step_tokens` rule. The whole run first, as
    /// [`Scorecard::worse_against`] judges and prints it, its refusals (too few cases, a
    /// scenario one side lacks, a `DiskLow` trial) refusing this too. Then, per scenario and
    /// action, each side's tokens at the step, a figure a case, the saved side's cheapest, and
    /// the [`rank_test`]: the step is better when every candidate case is below the saved
    /// cheapest, or the rank test rejects equality; a case of either side that took no step of
    /// the action is named and the step is not better. A scenario neither side took the action
    /// in is printed and not judged; an action no scenario took is not better. The verdict
    /// ([`StepVerdict`]) is printed last: better on its steps and not worse on the run; not
    /// better; or, a scenario worse on the run, refused naming both the steps' result and the
    /// scenarios. Both sides must be under this takt's `step_tokens` definition, else refused
    /// naming it: a scorecard saved without it has no figure a step to judge. A scenario case of
    /// either side whose tokens at a targeted step are unknown (a harness step of it recorded no
    /// tokens, [`Row::step_tokens`]) refuses the judgement before anything is printed, naming
    /// the case and the step: a missing record read as 0 would read as a saving. So does one
    /// whose run tokens are unknown at any step, targeted or not: the run is not judged, and
    /// with it not the steps ([`refuse_unknown_tokens`]).
    pub fn judge_steps(
        &self,
        saved: &Scorecard,
        actions: &[String],
        out: &mut dyn Write,
    ) -> Result<Judgement> {
        let mut targeted: Vec<&str> = Vec::new();
        for action in actions {
            let action = action.trim();
            if action.is_empty() {
                bail!(
                    "a targeted step is named by its action (`change.handover`); an empty one names none"
                );
            }
            if !targeted.contains(&action) {
                targeted.push(action);
            }
        }
        if targeted.is_empty() {
            bail!("no targeted step: name the actions the candidate's finding names");
        }
        let known = METRICS
            .iter()
            .find(|(metric, _)| *metric == "step_tokens")
            .map_or(0, |(_, version)| *version);
        for (card, side) in [(self, "this scorecard"), (saved, "the saved scorecard")] {
            match card.metrics.get("step_tokens") {
                Some(version) if *version == known => {}
                Some(version) => bail!(
                    "{side} is under step_tokens definition version {version}; this takt judges targeted steps under version {known}, and a scorecard under another is not judged"
                ),
                None => bail!(
                    "{side} has no step_tokens definition: it was saved without tokens by step, so its steps cannot be judged; save it again with this takt"
                ),
            }
        }
        let (candidates, baselines) = (self.cases(), saved.cases());
        let mut unknown: Vec<String> = Vec::new();
        for (cases, side) in [
            (&candidates, "this scorecard"),
            (&baselines, "the saved scorecard"),
        ] {
            for case in cases.iter().filter(|case| case.scenario.is_some()) {
                for &action in &targeted {
                    if case.step_tokens.get(action) == Some(&None) {
                        unknown.push(format!("{} step {action} ({side})", case.case));
                    }
                }
            }
        }
        if !unknown.is_empty() {
            bail!(
                "the tokens of a targeted step are unknown in {}: a harness step of it recorded no tokens, and a missing record is never read as 0, so the steps are not judged",
                unknown.join(", ")
            );
        }
        let worse = self.worse_against(saved, out)?;
        writeln!(
            out,
            "rule  step_tokens v{known}  a targeted step is better when every candidate case's tokens at it are below the saved side's cheapest, or a one-sided exact Mann-Whitney U test (a tie counts against the candidate) rejects equality at p <= {}/{} with {RANK_TEST_CASES} cases a side or more",
            RANK_TEST_ALPHA.0, RANK_TEST_ALPHA.1
        )?;
        let scenarios: BTreeSet<&str> = candidates
            .iter()
            .chain(&baselines)
            .filter_map(|case| case.scenario.as_deref())
            .collect();
        let mut steps: Vec<TargetedStep> = Vec::new();
        for scenario in scenarios {
            for &action in &targeted {
                let took = |cases: &[Case]| {
                    let (mut took, mut missing) = (Vec::new(), Vec::new());
                    for case in cases
                        .iter()
                        .filter(|case| case.scenario.as_deref() == Some(scenario))
                    {
                        // An unknown was refused above; none reaches here.
                        match case.step_tokens.get(action) {
                            Some(Some(tokens)) => took.push((case.case.clone(), *tokens)),
                            _ => missing.push(case.case.clone()),
                        }
                    }
                    (took, missing)
                };
                let ((candidate, mut missing), (saved_took, saved_missing)) =
                    (took(&candidates), took(&baselines));
                if candidate.is_empty() && saved_took.is_empty() {
                    writeln!(
                        out,
                        "scenario {scenario}  step {action}  taken by no case on either side, not judged"
                    )?;
                    continue;
                }
                missing.extend(saved_missing);
                let figures = |took: &[(String, u64)]| -> Vec<u64> {
                    took.iter().map(|(_, tokens)| *tokens).collect()
                };
                let (mine, theirs) = (figures(&candidate), figures(&saved_took));
                let saved_cheapest = theirs.iter().min().copied();
                let every_below = !mine.is_empty()
                    && saved_cheapest.is_some_and(|cheapest| mine.iter().all(|t| *t < cheapest));
                let rank = rank_test(&mine, &theirs);
                let better =
                    missing.is_empty() && (every_below || rank.is_some_and(|rank| rank.rejects));
                let list = |figures: &[u64]| {
                    figures
                        .iter()
                        .map(u64::to_string)
                        .collect::<Vec<_>>()
                        .join(" ")
                };
                writeln!(
                    out,
                    "scenario {scenario}  step {action}  candidate {}  saved {}  saved cheapest {}",
                    list(&mine),
                    list(&theirs),
                    saved_cheapest.map_or_else(|| "-".to_owned(), |cheapest| cheapest.to_string())
                )?;
                let tested = match rank {
                    Some(rank) => format!(
                        "U {}, {} of {} arrangements at U {} or less, {} equality at p <= {}/{}",
                        rank.u,
                        rank.at_most,
                        rank.total,
                        rank.u,
                        if rank.rejects {
                            "rejects"
                        } else {
                            "does not reject"
                        },
                        RANK_TEST_ALPHA.0,
                        RANK_TEST_ALPHA.1
                    ),
                    None => format!("no rank test (fewer than {RANK_TEST_CASES} cases a side)"),
                };
                writeln!(
                    out,
                    "scenario {scenario}  step {action}  every candidate case below: {}  {tested}{}  {}",
                    if every_below { "yes" } else { "no" },
                    if missing.is_empty() {
                        String::new()
                    } else {
                        format!("  no {action} step in {}", missing.join(", "))
                    },
                    if better { "better" } else { "not better" }
                )?;
                steps.push(TargetedStep {
                    scenario: scenario.to_owned(),
                    action: action.to_owned(),
                    candidate,
                    saved: saved_took,
                    saved_cheapest,
                    every_below,
                    rank,
                    missing,
                    better,
                });
            }
        }
        let not_better: Vec<&str> = targeted
            .iter()
            .copied()
            .filter(|action| {
                let judged: Vec<&TargetedStep> =
                    steps.iter().filter(|step| step.action == *action).collect();
                judged.is_empty() || judged.iter().any(|step| !step.better)
            })
            .collect();
        let better_on_steps = not_better.is_empty();
        let named = targeted.join(", ");
        let steps_said = if better_on_steps {
            format!("better on its steps ({named})")
        } else {
            format!(
                "not better on its steps ({named}; not better: {})",
                not_better.join(", ")
            )
        };
        let verdict = if !worse.is_empty() {
            let scenarios = worse
                .iter()
                .map(|scenario| format!("scenario {scenario}"))
                .collect::<Vec<_>>()
                .join(", ");
            writeln!(
                out,
                "verdict  refused: {steps_said}, but worse on the run ({scenarios})"
            )?;
            StepVerdict::WorseOnTheRun
        } else if better_on_steps {
            writeln!(out, "verdict  {steps_said}, not worse on the run")?;
            StepVerdict::Better
        } else {
            writeln!(out, "verdict  {steps_said}, not worse on the run")?;
            StepVerdict::NotBetter
        };
        Ok(Judgement {
            steps,
            worse,
            verdict,
        })
    }
}

/// What `takt scorecard` found: the scenarios marked worse, and with targeted steps, the verdict
/// on them ([`Scorecard::judge_steps`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reported {
    pub worse: usize,
    pub steps: Option<StepVerdict>,
}

/// `takt scorecard`: prints the scorecard of `repo`'s runs; with `save`, saves it there, named
/// by `revision`, the revision of takt under test, which a save needs; with `against`,
/// compares it with that saved scorecard, and with `steps` too, the actions a candidate targets,
/// judges it on them ([`Scorecard::judge_steps`]), which needs `against`. Runs whose layers
/// differ ([`layers_differ`]) are refused before anything is printed or saved, naming the
/// layer and the runs, unless `across_layers` asks to compare across them (ADR 0101).
pub fn report(
    repo: &Path,
    save: Option<&Path>,
    revision: Option<&str>,
    against: Option<&Path>,
    steps: &[String],
    across_layers: bool,
    out: &mut dyn Write,
) -> Result<Reported> {
    if !steps.is_empty() && against.is_none() {
        bail!(
            "--steps needs --against <saved scorecard>: a targeted step is judged against the saved side's"
        );
    }
    let saved = against.map(Scorecard::load).transpose()?;
    let card = Scorecard::read(repo)?;
    if !across_layers && let Some(difference) = layers_differ(&card.rows) {
        bail!("{difference}");
    }
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
        Some(saved) if !steps.is_empty() => {
            let judged = card.judge_steps(&saved, steps, out)?;
            Ok(Reported {
                worse: judged.worse.len(),
                steps: Some(judged.verdict),
            })
        }
        Some(saved) => Ok(Reported {
            worse: card.against(&saved, out)?,
            steps: None,
        }),
        None => Ok(Reported {
            worse: 0,
            steps: None,
        }),
    }
}
