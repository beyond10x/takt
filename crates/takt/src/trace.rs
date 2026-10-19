//! The trace: a case's run record (`events.jsonl`, [`crate::events`]) rendered as one
//! `takt-trace/1` file, JSON Lines: a header line on the run, then one line per operation in
//! the one vocabulary every renderer reads, whatever harness ran. An operation is a step, a
//! wait on the forge, a refusal, a stop, or the resume after one. Each names its actor
//! ([`Actor`]: `harness`, `validator`, `forge`, `takt`, `human`, `idle`), its intent
//! ([`Intent`]: `inspect`, `change`, `verify`, `research`, `wait`, `coordinate`) and its kind
//! ([`Kind`], the journey vocabulary plus `tool` for a step's inner tool call once one is
//! recorded). Times are seconds from the run's first instant: a step runs from the line before
//! it to its own line; a wait from its line to the next; a stop is an instant, and the resume
//! after it spans the gap. Token and cost figures come from the raw usage the harness reported
//! and from nothing else: a figure no source states is absent, never 0. Summaries and command
//! text are classified, never copied, so a trace carries no local path and can be published.
//! A record written before takt recorded the model, cost, turns and durations per call is
//! completed from a runner shim's `steps.jsonl` when one is given (one `claude --output-format
//! json` result per harness call, with `started_at` and `recorded_at`), and the header's notes
//! say which fields it supplied.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::Path;

use anyhow::{Context as _, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::events::{EVENTS_FILE, Events, TokenUsage};
use crate::scorecard::{instant, names_no_revision, outcome_of, scenario_of};

/// The trace's format.
pub const FORMAT: &str = "takt-trace/1";

/// The arm a takt run belongs to.
pub const ARM: &str = "takt";

/// Who performed an operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Actor {
    /// The judgement actor: one model call.
    Harness,
    /// A deterministic step on the tree: the tests, the gates.
    Validator,
    /// A deterministic step on the forge: the pull request, its checks, the merge.
    Forge,
    /// The kernel itself: a refusal, a stop, and the remainder of the wall time (commits,
    /// decisions, process starts, the gaps between steps).
    Takt,
    /// A person: the time from a stop to the resume after it.
    Human,
    /// A fixed sleep on the forge (`waiting`).
    Idle,
}

impl Actor {
    /// Every actor, in the order a report lists them.
    pub const ALL: [Actor; 6] = [
        Actor::Harness,
        Actor::Validator,
        Actor::Forge,
        Actor::Idle,
        Actor::Human,
        Actor::Takt,
    ];

    /// The actor's name as the trace writes it.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Actor::Harness => "harness",
            Actor::Validator => "validator",
            Actor::Forge => "forge",
            Actor::Takt => "takt",
            Actor::Human => "human",
            Actor::Idle => "idle",
        }
    }
}

/// What an operation was for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Intent {
    Inspect,
    Change,
    Verify,
    Research,
    Wait,
    Coordinate,
}

/// What an operation was, in the journey vocabulary, plus `tool`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    ModelCall,
    Read,
    Edit,
    Test,
    Gate,
    Push,
    PrOpen,
    CiWait,
    CiPass,
    ApprovalWait,
    Refusal,
    NoProgress,
    Resume,
    Other,
    Tool,
}

impl Kind {
    /// The kind's name as the trace writes it.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Kind::ModelCall => "model_call",
            Kind::Read => "read",
            Kind::Edit => "edit",
            Kind::Test => "test",
            Kind::Gate => "gate",
            Kind::Push => "push",
            Kind::PrOpen => "pr_open",
            Kind::CiWait => "ci_wait",
            Kind::CiPass => "ci_pass",
            Kind::ApprovalWait => "approval_wait",
            Kind::Refusal => "refusal",
            Kind::NoProgress => "no_progress",
            Kind::Resume => "resume",
            Kind::Other => "other",
            Kind::Tool => "tool",
        }
    }
}

/// What an operation's outcome rests on: a record or an exit status a validator observed, or
/// the model's own text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Evidence {
    Observed,
    Declared,
}

/// The tokens of one model call by component, as the harness reported them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpTokens {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
}

impl OpTokens {
    fn from_usage(usage: TokenUsage) -> Self {
        Self {
            input: usage.input,
            output: usage.output,
            cache_read: usage.cache_read,
            cache_write: usage.cache_creation,
        }
    }

    /// The four components summed, at most `u64::MAX`.
    #[must_use]
    pub fn sum(&self) -> u64 {
        self.input
            .saturating_add(self.output)
            .saturating_add(self.cache_read)
            .saturating_add(self.cache_write)
    }
}

/// One operation of a run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Op {
    /// The operation's number in the trace, from 1.
    pub seq: usize,
    /// The step the operation belongs to, 0 before a run's first.
    pub step: u64,
    /// The operation this one happened inside (a tool call inside a model call); none yet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<usize>,
    pub actor: Actor,
    pub intent: Intent,
    pub kind: Kind,
    /// The protocol action, or the actions a wait observed again, joined by `,`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    /// The validator's or the harness's name; who refused, for a refusal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// The class of a tool call; none yet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_class: Option<String>,
    /// Seconds from the run's first instant, to the millisecond; none where the record has no
    /// instant.
    pub t_start: Option<f64>,
    pub t_end: Option<f64>,
    /// The call's tokens by component, when the source states them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens: Option<OpTokens>,
    /// The call's tokens summed: the step's `tokens`, or the components' sum.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens_sum: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_usd: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turns: Option<u64>,
    /// The call's duration as the harness reported it, in milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    /// The call's time in the API as the harness reported it, in milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_ms: Option<u64>,
    /// `pass`, `fail`, `pending`, `opened`, `pushed`, `merged`, `changed`, `unchanged`,
    /// `nothing`, `refused`, `approval`, `error`; a stop's, its outcome as the scorecard names
    /// it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outcome: Option<String>,
    /// The paths a judgement step committed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub paths: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<Evidence>,
    /// A pattern mark: `approval-only call`, `seed pr.open`, `2nd CI`, `inspect, then
    /// NoProgress`, `repeated failure (as step N)`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

impl Op {
    /// The seconds the operation spanned, to the millisecond, when both ends are known.
    #[must_use]
    pub fn seconds(&self) -> Option<f64> {
        Some(round_ms(self.t_end? - self.t_start?))
    }

    /// Whether the operation is a step a validator, the forge or the harness took.
    #[must_use]
    pub fn is_step(&self) -> bool {
        matches!(self.actor, Actor::Harness | Actor::Validator | Actor::Forge)
    }
}

/// The run's tokens: the components summed over its model calls when every call states them,
/// and the sum of the calls' tokens (each the step's `tokens`, else its components' sum) when
/// every call states one; a sum some call cannot join is absent, never a smaller number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Tokens {
    pub input: Option<u64>,
    pub output: Option<u64>,
    pub cache_read: Option<u64>,
    pub cache_write: Option<u64>,
    pub sum: Option<u64>,
}

/// The trace's first line: the run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Header {
    /// [`FORMAT`].
    pub format: String,
    /// The run's name: the case id, or the name given.
    pub run: String,
    /// The case id.
    pub case: String,
    /// [`ARM`].
    pub arm: String,
    /// The harness's name, from its steps; none when no harness step ran.
    pub harness: Option<String>,
    /// The models the calls reported, distinct, in order, joined by `, `; none when none did.
    pub model: Option<String>,
    /// takt's revision, when the binary named one.
    pub revision: Option<String>,
    /// The binary as `opened` named it, as `takt --version` prints it.
    pub binary: Option<String>,
    pub protocol: Option<String>,
    pub story: Option<String>,
    /// The scenario, from a case id `scenario-<id>-trial-<n>`.
    pub scenario: Option<String>,
    /// The first `opened` line's instant and the last line's, verbatim.
    pub t0: Option<String>,
    pub t_end: Option<String>,
    /// From `t0` to `t_end`, in seconds to the millisecond.
    pub wall_s: Option<f64>,
    /// The last stop, verbatim.
    pub stop: Option<String>,
    /// The last run's outcome as the scorecard names it.
    pub outcome: String,
    /// The `opened` lines, and how many of them resumed the case.
    pub invocations: u64,
    pub resumes: u64,
    /// The steps taken, every run's summed.
    pub steps: u64,
    pub tokens: Tokens,
    /// The calls' cost summed, when every call states one.
    pub cost_usd: Option<f64>,
    /// The held-out check; never in the record.
    pub acceptance: Option<String>,
    /// The files the trace was read from, each relative to the case directory, or its file
    /// name alone when it lies elsewhere; never an absolute path.
    pub sources: Vec<String>,
    /// What the steps file supplied, and what did not match.
    pub notes: Vec<String>,
}

/// One line of the file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Line {
    Header(Box<Header>),
    Op(Box<Op>),
}

/// One run's trace.
#[derive(Debug, Clone, PartialEq)]
pub struct Trace {
    pub header: Header,
    pub ops: Vec<Op>,
}

/// `nanos` as seconds, to the millisecond.
fn seconds(nanos: i128) -> f64 {
    #[allow(clippy::cast_precision_loss)]
    let ms = (nanos / 1_000_000) as f64;
    ms / 1000.0
}

/// `value` to the millisecond.
fn round_ms(value: f64) -> f64 {
    (value * 1000.0).round() / 1000.0
}

/// `path` as the trace names a source: relative to the case directory when it lies under it
/// (`events.jsonl`, `steps.jsonl`), else its file name alone; never an absolute path and
/// never one under `~`, so a trace carries no local path whatever `HOME` is.
fn source_name(case_dir: &Path, path: &Path) -> String {
    let resolved = |path: &Path| {
        std::fs::canonicalize(path)
            .or_else(|_| std::path::absolute(path))
            .unwrap_or_else(|_| path.to_path_buf())
    };
    let name = |path: &Path| path.to_string_lossy().into_owned();
    match resolved(path).strip_prefix(resolved(case_dir)) {
        Ok(rest) if !rest.as_os_str().is_empty() => name(rest),
        _ => path
            .file_name()
            .map_or_else(|| name(path), |file| name(Path::new(file))),
    }
}

/// `text` as a trace carries it and a report prints it, the one escape every text a record
/// contributes passes: the home directory as `~`, so no local path travels, and each control
/// character (C0, DEL and C1) as `\u{..}`, so a record a harness shaped cannot drive the
/// terminal a report is read on. Passing a text through twice changes nothing.
#[must_use]
pub fn escape(text: &str) -> String {
    let home = std::env::var_os("HOME").map(|home| home.to_string_lossy().into_owned());
    escape_with(text, home.as_deref())
}

/// [`escape`] with the home directory given: each occurrence of `home` that no more of a
/// name follows becomes `~`, so `<home>/x` reads `~/x` and `<home>name` is left as it is.
fn escape_with(text: &str, home: Option<&str>) -> String {
    let home = home
        .map(|home| home.trim_end_matches('/'))
        .filter(|home| !home.is_empty());
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(c) = rest.chars().next() {
        if let Some(home) = home
            && let Some(after) = rest.strip_prefix(home)
            && !after.starts_with(continues_name)
        {
            out.push('~');
            rest = after;
            continue;
        }
        if c.is_control() {
            out.push_str(&format!("\\u{{{:x}}}", u32::from(c)));
        } else {
            out.push(c);
        }
        rest = &rest[c.len_utf8()..];
    }
    out
}

/// Whether `c` continues a path component's name, so a home directory it follows was not
/// the one named.
fn continues_name(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | '-' | '.')
}

/// The four components of a shim line's `usage`, when it states every one of them; a line
/// that states fewer supplies none, never a 0 for the one it lacks.
fn components(usage: &serde_json::Map<String, Value>) -> Option<OpTokens> {
    let component = |key: &str| usage.get(key).and_then(Value::as_u64);
    Some(OpTokens {
        input: component("input_tokens")?,
        output: component("output_tokens")?,
        cache_read: component("cache_read_input_tokens")?,
        cache_write: component("cache_creation_input_tokens")?,
    })
}

/// `summary` without its durations (`7.89s`, `0.007s`, `12ms`), so two failures that differ
/// only in how long the run took compare equal.
fn without_durations(summary: &str) -> String {
    summary
        .split_whitespace()
        .filter(|word| {
            let word = word.trim_end_matches([',', ';', ')', '.']);
            let digits = word.strip_suffix("ms").or_else(|| word.strip_suffix('s'));
            !digits.is_some_and(|digits| {
                !digits.is_empty()
                    && digits
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || byte == b'.')
            })
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// A step's classification from who took it, its action and what it reported.
struct Class {
    actor: Actor,
    intent: Intent,
    kind: Kind,
    outcome: Option<&'static str>,
    evidence: Evidence,
}

fn classify(by: &str, action: &str, summary: &str) -> Class {
    let class = |actor, intent, kind, outcome, evidence| Class {
        actor,
        intent,
        kind,
        outcome,
        evidence,
    };
    if by == "harness" {
        return match action {
            "repository.merge" => class(
                Actor::Harness,
                Intent::Coordinate,
                Kind::ModelCall,
                Some("approval"),
                Evidence::Declared,
            ),
            "repository.inspect" => class(
                Actor::Harness,
                Intent::Inspect,
                Kind::Read,
                Some("unchanged"),
                Evidence::Declared,
            ),
            _ => class(
                Actor::Harness,
                Intent::Change,
                Kind::Edit,
                None,
                Evidence::Declared,
            ),
        };
    }
    let pass = |marker: &str| {
        if summary.contains(marker) {
            Some("pass")
        } else {
            Some("fail")
        }
    };
    match action {
        "tests.run" => class(
            Actor::Validator,
            Intent::Verify,
            Kind::Test,
            pass("exited 0: pass"),
            Evidence::Observed,
        ),
        "gate.run" => class(
            Actor::Validator,
            Intent::Verify,
            Kind::Gate,
            pass(": pass"),
            Evidence::Observed,
        ),
        "pr.open" => {
            let (kind, outcome) = if summary.contains("did not pass") || summary.contains("refused")
            {
                (Kind::Refusal, "refused")
            } else if summary.contains("opened pull request") {
                (Kind::PrOpen, "opened")
            } else if summary.starts_with("pushed") {
                (Kind::Push, "pushed")
            } else {
                (Kind::PrOpen, "nothing")
            };
            class(
                Actor::Forge,
                Intent::Coordinate,
                kind,
                Some(outcome),
                Evidence::Observed,
            )
        }
        "ci.read" => {
            let (intent, kind, outcome) = if summary.contains("pending") {
                (Intent::Wait, Kind::CiWait, "pending")
            } else if summary.contains(": pass") {
                (Intent::Verify, Kind::CiPass, "pass")
            } else if summary.contains(": fail") {
                (Intent::Verify, Kind::Other, "fail")
            } else {
                (Intent::Coordinate, Kind::Other, "nothing")
            };
            class(
                Actor::Forge,
                intent,
                kind,
                Some(outcome),
                Evidence::Observed,
            )
        }
        "repository.merge" => class(
            Actor::Forge,
            Intent::Coordinate,
            Kind::Other,
            Some(if summary.contains("merged pull request") {
                "merged"
            } else {
                "nothing"
            }),
            Evidence::Observed,
        ),
        _ => class(
            Actor::Validator,
            Intent::Verify,
            Kind::Other,
            None,
            Evidence::Observed,
        ),
    }
}

/// An operation being built: the op, its ends as instants, and the summary it was classified
/// from, kept for the pattern marks and never written.
struct Draft {
    op: Op,
    start: Option<i128>,
    end: Option<i128>,
    summary: Option<String>,
}

/// The trace being built from a record's lines.
struct Builder {
    drafts: Vec<Draft>,
    t0: Option<(i128, String)>,
    last: Option<(i128, String)>,
    previous: Option<i128>,
    invocations: u64,
    steps: u64,
    run_steps: u64,
    run_errors: u64,
    case: Option<String>,
    protocol: Option<String>,
    story: Option<String>,
    binary: Option<String>,
    harness: Option<String>,
    stop: Option<String>,
    pending_harness: Option<usize>,
    pending_validator: Option<usize>,
    pending_idle: Option<usize>,
    pending_stop: Option<i128>,
}

impl Builder {
    fn new() -> Self {
        Self {
            drafts: Vec::new(),
            t0: None,
            last: None,
            previous: None,
            invocations: 0,
            steps: 0,
            run_steps: 0,
            run_errors: 0,
            case: None,
            protocol: None,
            story: None,
            binary: None,
            harness: None,
            stop: None,
            pending_harness: None,
            pending_validator: None,
            pending_idle: None,
            pending_stop: None,
        }
    }

    fn push(
        &mut self,
        step: u64,
        actor: Actor,
        intent: Intent,
        kind: Kind,
        start: Option<i128>,
        end: Option<i128>,
    ) -> usize {
        let seq = self.drafts.len() + 1;
        self.drafts.push(Draft {
            op: Op {
                seq,
                step,
                parent: None,
                actor,
                intent,
                kind,
                action: None,
                name: None,
                tool_class: None,
                t_start: None,
                t_end: None,
                tokens: None,
                tokens_sum: None,
                model: None,
                cost_usd: None,
                turns: None,
                duration_ms: None,
                api_ms: None,
                outcome: None,
                paths: Vec::new(),
                evidence: None,
                label: None,
            },
            start,
            end,
            summary: None,
        });
        seq - 1
    }

    /// The text at `key`, escaped ([`escape`]), when the line holds one.
    fn text(line: &Value, key: &str) -> Option<String> {
        line[key].as_str().map(escape)
    }

    fn line(&mut self, line: &Value) {
        let event = line["event"].as_str().unwrap_or("");
        if event == "skipped" {
            return;
        }
        let at_text = line["at"].as_str();
        let at = at_text.and_then(instant);
        let step = line["step"].as_u64().unwrap_or(0);
        if let (Some(index), Some(at)) = (self.pending_idle, at) {
            self.drafts[index].end = Some(at);
            self.pending_idle = None;
        }
        match event {
            "opened" => {
                self.invocations += 1;
                if self.invocations > 1 {
                    self.steps += self.run_steps;
                    self.run_steps = 0;
                    self.run_errors = 0;
                    if let Some(stopped) = self.pending_stop.take() {
                        let index = self.push(
                            step,
                            Actor::Human,
                            Intent::Wait,
                            Kind::Resume,
                            Some(stopped),
                            at,
                        );
                        self.drafts[index].op.evidence = Some(Evidence::Observed);
                    }
                }
                if self.t0.is_none()
                    && let (Some(at), Some(text)) = (at, at_text)
                {
                    self.t0 = Some((at, escape(text)));
                }
                self.case = self.case.take().or_else(|| Self::text(line, "case"));
                self.protocol = self
                    .protocol
                    .take()
                    .or_else(|| Self::text(line, "protocol"));
                self.story = self.story.take().or_else(|| Self::text(line, "story"));
                self.binary = self.binary.take().or_else(|| Self::text(line, "binary"));
                self.pending_harness = None;
                self.pending_validator = None;
            }
            "step" => {
                let by = line["by"].as_str().unwrap_or("");
                let action = line["action"].as_str().unwrap_or("");
                let summary = line["summary"].as_str().unwrap_or("");
                let class = classify(by, action, summary);
                let index = self.push(
                    step,
                    class.actor,
                    class.intent,
                    class.kind,
                    self.previous,
                    at,
                );
                let usage: Option<TokenUsage> = serde_json::from_value(line["usage"].clone()).ok();
                let draft = &mut self.drafts[index];
                draft.summary = Some(summary.to_owned());
                let op = &mut draft.op;
                op.action = Some(escape(action));
                op.name = Self::text(line, "name");
                op.outcome = class.outcome.map(str::to_owned);
                op.evidence = Some(class.evidence);
                op.tokens = usage.map(OpTokens::from_usage);
                op.tokens_sum = line["tokens"]
                    .as_u64()
                    .or_else(|| op.tokens.map(|tokens| tokens.sum()));
                op.model = Self::text(line, "model");
                op.cost_usd = line["cost_usd"].as_f64();
                op.turns = line["turns"].as_u64();
                op.duration_ms = line["duration_ms"].as_u64();
                op.api_ms = line["api_ms"].as_u64();
                self.run_steps = self.run_steps.max(step);
                if by == "harness" {
                    self.harness = self.harness.take().or_else(|| Self::text(line, "name"));
                    self.pending_harness = Some(index);
                    self.pending_validator = None;
                } else {
                    self.pending_validator = Some(index);
                    self.pending_harness = None;
                }
            }
            "effect" => {
                if let Some(index) = self.pending_harness.take() {
                    let op = &mut self.drafts[index].op;
                    let paths: Vec<String> =
                        serde_json::from_value::<Vec<String>>(line["paths"].clone())
                            .unwrap_or_default()
                            .iter()
                            .map(|path| escape(path))
                            .collect();
                    if paths.is_empty() {
                        op.outcome = Some("unchanged".to_owned());
                    } else {
                        op.kind = Kind::Edit;
                        op.intent = Intent::Change;
                        op.outcome = Some("changed".to_owned());
                    }
                    op.paths = paths;
                }
            }
            "evidence" => {
                if let (Some(index), Some(result)) =
                    (self.pending_validator.take(), line["result"].as_str())
                {
                    let op = &mut self.drafts[index].op;
                    if op.kind != Kind::Push {
                        op.outcome =
                            Some(if result == "open" { "opened" } else { result }.to_owned());
                    }
                }
            }
            "waiting" => {
                let on: Vec<String> =
                    serde_json::from_value(line["on"].clone()).unwrap_or_default();
                let kind = if on.iter().any(|action| action == "ci.read") {
                    Kind::CiWait
                } else {
                    Kind::Other
                };
                let index = self.push(step, Actor::Idle, Intent::Wait, kind, at, None);
                let op = &mut self.drafts[index].op;
                op.action = Some(escape(&on.join(",")));
                op.evidence = Some(Evidence::Observed);
                self.pending_idle = Some(index);
            }
            "refusal" => {
                if let Some(index) = self.pending_harness.take() {
                    let op = &mut self.drafts[index].op;
                    if op.outcome.is_none() {
                        op.outcome = Some("refused".to_owned());
                    }
                }
                let index = self.push(step, Actor::Takt, Intent::Coordinate, Kind::Refusal, at, at);
                let op = &mut self.drafts[index].op;
                op.name = Self::text(line, "by");
                op.outcome = Some("refused".to_owned());
                op.evidence = Some(Evidence::Observed);
            }
            "error" => {
                self.run_errors += 1;
                let index = self.push(
                    step,
                    Actor::Takt,
                    Intent::Coordinate,
                    Kind::Other,
                    self.previous,
                    at,
                );
                let op = &mut self.drafts[index].op;
                op.outcome = Some("error".to_owned());
                op.evidence = Some(Evidence::Observed);
            }
            "stop" => {
                let stop = escape(line["stop"].as_str().unwrap_or(""));
                let outcome = outcome_of(Some(&stop), self.run_errors);
                let (actor, intent, kind) = match outcome.as_str() {
                    "ApprovalRequired" => (Actor::Human, Intent::Wait, Kind::ApprovalWait),
                    "NoProgress" => (Actor::Takt, Intent::Coordinate, Kind::NoProgress),
                    // The run lacks a performer (the forge, the gates document): takt's stop,
                    // not a person's wait, which only a grant ends.
                    "Unperformable" => (Actor::Takt, Intent::Coordinate, Kind::Other),
                    _ => (Actor::Takt, Intent::Coordinate, Kind::Other),
                };
                let index = self.push(step, actor, intent, kind, at, at);
                let op = &mut self.drafts[index].op;
                op.outcome = Some(outcome);
                op.evidence = Some(Evidence::Observed);
                self.stop = Some(stop);
                self.pending_stop = at;
                self.pending_harness = None;
                self.pending_validator = None;
            }
            _ => {}
        }
        if let (Some(at), Some(text)) = (at, at_text) {
            self.previous = Some(at);
            self.last = Some((at, escape(text)));
        }
    }

    /// Pattern marks over the built operations.
    fn label(&mut self) {
        let mut edited = false;
        let mut seed_done = false;
        let mut ci_passes = 0;
        let mut last_fail: BTreeMap<String, (u64, String)> = BTreeMap::new();
        let mut last_step: Option<(Actor, Kind)> = None;
        for draft in &mut self.drafts {
            let op = &mut draft.op;
            let (actor, kind, step) = (op.actor, op.kind, op.step);
            let outcome = op.outcome.as_deref().unwrap_or("");
            let action = op.action.as_deref().unwrap_or("");
            let mut label: Option<String> = None;
            if actor == Actor::Harness && kind == Kind::ModelCall && outcome == "approval" {
                label = Some("approval-only call".to_owned());
            }
            if kind == Kind::Edit && outcome == "changed" {
                edited = true;
            }
            if action == "pr.open" && outcome == "nothing" && !edited && !seed_done {
                label = Some("seed pr.open".to_owned());
                seed_done = true;
            }
            if kind == Kind::CiPass {
                ci_passes += 1;
                if ci_passes == 2 {
                    label = Some("2nd CI".to_owned());
                }
            }
            if kind == Kind::NoProgress && last_step == Some((Actor::Harness, Kind::Read)) {
                label = Some("inspect, then NoProgress".to_owned());
            }
            if matches!(actor, Actor::Validator | Actor::Forge) && outcome == "fail" {
                let summary = draft
                    .summary
                    .as_deref()
                    .map(without_durations)
                    .unwrap_or_default();
                if let Some((earlier, same)) = last_fail.get(action)
                    && *same == summary
                {
                    label = Some(format!("repeated failure (as step {earlier})"));
                }
                last_fail.insert(action.to_owned(), (step, summary));
            }
            if op.is_step() {
                last_step = Some((actor, kind));
            }
            if label.is_some() {
                op.label = label;
            }
        }
    }

    /// Completes the harness operations from a runner shim's `steps.jsonl`: one result per
    /// call, paired with the calls by time when every line carries `recorded_at`, else in
    /// order. A field the record already states is kept. The notes say what the file
    /// supplied and what did not pair.
    fn merge_steps(&mut self, steps: &[Value], notes: &mut Vec<String>) {
        let unparsed = steps
            .iter()
            .filter(|line| line.get("unparsed").is_some())
            .count();
        let lines: Vec<&Value> = steps
            .iter()
            .filter(|line| line.get("unparsed").is_none())
            .collect();
        let calls: Vec<usize> = (0..self.drafts.len())
            .filter(|index| self.drafts[*index].op.actor == Actor::Harness)
            .collect();
        let by_time = !lines.is_empty()
            && lines
                .iter()
                .all(|line| line["recorded_at"].as_str().and_then(instant).is_some());
        let slack: i128 = 2_000_000_000;
        let mut used = vec![false; lines.len()];
        let mut pairs: Vec<(usize, usize)> = Vec::new();
        for (n, index) in calls.iter().enumerate() {
            let chosen = if by_time {
                let (start, end) = (self.drafts[*index].start, self.drafts[*index].end);
                (0..lines.len()).find(|j| {
                    if used[*j] {
                        return false;
                    }
                    let recorded = lines[*j]["recorded_at"].as_str().and_then(instant);
                    let started = lines[*j]["started_at"].as_str().and_then(instant);
                    let ends_in_time = end
                        .is_none_or(|end| recorded.is_none_or(|recorded| recorded <= end + slack));
                    let starts_in_time = start
                        .is_none_or(|start| started.is_none_or(|started| started + slack >= start));
                    let not_before = start.is_none_or(|start| {
                        recorded.is_none_or(|recorded| recorded + slack >= start)
                    });
                    ends_in_time && starts_in_time && not_before
                })
            } else {
                (n < lines.len()).then_some(n)
            };
            if let Some(j) = chosen {
                used[j] = true;
                pairs.push((*index, j));
            }
        }
        let mut supplied: BTreeSet<&'static str> = BTreeSet::new();
        for (index, j) in &pairs {
            let line = lines[*j];
            let op = &mut self.drafts[*index].op;
            if op.tokens.is_none()
                && let Some(usage) = line["usage"].as_object()
                && let Some(tokens) = components(usage)
                && tokens.sum() > 0
            {
                op.tokens = Some(tokens);
                supplied.insert("token components");
                if op.tokens_sum.is_none() {
                    op.tokens_sum = Some(tokens.sum());
                    supplied.insert("tokens");
                }
            }
            if op.model.is_none()
                && let Some(models) = line["modelUsage"].as_object()
                && !models.is_empty()
            {
                op.model = Some(escape(
                    &models.keys().cloned().collect::<Vec<_>>().join("+"),
                ));
                supplied.insert("model");
            }
            if op.cost_usd.is_none()
                && let Some(cost) = line["total_cost_usd"].as_f64()
            {
                op.cost_usd = Some(cost);
                supplied.insert("cost_usd");
            }
            if op.turns.is_none()
                && let Some(turns) = line["num_turns"].as_u64()
            {
                op.turns = Some(turns);
                supplied.insert("turns");
            }
            if op.duration_ms.is_none()
                && let Some(duration) = line["duration_ms"].as_u64()
            {
                op.duration_ms = Some(duration);
                supplied.insert("duration_ms");
            }
            if op.api_ms.is_none()
                && let Some(api) = line["duration_api_ms"].as_u64()
            {
                op.api_ms = Some(api);
                supplied.insert("api_ms");
            }
        }
        let order = [
            "model",
            "cost_usd",
            "turns",
            "duration_ms",
            "api_ms",
            "tokens",
            "token components",
        ];
        let fields: Vec<&str> = order
            .iter()
            .copied()
            .filter(|field| supplied.contains(field))
            .collect();
        if fields.is_empty() {
            notes.push(format!(
                "steps file: nothing supplied, {} of {} calls paired",
                pairs.len(),
                calls.len()
            ));
        } else {
            notes.push(format!(
                "steps file: {} for {} of {} calls",
                fields.join(", "),
                pairs.len(),
                calls.len()
            ));
        }
        let unpaired = lines.len() - pairs.len();
        if unpaired > 0 {
            notes.push(format!(
                "steps file: {unpaired} line(s) paired with no harness step"
            ));
        }
        if unparsed > 0 {
            notes.push(format!(
                "steps file: {unparsed} line(s) the shim could not parse, skipped"
            ));
        }
    }

    fn finish(
        mut self,
        case_dir_name: &str,
        run: Option<&str>,
        sources: Vec<String>,
        notes: Vec<String>,
    ) -> Trace {
        self.steps += self.run_steps;
        if let (Some(index), Some((last, _))) = (self.pending_idle, &self.last) {
            self.drafts[index].end = Some(*last);
        }
        let t0 = self.t0.as_ref().map(|(t0, _)| *t0);
        let mut ops: Vec<Op> = Vec::with_capacity(self.drafts.len());
        for draft in self.drafts {
            let mut op = draft.op;
            op.t_start = t0.and_then(|t0| draft.start.map(|start| seconds(start - t0)));
            op.t_end = t0.and_then(|t0| draft.end.map(|end| seconds(end - t0)));
            ops.push(op);
        }
        let calls: Vec<&Op> = ops.iter().filter(|op| op.actor == Actor::Harness).collect();
        let all = |figure: fn(&Op) -> Option<u64>| -> Option<u64> {
            if calls.is_empty() {
                return None;
            }
            calls
                .iter()
                .copied()
                .map(figure)
                .try_fold(0_u64, |sum, value| Some(sum.saturating_add(value?)))
        };
        let tokens = Tokens {
            input: all(|op| op.tokens.map(|tokens| tokens.input)),
            output: all(|op| op.tokens.map(|tokens| tokens.output)),
            cache_read: all(|op| op.tokens.map(|tokens| tokens.cache_read)),
            cache_write: all(|op| op.tokens.map(|tokens| tokens.cache_write)),
            sum: calls
                .iter()
                .map(|op| op.tokens_sum)
                .try_fold(0_u64, |sum, tokens| Some(sum.saturating_add(tokens?))),
        };
        let cost_usd = if calls.is_empty() {
            None
        } else {
            calls
                .iter()
                .map(|op| op.cost_usd)
                .try_fold(0.0, |sum, cost| Some(sum + cost?))
        };
        let mut models: Vec<String> = Vec::new();
        for op in &calls {
            if let Some(model) = &op.model
                && !models.contains(model)
            {
                models.push(model.clone());
            }
        }
        let case = self.case.clone().unwrap_or_else(|| escape(case_dir_name));
        let revision = self
            .binary
            .as_deref()
            .filter(|binary| !names_no_revision(Some(binary)))
            .map(str::to_owned);
        let wall_s = match (&self.t0, &self.last) {
            (Some((t0, _)), Some((last, _))) => Some(seconds(last - t0)),
            _ => None,
        };
        let header = Header {
            format: FORMAT.to_owned(),
            run: run.map_or_else(|| case.clone(), escape),
            scenario: scenario_of(&case).map(str::to_owned),
            case,
            arm: ARM.to_owned(),
            harness: self.harness,
            model: (!models.is_empty()).then(|| models.join(", ")),
            revision,
            binary: self.binary,
            protocol: self.protocol,
            story: self.story,
            t0: self.t0.map(|(_, text)| text),
            t_end: self.last.map(|(_, text)| text),
            wall_s,
            outcome: outcome_of(self.stop.as_deref(), self.run_errors),
            stop: self.stop,
            invocations: self.invocations,
            resumes: self.invocations.saturating_sub(1),
            steps: self.steps,
            tokens,
            cost_usd,
            acceptance: None,
            sources,
            notes,
        };
        Trace { header, ops }
    }
}

/// The lines of a runner shim's `steps.jsonl`, each read as JSON; a line that does not read is
/// an error naming it.
fn read_steps(path: &Path) -> Result<Vec<Value>> {
    let text = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let mut lines = Vec::new();
    for (n, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        lines.push(
            serde_json::from_str(line)
                .with_context(|| format!("line {} of {} does not read", n + 1, path.display()))?,
        );
    }
    Ok(lines)
}

impl Trace {
    /// The trace of the case at `case_dir`, read from its `events.jsonl` and, when given, the
    /// runner shim's `steps` file; named `run`, or by the case id. A case with no record, or
    /// a record or steps file that does not read, is an error naming it.
    pub fn from_case(case_dir: &Path, steps: Option<&Path>, run: Option<&str>) -> Result<Self> {
        let events = case_dir.join(EVENTS_FILE);
        if !events.is_file() {
            bail!(
                "{} has no {EVENTS_FILE}: no run record to trace",
                case_dir.display()
            );
        }
        let lines = Events::new(case_dir).read()?;
        let shim = steps.map(read_steps).transpose()?;
        let mut sources = vec![source_name(case_dir, &events)];
        if let Some(path) = steps {
            sources.push(source_name(case_dir, path));
        }
        let name = case_dir
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        Ok(Self::from_lines(
            &lines,
            shim.as_deref(),
            &name,
            run,
            sources,
        ))
    }

    /// The trace of a record's `lines`, as [`Events::read`] gives them, completed from the
    /// shim's `steps` when given; `case_dir_name` names the case when no `opened` line does.
    #[must_use]
    pub fn from_lines(
        lines: &[Value],
        steps: Option<&[Value]>,
        case_dir_name: &str,
        run: Option<&str>,
        sources: Vec<String>,
    ) -> Self {
        let mut builder = Builder::new();
        for line in lines {
            builder.line(line);
        }
        let mut notes = Vec::new();
        if let Some(steps) = steps {
            builder.merge_steps(steps, &mut notes);
        }
        builder.label();
        builder.finish(case_dir_name, run, sources, notes)
    }

    /// The operations the harness performed: the model calls.
    #[must_use]
    pub fn calls(&self) -> Vec<&Op> {
        self.ops
            .iter()
            .filter(|op| op.actor == Actor::Harness)
            .collect()
    }

    /// Writes the trace: the header line, then one line per operation.
    pub fn write(&self, out: &mut dyn Write) -> Result<()> {
        let mut text = serde_json::to_string(&Line::Header(Box::new(self.header.clone())))
            .context("write the trace header")?;
        text.push('\n');
        for op in &self.ops {
            text.push_str(
                &serde_json::to_string(&Line::Op(Box::new(op.clone())))
                    .context("write an operation")?,
            );
            text.push('\n');
        }
        out.write_all(text.as_bytes()).context("write the trace")
    }

    /// Writes the trace to the file at `path`, replacing one there.
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create {}", parent.display()))?;
        }
        let mut file =
            std::fs::File::create(path).with_context(|| format!("write {}", path.display()))?;
        self.write(&mut file)
            .with_context(|| format!("write {}", path.display()))
    }

    /// A saved trace; one that does not read, or of another format, is an error naming it.
    pub fn load(path: &Path) -> Result<Self> {
        let text =
            std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
        let mut header: Option<Header> = None;
        let mut ops = Vec::new();
        for (n, line) in text.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let parsed = Self::read_line(line).with_context(|| {
                format!(
                    "line {} of {} is not a {FORMAT} line",
                    n + 1,
                    path.display()
                )
            })?;
            match parsed {
                Line::Header(found) => {
                    if header.is_some() {
                        bail!("{} has a second header on line {}", path.display(), n + 1);
                    }
                    if found.format != FORMAT {
                        bail!(
                            "{} is a trace of format {}, not {FORMAT}",
                            path.display(),
                            found.format
                        );
                    }
                    header = Some(*found);
                }
                Line::Op(op) => {
                    if header.is_none() {
                        bail!(
                            "{} has an operation before its header, on line {}",
                            path.display(),
                            n + 1
                        );
                    }
                    ops.push(*op);
                }
            }
        }
        let Some(header) = header else {
            bail!("{} has no {FORMAT} header", path.display());
        };
        Ok(Self { header, ops })
    }

    /// One line of a trace file, by its `type`. Read through a [`Value`] rather than as the
    /// tagged [`Line`]: a tagged enum buffers its fields, and a buffered number does not read
    /// back as an `f64` where `serde_json` keeps numbers exact.
    fn read_line(line: &str) -> Result<Line> {
        let mut value: Value = serde_json::from_str(line)?;
        let kind = value
            .as_object_mut()
            .and_then(|object| object.remove("type"))
            .and_then(|kind| kind.as_str().map(str::to_owned));
        Ok(match kind.as_deref() {
            Some("header") => Line::Header(serde_json::from_value(value)?),
            Some("op") => Line::Op(serde_json::from_value(value)?),
            other => bail!("its type is {other:?}, neither header nor op"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations_are_dropped_before_two_failures_compare() {
        assert_eq!(
            without_durations("FAILED. 6 passed; 2 failed; finished in 7.89s"),
            without_durations("FAILED. 6 passed; 2 failed; finished in 0.12s")
        );
        assert_ne!(
            without_durations("6 passed; 2 failed"),
            without_durations("5 passed; 3 failed")
        );
        assert_eq!(without_durations("Ran 5 tests in 0.007s"), "Ran 5 tests in");
    }

    #[test]
    fn a_harness_step_is_classified_by_its_action() {
        let merge = classify("harness", "repository.merge", "requesting merge approval");
        assert_eq!(merge.kind, Kind::ModelCall);
        assert_eq!(merge.outcome, Some("approval"));
        let inspect = classify("harness", "repository.inspect", "read");
        assert_eq!(
            (inspect.kind, inspect.intent),
            (Kind::Read, Intent::Inspect)
        );
        let tests = classify(
            "validator",
            "tests.run",
            "the test command exited 101: fail",
        );
        assert_eq!(
            (tests.actor, tests.kind, tests.outcome),
            (Actor::Validator, Kind::Test, Some("fail"))
        );
        let ci = classify(
            "validator",
            "ci.read",
            "checks on pull request #4 at abc: pending; no record",
        );
        assert_eq!(
            (ci.actor, ci.kind, ci.intent),
            (Actor::Forge, Kind::CiWait, Intent::Wait)
        );
    }

    #[test]
    fn seconds_round_to_the_millisecond() {
        assert_eq!(seconds(30_003_589_000), 30.003);
        assert_eq!(round_ms(44.000_000_001), 44.0);
    }

    #[test]
    fn every_text_passes_one_escape() {
        assert_eq!(
            escape_with("a\u{1b}[31mb\u{7f}c\u{9b}d", None),
            "a\\u{1b}[31mb\\u{7f}c\\u{9b}d"
        );
        assert_eq!(escape_with("\n\t", None), "\\u{a}\\u{9}");
        assert_eq!(
            escape_with("read /srv/u/x and /srv/user/y", Some("/srv/u")),
            "read ~/x and /srv/user/y"
        );
        assert_eq!(escape_with("/srv/u", Some("/srv/u/")), "~");
        assert_eq!(escape_with("/x", Some("/")), "/x");
        assert_eq!(escape_with("plain", Some("")), "plain");
        assert_eq!(escape_with("plain", None), "plain");
        let once = escape_with("Error (read /srv/u/x \u{1b}[2J)", Some("/srv/u"));
        assert_eq!(
            escape_with(&once, Some("/srv/u")),
            once,
            "escaping twice changes nothing"
        );
    }
}
