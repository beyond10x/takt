//! The run report: where a run's seconds and tokens went, read from its trace
//! ([`crate::trace`]) and from nothing else. The wall time is split by actor: the harness's
//! seconds are the durations it reported for its calls when every call has one, else the
//! calls' spans in the record; a validator's, the forge's, an idle wait's and a person's are
//! their spans; takt's is the remainder (commits, decisions, process starts, the gaps between
//! steps). Printed as markdown: one table across the runs given, then per run the actors'
//! shares, the model calls' tokens and cost, a row per step, the defects (every pattern label,
//! refusal, `NoProgress` and error), what the sources did not record, and which source each
//! figure came from.
//!
//! A figure no source states is `unknown`, never 0: a sum over operations some of which state
//! none prints as `unknown (<stated> over <k> of <n> …)`. Every text a record or a path
//! contributes passes the trace's one escape ([`crate::trace::escape`]) before it is printed:
//! a control character as `\u{..}`, the home directory as `~`.

use std::collections::BTreeMap;
use std::io::Write;

use crate::trace::{Actor, Kind, Op, Trace, escape};

/// What the harness's seconds rest on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Basis {
    /// Every call's duration as the harness reported it, summed.
    Reported,
    /// The calls' spans in the record, from the line before each call to its own: not every
    /// call reported a duration.
    Spans,
}

/// A figure summed over operations, some of which may state none: the sum of those that do,
/// and how many of how many stated one. Complete when every one did; an incomplete sum is
/// never printed as a number.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Sum<T> {
    /// The figures stated, summed.
    pub stated: T,
    /// How many operations stated one.
    pub known: u64,
    /// How many operations the sum is over.
    pub of: u64,
}

impl<T: Copy> Sum<T> {
    /// Whether every operation stated its figure.
    #[must_use]
    pub fn complete(&self) -> bool {
        self.known == self.of
    }

    /// The sum, when every operation stated its figure.
    #[must_use]
    pub fn value(&self) -> Option<T> {
        self.complete().then_some(self.stated)
    }

    /// The sum as the report prints it, `stated` being [`Sum::stated`] formatted: that text
    /// when complete, else `unknown (<stated> over <known> of <of> <noun>s)`.
    fn print(&self, stated: &str, noun: &str) -> String {
        if self.complete() {
            stated.to_owned()
        } else {
            format!(
                "unknown ({stated} over {} of {} {noun}{})",
                self.known,
                self.of,
                plural(self.of)
            )
        }
    }
}

impl Sum<f64> {
    fn add(&mut self, figure: Option<f64>) {
        self.of += 1;
        if let Some(figure) = figure {
            self.known += 1;
            self.stated += figure;
        }
    }
}

impl Sum<u64> {
    fn add(&mut self, figure: Option<u64>) {
        self.of += 1;
        if let Some(figure) = figure {
            self.known += 1;
            self.stated = self.stated.saturating_add(figure);
        }
    }
}

/// One run's attribution.
#[derive(Debug, Clone, PartialEq)]
pub struct Attribution {
    /// The run's wall time, when the record has instants.
    pub wall_s: Option<f64>,
    /// Seconds per actor but takt: the harness's, its calls' reported durations when every
    /// call has one, else the calls' spans; another's, its operations' spans. Complete when
    /// every operation of the actor states its seconds.
    pub seconds: BTreeMap<Actor, Sum<f64>>,
    /// takt's seconds: the wall time less every other actor's, when the wall time is known
    /// and each of those sums is complete.
    pub takt_s: Option<f64>,
    pub basis: Basis,
    /// The calls' time in the API summed, when every call states it.
    pub api_s: Option<f64>,
    pub steps: u64,
    pub model_calls: u64,
    /// The calls' turns summed, when every call states them.
    pub turns: Option<u64>,
    /// The calls' tokens summed, over the calls that state them.
    pub tokens: Sum<u64>,
    /// The `ci.read` steps the run took, every outcome counted.
    pub ci_reads: u64,
    /// Their spans summed, in seconds; the idle waits between them are not inside.
    pub ci_read_s: Sum<f64>,
    /// The idle waits whose actions include `ci.read`, and their seconds summed.
    pub ci_waits: u64,
    pub ci_wait_s: Sum<f64>,
    /// The CI cycles: each `pr.open` step that pushed or opened a pull request and was
    /// followed by a `ci.read` that passed before the next such step.
    pub ci_cycles: u64,
}

impl Attribution {
    /// What a `ci.read` costs the run, in seconds: the reads' spans and the idle waits on
    /// `ci.read` before them, summed, over the reads' count; none without a read, and none
    /// while a read or a wait has no span.
    #[must_use]
    pub fn per_ci_read(&self) -> Option<f64> {
        if self.ci_reads == 0 {
            return None;
        }
        #[allow(clippy::cast_precision_loss)]
        let reads = self.ci_reads as f64;
        Some((self.ci_read_s.value()? + self.ci_wait_s.value()?) / reads)
    }

    /// The seconds of `actor`, when known: another actor's when every operation of it states
    /// its seconds, takt's when the wall time and every other actor's are known.
    #[must_use]
    pub fn of(&self, actor: Actor) -> Option<f64> {
        match actor {
            Actor::Takt => self.takt_s,
            _ => self.seconds.get(&actor).and_then(Sum::value),
        }
    }

    /// The seconds of `actor` as the report prints them, whole: the number; takt's `unknown`
    /// when not known; another actor's `unknown (<stated> over <k> of <n> …)` when an
    /// operation of it states none.
    fn printed(&self, actor: Actor) -> String {
        if actor == Actor::Takt {
            return self.takt_s.map_or_else(|| "unknown".to_owned(), whole);
        }
        let sum = self.seconds.get(&actor).copied().unwrap_or_default();
        sum.print(&whole(sum.stated), noun(actor))
    }
}

/// What the report counts an actor's operations as.
fn noun(actor: Actor) -> &'static str {
    match actor {
        Actor::Harness => "call",
        Actor::Validator | Actor::Forge => "step",
        Actor::Idle | Actor::Human => "wait",
        Actor::Takt => "operation",
    }
}

/// The CI cycles of `ops`: a `pr.open` step that pushed or opened a pull request starts one,
/// a later `ci.read` that passed completes it; a push before the pass replaces it, so the
/// checks a later push superseded are not counted.
fn ci_cycles(ops: &[Op]) -> u64 {
    let mut started = false;
    let mut cycles = 0;
    for op in ops.iter().filter(|op| op.is_step()) {
        match (op.action.as_deref(), op.outcome.as_deref()) {
            (Some("pr.open"), Some("pushed" | "opened")) => started = true,
            (Some("ci.read"), Some("pass")) if started => {
                cycles += 1;
                started = false;
            }
            _ => {}
        }
    }
    cycles
}

/// Milliseconds as seconds.
fn ms(value: u64) -> f64 {
    #[allow(clippy::cast_precision_loss)]
    let value = value as f64;
    value / 1000.0
}

/// Whether an idle wait's actions, joined by `,`, include `ci.read`.
fn on_ci_read(on: &str) -> bool {
    on.split(',').any(|action| action == "ci.read")
}

/// The attribution of `trace`.
#[must_use]
pub fn attribution(trace: &Trace) -> Attribution {
    let calls = trace.calls();
    let reported = !calls.is_empty() && calls.iter().all(|op| op.duration_ms.is_some());
    let mut seconds: BTreeMap<Actor, Sum<f64>> = Actor::ALL
        .iter()
        .filter(|actor| **actor != Actor::Takt)
        .map(|actor| (*actor, Sum::default()))
        .collect();
    let mut ci_read_s: Sum<f64> = Sum::default();
    let mut ci_wait_s: Sum<f64> = Sum::default();
    for op in &trace.ops {
        if op.is_step() && op.action.as_deref() == Some("ci.read") {
            ci_read_s.add(op.seconds());
        } else if op.actor == Actor::Idle && op.action.as_deref().is_some_and(on_ci_read) {
            ci_wait_s.add(op.seconds());
        }
        if op.actor == Actor::Takt {
            continue;
        }
        let span = if op.actor == Actor::Harness && reported {
            op.duration_ms.map(ms)
        } else {
            op.seconds()
        };
        seconds.entry(op.actor).or_default().add(span);
    }
    let takt_s = trace.header.wall_s.and_then(|wall| {
        let measured = seconds
            .values()
            .try_fold(0.0, |sum, actor| Some(sum + actor.value()?))?;
        Some((wall - measured).max(0.0))
    });
    let every = |figure: fn(&Op) -> Option<u64>| -> Option<u64> {
        if calls.is_empty() {
            return None;
        }
        calls
            .iter()
            .copied()
            .map(figure)
            .try_fold(0_u64, |sum, value| Some(sum.saturating_add(value?)))
    };
    let mut tokens: Sum<u64> = Sum::default();
    for op in &calls {
        tokens.add(op.tokens_sum);
    }
    Attribution {
        ci_reads: ci_read_s.of,
        ci_read_s,
        ci_waits: ci_wait_s.of,
        ci_wait_s,
        ci_cycles: ci_cycles(&trace.ops),
        wall_s: trace.header.wall_s,
        seconds,
        takt_s,
        basis: if reported {
            Basis::Reported
        } else {
            Basis::Spans
        },
        api_s: every(|op| op.api_ms).map(ms),
        steps: trace.header.steps,
        model_calls: calls.len() as u64,
        turns: every(|op| op.turns),
        tokens,
    }
}

/// `value` rounded to a whole number, printed without a fraction.
fn whole(value: f64) -> String {
    format!("{}", value.round())
}

/// `value` to one decimal.
fn tenth(value: f64) -> String {
    format!("{value:.1}")
}

/// A count, `-` when unknown.
fn count(value: Option<u64>) -> String {
    value.map_or_else(|| "-".to_owned(), |value| value.to_string())
}

/// A cost to the thousandth of a dollar, `-` when unknown.
fn cost(value: Option<f64>) -> String {
    value.map_or_else(|| "-".to_owned(), |value| format!("{value:.3}"))
}

/// A text as the report prints it, `-` when absent.
fn dash(value: Option<&str>) -> String {
    value.map_or_else(|| "-".to_owned(), escape)
}

/// `part` as a share of `wall`, to the per cent; `-` when either is unknown.
fn share(part: Option<f64>, wall: Option<f64>) -> String {
    match (part, wall) {
        (Some(part), Some(wall)) if wall > 0.0 => format!("{}%", (part / wall * 100.0).round()),
        _ => "-".to_owned(),
    }
}

/// `sum` seconds over its operations as the CI lines print them: `<s> s over <n> <noun>s`,
/// or `unknown (<s> s over <k> of <n> <noun>s)` when an operation has no span.
fn over(sum: &Sum<f64>, noun: &str) -> String {
    if sum.complete() {
        format!(
            "{} s over {} {noun}{}",
            tenth(sum.stated),
            sum.of,
            plural(sum.of)
        )
    } else {
        sum.print(&format!("{} s", tenth(sum.stated)), noun)
    }
}

fn plural(n: u64) -> &'static str {
    if n == 1 { "" } else { "s" }
}

/// The file name of a source path.
fn basename(source: &str) -> &str {
    source.rsplit('/').next().unwrap_or(source)
}

/// What a row of the per-step table calls the operation.
fn action(op: &Op) -> String {
    match (op.actor, op.action.as_deref()) {
        (Actor::Idle, Some(on)) => format!("waiting on {}", escape(on)),
        (Actor::Idle, None) => "waiting".to_owned(),
        (Actor::Human, _) | (_, None) => op.kind.name().to_owned(),
        (_, Some(action)) => escape(action),
    }
}

/// The operation's seconds, tokens and cost in parentheses, those that are known; empty when
/// none is.
fn figures(op: &Op) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(seconds) = op.seconds() {
        parts.push(format!("{} s", tenth(seconds)));
    }
    if let Some(tokens) = op.tokens_sum {
        parts.push(format!("{tokens} tokens"));
    }
    if let Some(cost) = op.cost_usd {
        parts.push(format!("${cost:.3}"));
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!(" ({})", parts.join(", "))
    }
}

/// Whether the operation is a row of the per-step table: a step, an idle wait or a resume;
/// a stop, a refusal and an error are listed under the defects instead.
fn is_row(op: &Op) -> bool {
    op.is_step()
        || (matches!(op.actor, Actor::Idle | Actor::Human) && op.kind != Kind::ApprovalWait)
}

/// The defects a trace shows: every pattern label, refusal, `NoProgress` stop and error, each
/// with the step it belongs to.
#[must_use]
pub fn defects(trace: &Trace) -> Vec<String> {
    let mut defects = Vec::new();
    for op in &trace.ops {
        if let Some(label) = &op.label {
            defects.push(format!(
                "step {}: {}{}",
                op.step,
                escape(label),
                figures(op)
            ));
            continue;
        }
        match op.kind {
            Kind::Refusal => defects.push(format!(
                "step {}: refusal by {}",
                op.step,
                dash(op.name.as_deref())
            )),
            Kind::NoProgress => defects.push(format!("step {}: stop NoProgress", op.step)),
            Kind::Other if op.outcome.as_deref() == Some("error") => {
                defects.push(format!("step {}: error{}", op.step, figures(op)));
            }
            _ => {}
        }
    }
    defects
}

/// What the sources did not record, as the report lists it.
#[must_use]
pub fn not_recorded(trace: &Trace) -> Vec<&'static str> {
    let a = attribution(trace);
    let header = &trace.header;
    let mut missing = vec!["acceptance"];
    if header.revision.is_none() {
        missing.push("takt revision");
    }
    if a.wall_s.is_none() || a.seconds.values().any(|sum| !sum.complete()) {
        missing.push("times");
    }
    if header.model.is_none() {
        missing.push("model");
    }
    if !a.tokens.complete() {
        missing.push("tokens");
    }
    if header.cost_usd.is_none() {
        missing.push("cost");
    }
    if a.turns.is_none() {
        missing.push("turns");
    }
    if a.basis == Basis::Spans {
        missing.push("call durations");
    }
    if a.api_s.is_none() {
        missing.push("api seconds");
    }
    if header.tokens.input.is_none() {
        missing.push("token components");
    }
    missing
}

/// The layers of the stack a run ran on (ADR 0101), in the order the report lists them: takt,
/// the harness session layer, the model layer, the runtime ([`crate::scorecard::LAYERS`]).
pub use crate::scorecard::LAYERS;

/// One layer's cost and failures in one run, each read from the record the layer wrote and
/// none when that record states none, never 0: the report prints `not reported` for none.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LayerCost {
    /// The layer's revision as the run recorded it; none when the record has no `layers` or
    /// the layer is absent from them.
    pub revision: Option<String>,
    /// The tokens the layer's record states, summed when every record states them.
    pub tokens: Option<u64>,
    /// The dollars the layer's record states, summed when every record states them.
    pub cost_usd: Option<f64>,
    /// The seconds the layer's record states, summed when every record states them.
    pub wall_s: Option<f64>,
    /// The failures the layer's record states, each with the step and the record line; none
    /// when no record of the layer's failures is read.
    pub failures: Option<Vec<String>>,
    /// The record lines the figures came from, in the record's order.
    pub records: Vec<usize>,
}

/// The cost and the failures of `trace` by layer, in [`LAYERS`] order, each read from the
/// record its layer wrote: the model layer's tokens, cost and API seconds from the harness
/// steps' lines (the stream's result the harness reported); the harness layer's call
/// durations from the same lines; takt's wall seconds from its validators' spans and its
/// remainder, and its failures (refusals, errors, `NoProgress` stops) from its own lines. The
/// harness session layer's own trace (`trace-ir/1`), the model layer's own failures and the
/// runtime's decision records are read by nothing yet: those figures are none.
#[must_use]
pub fn by_layer(trace: &Trace) -> Vec<(&'static str, LayerCost)> {
    let a = attribution(trace);
    let calls = trace.calls();
    let call_lines: Vec<usize> = calls.iter().filter_map(|op| op.line).collect();
    let revision = |layer: &str| {
        trace
            .header
            .layers
            .as_ref()
            .and_then(|layers| layers.get(layer).cloned())
    };
    let every = |figure: &dyn Fn(&Op) -> Option<f64>| -> Option<f64> {
        if calls.is_empty() {
            return None;
        }
        calls
            .iter()
            .map(|op| figure(op))
            .try_fold(0.0, |sum, value| Some(sum + value?))
    };
    let mut takt_failures: Vec<String> = Vec::new();
    let mut takt_records: Vec<usize> = Vec::new();
    for op in &trace.ops {
        let at = |op: &Op| {
            op.line.map_or_else(
                || " (line not numbered)".to_owned(),
                |line| format!(" ({}:{line})", record_name(trace)),
            )
        };
        match op.kind {
            Kind::Refusal => takt_failures.push(format!(
                "step {}: refusal by {}{}",
                op.step,
                dash(op.name.as_deref()),
                at(op)
            )),
            Kind::NoProgress => {
                takt_failures.push(format!("step {}: stop NoProgress{}", op.step, at(op)));
            }
            Kind::Other if op.outcome.as_deref() == Some("error") => {
                takt_failures.push(format!("step {}: error{}", op.step, at(op)));
            }
            _ => {}
        }
        if op.actor == Actor::Validator
            || matches!(op.kind, Kind::Refusal | Kind::NoProgress)
            || (op.kind == Kind::Other && op.outcome.as_deref() == Some("error"))
        {
            takt_records.extend(op.line);
        }
    }
    let validator_s = a.seconds.get(&Actor::Validator).and_then(Sum::value);
    let takt = LayerCost {
        revision: revision("takt"),
        tokens: None,
        cost_usd: None,
        wall_s: match (validator_s, a.takt_s) {
            (Some(validator), Some(takt)) => Some(validator + takt),
            _ => None,
        },
        failures: Some(takt_failures),
        records: takt_records,
    };
    let harness = LayerCost {
        revision: revision("harness"),
        tokens: None,
        cost_usd: None,
        wall_s: every(&|op| op.duration_ms.map(ms)),
        failures: None,
        records: call_lines.clone(),
    };
    let model = LayerCost {
        revision: revision("model"),
        tokens: a.tokens.value().filter(|_| !calls.is_empty()),
        cost_usd: every(&|op| op.cost_usd),
        wall_s: every(&|op| op.api_ms.map(ms)),
        failures: None,
        records: call_lines,
    };
    let runtime = LayerCost {
        revision: revision("runtime"),
        ..LayerCost::default()
    };
    vec![
        ("takt", takt),
        ("harness", harness),
        ("model", model),
        ("runtime", runtime),
    ]
}

/// The name of the record a trace's figures came from: its first source's file name, or
/// `events.jsonl`.
fn record_name(trace: &Trace) -> String {
    trace.header.sources.first().map_or_else(
        || "events.jsonl".to_owned(),
        |source| escape(basename(source)),
    )
}

/// A figure as the by-layer tables print it: the text, or `not reported` for none.
fn reported<T>(value: Option<T>, text: impl Fn(T) -> String) -> String {
    value.map_or_else(|| "not reported".to_owned(), text)
}

/// The record lines as the by-layer table names them: `<file>:<l1>,<l2>`; `-` for none.
fn records(trace: &Trace, lines: &[usize]) -> String {
    if lines.is_empty() {
        return "-".to_owned();
    }
    let lines: Vec<String> = lines.iter().map(usize::to_string).collect();
    format!("{}:{}", record_name(trace), lines.join(","))
}

/// One run's cost and failures by layer: the table, then each failure under its layer.
fn layer_section(trace: &Trace, out: &mut dyn Write) -> std::io::Result<()> {
    writeln!(
        out,
        "| layer | revision | tokens | cost $ | wall s | failures | records |"
    )?;
    writeln!(out, "|---|---|---|---|---|---|---|")?;
    let layers = by_layer(trace);
    for (layer, cost) in &layers {
        writeln!(
            out,
            "| {layer} | {} | {} | {} | {} | {} | {} |",
            cost.revision
                .as_deref()
                .map_or_else(|| "not recorded".to_owned(), escape),
            reported(cost.tokens, |tokens| tokens.to_string()),
            reported(cost.cost_usd, |cost| format!("{cost:.3}")),
            reported(cost.wall_s, whole),
            reported(cost.failures.as_ref(), |failures| failures
                .len()
                .to_string()),
            records(trace, &cost.records)
        )?;
    }
    let failures: Vec<String> = layers
        .iter()
        .flat_map(|(layer, cost)| {
            cost.failures
                .iter()
                .flatten()
                .map(move |failure| format!("- {layer}: {failure}"))
        })
        .collect();
    if failures.is_empty() {
        let (reported, not): (Vec<&str>, Vec<&str>) = layers
            .iter()
            .map(|(layer, cost)| (*layer, cost.failures.is_some()))
            .fold(
                (Vec::new(), Vec::new()),
                |(mut yes, mut no), (layer, is)| {
                    if is {
                        yes.push(layer)
                    } else {
                        no.push(layer)
                    }
                    (yes, no)
                },
            );
        writeln!(
            out,
            "failures by layer: {} none; {} not reported",
            reported.join(", "),
            not.join(", ")
        )?;
    } else {
        writeln!(out, "failures by layer:")?;
        for failure in failures {
            writeln!(out, "{failure}")?;
        }
    }
    Ok(())
}

/// A sum over runs as the total table prints it: `not reported` when no run reported the
/// figure, the sum when every run did, else the sum over the runs that did, marked.
fn total<T: Copy>(sum: &Sum<T>, text: impl Fn(T) -> String) -> String {
    if sum.known == 0 {
        "not reported".to_owned()
    } else if sum.complete() {
        text(sum.stated)
    } else {
        format!(
            "not reported ({} over {} of {} runs)",
            text(sum.stated),
            sum.known,
            sum.of
        )
    }
}

/// The total across `traces` by layer, then which layers are reported today.
fn total_section(traces: &[Trace], out: &mut dyn Write) -> std::io::Result<()> {
    writeln!(out)?;
    writeln!(out, "## total by layer")?;
    writeln!(out)?;
    writeln!(
        out,
        "| layer | tokens | cost $ | wall s | failures | runs |"
    )?;
    writeln!(out, "|---|---|---|---|---|---|")?;
    /// One layer's figures summed over the runs, each over the runs that reported it.
    #[derive(Default)]
    struct Total {
        tokens: Sum<u64>,
        dollars: Sum<f64>,
        seconds: Sum<f64>,
        failures: Sum<u64>,
    }
    let mut totals: BTreeMap<&'static str, Total> = BTreeMap::new();
    for trace in traces {
        for (layer, cost) in by_layer(trace) {
            let sum = totals.entry(layer).or_default();
            sum.tokens.add(cost.tokens);
            sum.dollars.add(cost.cost_usd);
            sum.seconds.add(cost.wall_s);
            sum.failures
                .add(cost.failures.map(|failures| failures.len() as u64));
        }
    }
    for layer in LAYERS {
        let sum = totals.entry(layer).or_default();
        writeln!(
            out,
            "| {layer} | {} | {} | {} | {} | {} |",
            total(&sum.tokens, |tokens| tokens.to_string()),
            total(&sum.dollars, |cost| format!("{cost:.3}")),
            total(&sum.seconds, whole),
            total(&sum.failures, |failures| failures.to_string()),
            traces.len()
        )?;
    }
    writeln!(out)?;
    writeln!(
        out,
        "layers reported today: model (tokens, cost and API seconds from the harness steps' lines, the stream's result the harness reported); harness (call durations from the same lines); takt (wall seconds from its validators' spans and its remainder; failures from its refusal, error and NoProgress lines). Not read yet, so not reported: the harness session layer's own trace (trace-ir/1), the model layer's own failures, and the runtime's decision records."
    )
}

/// One run's section.
fn section(trace: &Trace, out: &mut dyn Write) -> std::io::Result<()> {
    let a = attribution(trace);
    let header = &trace.header;
    writeln!(out)?;
    writeln!(out, "### {}", escape(&header.run))?;
    writeln!(out)?;
    writeln!(
        out,
        "harness {}; model {}; {} invocation{}, {} resume{}; {} to {}; stop {}",
        dash(header.harness.as_deref()),
        dash(header.model.as_deref()),
        header.invocations,
        plural(header.invocations),
        header.resumes,
        plural(header.resumes),
        dash(header.t0.as_deref()),
        dash(header.t_end.as_deref()),
        dash(header.stop.as_deref())
    )?;
    writeln!(out)?;
    writeln!(out, "| actor | seconds | share |")?;
    writeln!(out, "|---|---|---|")?;
    for actor in Actor::ALL {
        writeln!(
            out,
            "| {} | {} | {} |",
            actor.name(),
            a.printed(actor),
            share(a.of(actor), a.wall_s)
        )?;
    }
    writeln!(out)?;
    let basis = match a.basis {
        Basis::Reported => format!(
            "the harness's own duration, {n} of {n} calls",
            n = a.model_calls
        ),
        Basis::Spans => {
            "the step spans in the record (not every call reported a duration)".to_owned()
        }
    };
    writeln!(
        out,
        "harness seconds: {basis}; api {}; takt: the remainder (commits, decisions, process starts, gaps)",
        a.api_s
            .map_or_else(|| "-".to_owned(), |api| format!("{} s", whole(api)))
    )?;
    let components = header.tokens;
    let tokens = a.tokens.print(&a.tokens.stated.to_string(), "call");
    let cost_text = header
        .cost_usd
        .map_or_else(|| "-".to_owned(), |cost| format!("${cost:.3}"));
    match (
        components.input,
        components.output,
        components.cache_read,
        components.cache_write,
    ) {
        (Some(input), Some(output), Some(read), Some(write)) => writeln!(
            out,
            "tokens {tokens}: input {input}, output {output}, cache read {read}, cache write {write}; cost {cost_text}"
        )?,
        _ => writeln!(
            out,
            "tokens {tokens}: components not recorded; cost {cost_text}"
        )?,
    }
    writeln!(out, "ci reads: {}", a.ci_reads)?;
    writeln!(out, "ci cycles: {}", a.ci_cycles)?;
    writeln!(
        out,
        "seconds per ci read: {}; {}; waits on ci.read between them: {}, {}",
        a.per_ci_read().map_or_else(|| "-".to_owned(), tenth),
        over(&a.ci_read_s, "read"),
        a.ci_waits,
        a.ci_wait_s
            .print(&format!("{} s", tenth(a.ci_wait_s.stated)), "wait")
    )?;
    writeln!(out)?;
    writeln!(
        out,
        "| step | actor | action | seconds | harness s | turns | tokens | cost $ |"
    )?;
    writeln!(out, "|---|---|---|---|---|---|---|---|")?;
    for op in trace.ops.iter().filter(|op| is_row(op)) {
        writeln!(
            out,
            "| {} | {} | {} | {} | {} | {} | {} | {} |",
            op.step,
            op.actor.name(),
            action(op),
            op.seconds().map_or_else(|| "-".to_owned(), tenth),
            op.duration_ms
                .map_or_else(|| "-".to_owned(), |duration| tenth(ms(duration))),
            count(op.turns),
            count(op.tokens_sum),
            cost(op.cost_usd)
        )?;
    }
    writeln!(out)?;
    if !header.obligations.is_empty() {
        writeln!(out, "repository obligations:\n")?;
        write!(out, "{}", crate::obligations::table(&header.obligations))?;
        writeln!(out)?;
    }
    layer_section(trace, out)?;
    writeln!(out)?;
    let defects = defects(trace);
    if defects.is_empty() {
        writeln!(out, "defects: none")?;
    } else {
        writeln!(out, "defects:")?;
        for defect in defects {
            writeln!(out, "- {defect}")?;
        }
    }
    writeln!(out, "not recorded: {}", not_recorded(trace).join("; "))?;
    let mut sources: Vec<String> = Vec::new();
    if let Some(events) = header.sources.first() {
        sources.push(format!(
            "{} (steps, times, tokens)",
            escape(basename(events))
        ));
    }
    let steps_name = header
        .sources
        .get(1)
        .map_or("steps file", |path| basename(path));
    for note in &header.notes {
        sources.push(escape(&note.replacen("steps file", steps_name, 1)));
    }
    writeln!(
        out,
        "sources: {}",
        if sources.is_empty() {
            "-".to_owned()
        } else {
            sources.join("; ")
        }
    )
}

/// Prints the report of `traces`: one table across the runs, then a section per run.
pub fn print(traces: &[Trace], out: &mut dyn Write) -> std::io::Result<()> {
    writeln!(
        out,
        "| run | protocol | story | outcome | wall s | steps | model calls | turns | tokens | cost $ | validator s | forge s | idle s | human s | harness s | api s | other s | ci reads | s per ci read | layers |"
    )?;
    writeln!(
        out,
        "|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|"
    )?;
    for trace in traces {
        let a = attribution(trace);
        let header = &trace.header;
        writeln!(
            out,
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
            escape(&header.run),
            dash(header.protocol.as_deref()),
            dash(header.story.as_deref()),
            escape(&header.outcome),
            a.wall_s.map_or_else(|| "-".to_owned(), whole),
            a.steps,
            a.model_calls,
            count(a.turns),
            a.tokens.print(&a.tokens.stated.to_string(), "call"),
            cost(header.cost_usd),
            a.printed(Actor::Validator),
            a.printed(Actor::Forge),
            a.printed(Actor::Idle),
            a.printed(Actor::Human),
            a.printed(Actor::Harness),
            a.api_s.map_or_else(|| "-".to_owned(), whole),
            a.printed(Actor::Takt),
            a.ci_reads,
            a.per_ci_read().map_or_else(|| "-".to_owned(), tenth),
            escape(&crate::scorecard::layers_text(header.layers.as_ref()))
        )?;
    }
    for trace in traces {
        section(trace, out)?;
    }
    total_section(traces, out)
}

/// The runs of `traces` as one JSON document, `{"runs": [...]}`: for each run its name, case,
/// protocol, story, outcome and stop as the trace's header holds them, and its repository
/// obligations (`id`, `text`, `status`; none recorded is an empty list), the same rows the
/// markdown report prints.
///
/// # Errors
/// A write that fails.
pub fn print_json(traces: &[Trace], out: &mut dyn Write) -> std::io::Result<()> {
    let runs: Vec<serde_json::Value> = traces
        .iter()
        .map(|trace| {
            let header = &trace.header;
            serde_json::json!({
                "run": header.run,
                "case": header.case,
                "protocol": header.protocol,
                "story": header.story,
                "outcome": header.outcome,
                "stop": header.stop,
                "obligations": header.obligations,
            })
        })
        .collect();
    writeln!(out, "{}", serde_json::json!({ "runs": runs }))
}

/// The day of `takt report --cases` an instant no record states, or one that does not read as
/// one, counts on.
pub const UNKNOWN_DAY: &str = "unknown";

/// The figures `takt report --cases` sums for one repository on one UTC day, or over all of
/// them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DayFigures {
    /// The cases whose last `stop` line is `Completed (accepted)`, each on the day of the run
    /// that wrote that line. A scenario case's held-out result does not change it.
    pub accepted: u64,
    /// The tokens the harness reported on the runs opened that day, read by the scorecard's
    /// rule (`scorecard::runs`): a validator's step adds 0, `skipped` lines and lines after a
    /// run's `stop` are not read, and the `stop` line's own `tokens` never is. None, printed
    /// `unknown`, when a harness step of one of them recorded no tokens, or 0, and on the day
    /// [`UNKNOWN_DAY`] of a repository with a case that holds no run record
    /// ([`CaseSums::unrecorded`]).
    pub tokens: Option<u64>,
    /// The decisions of the cases' `authority.yaml` by a person, granted or denied, each on the
    /// day it was given (`granted_at`): an approver [`crate::case::is_approver`] admits, of the
    /// kind `human` (`human:<name>`, a name `takt approve` accepts). A grant by an agent role,
    /// and an approver `takt approve` refuses (`human:` with no name), is none.
    pub interventions: u64,
}

impl Default for DayFigures {
    fn default() -> Self {
        Self {
            accepted: 0,
            tokens: Some(0),
            interventions: 0,
        }
    }
}

impl DayFigures {
    fn add_tokens(&mut self, tokens: Option<u64>) -> anyhow::Result<()> {
        self.tokens = match (self.tokens, tokens) {
            (Some(sum), Some(tokens)) => Some(
                sum.checked_add(tokens)
                    .ok_or_else(|| anyhow::anyhow!("the tokens summed exceed {}", u64::MAX))?,
            ),
            _ => None,
        };
        Ok(())
    }

    fn add(&mut self, other: &Self) -> anyhow::Result<()> {
        let overflow = || anyhow::anyhow!("a sum exceeds {}", u64::MAX);
        self.accepted = self
            .accepted
            .checked_add(other.accepted)
            .ok_or_else(overflow)?;
        self.interventions = self
            .interventions
            .checked_add(other.interventions)
            .ok_or_else(overflow)?;
        self.add_tokens(other.tokens)
    }
}

/// One row of `takt report --cases`: a repository, a UTC day, its figures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DayRow {
    /// The repository: the directory above the cases root a `--cases` argument ends in
    /// (`<repo>/.takt/cases`, `<repo>/.engineering/cases`), else the argument as given; of a
    /// cases directory given more than once, however spelled, its first spelling.
    pub repository: String,
    /// The UTC date, `YYYY-MM-DD`, or [`UNKNOWN_DAY`].
    pub day: String,
    /// What it sums to.
    pub figures: DayFigures,
}

/// What `takt report --cases` prints: a row per repository and day, in that order, their
/// total, and the cases that hold no run record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseSums {
    /// The rows.
    pub rows: Vec<DayRow>,
    /// The rows summed.
    pub total: DayFigures,
    /// The case directories, as found under the cases directory given, whose run record states
    /// no run: no `events.jsonl`, as a case whose record lives only on its case branch, or one
    /// with no `opened` line. Each makes its repository's tokens on the day [`UNKNOWN_DAY`]
    /// unknown, so the total's too, and is named after the table.
    pub unrecorded: Vec<String>,
}

/// Whether `dir`, a directory in a cases directory, is a case: it holds `case.yaml` or the run
/// record (`events.jsonl`), whatever each is; a link at either is refused when it is read.
fn is_case(dir: &std::path::Path) -> bool {
    ["case.yaml", crate::events::EVENTS_FILE]
        .iter()
        .any(|file| std::fs::symlink_metadata(dir.join(file)).is_ok())
}

/// The cases under the cases directory `root`, by name: each directory in it that
/// [`is_case`]; every other directory and every file is skipped. A link in it is refused naming
/// it, as the run record is never read through one ([`crate::events::Events::read`]), and so is
/// a cases directory that does not read or holds no case.
fn cases_in(root: &std::path::Path) -> anyhow::Result<Vec<(String, std::path::PathBuf)>> {
    use anyhow::Context as _;
    let mut cases = Vec::new();
    for entry in std::fs::read_dir(root)
        .with_context(|| format!("read the cases directory {}", root.display()))?
    {
        let entry =
            entry.with_context(|| format!("read the cases directory {}", root.display()))?;
        let path = entry.path();
        let kind = entry
            .file_type()
            .with_context(|| format!("read {}", path.display()))?;
        anyhow::ensure!(
            !kind.is_symlink(),
            "{} is a symbolic link in the cases directory {}; a case is never read through one",
            path.display(),
            root.display()
        );
        if kind.is_dir() && is_case(&path) {
            cases.push((entry.file_name().to_string_lossy().into_owned(), path));
        }
    }
    anyhow::ensure!(
        !cases.is_empty(),
        "the cases directory {} holds no case: no directory in it holds case.yaml or {}",
        root.display(),
        crate::events::EVENTS_FILE
    );
    cases.sort();
    Ok(cases)
}

/// The repository a cases directory belongs to ([`DayRow::repository`]).
fn repository_of(cases: &std::path::Path) -> String {
    for root in crate::case::CASE_ROOTS {
        if cases.ends_with(root)
            && let Some(repository) = cases.parent().and_then(std::path::Path::parent)
        {
            let shown = repository.display().to_string();
            return if shown.is_empty() {
                ".".to_owned()
            } else {
                shown
            };
        }
    }
    cases.display().to_string()
}

/// The UTC date of an event line's instant, or [`UNKNOWN_DAY`] for none and one that does not
/// read.
fn day_of(at: Option<&str>) -> String {
    at.filter(|at| crate::scorecard::instant(at).is_some())
        .and_then(|at| at.get(..10))
        .map_or_else(|| UNKNOWN_DAY.to_owned(), str::to_owned)
}

/// Whether `approver`, a decision's in `authority.yaml`, is a person: an approver
/// [`crate::case::is_approver`] admits, of the kind `human`.
fn is_person(approver: &str) -> bool {
    crate::case::is_approver(approver)
        && approver
            .split_once(':')
            .is_some_and(|(kind, _)| kind == "human")
}

/// Sums, per repository and UTC day, the cases under each cases directory in `roots`: the
/// units accepted, the tokens and the interventions ([`DayFigures`]). Each run counts on the UTC
/// date of its `opened` line, each decision on that of its `granted_at`.
///
/// A case is a directory in a cases directory that holds `case.yaml` or `events.jsonl`; any
/// other directory or file there is not one and is skipped. A cases directory given more than
/// once, however spelled (its canonical path), is read once, under its first spelling. A case
/// whose run record states no run counts no tokens as 0: it makes its repository's tokens on
/// the day [`UNKNOWN_DAY`] unknown and is named in [`CaseSums::unrecorded`].
///
/// # Errors
/// A cases directory that does not read or holds no case, a link in one, a case's
/// `events.jsonl` or `authority.yaml` that does not read (each named), or a sum past
/// `u64::MAX`.
pub fn case_sums(roots: &[std::path::PathBuf]) -> anyhow::Result<CaseSums> {
    use anyhow::Context as _;
    let mut days: BTreeMap<(String, String), DayFigures> = BTreeMap::new();
    let mut read = std::collections::BTreeSet::new();
    let mut unrecorded = Vec::new();
    for root in roots {
        let canonical = std::fs::canonicalize(root)
            .with_context(|| format!("read the cases directory {}", root.display()))?;
        if !read.insert(canonical) {
            continue;
        }
        let repository = repository_of(root);
        for (name, dir) in cases_in(root)? {
            let lines = crate::events::Events::new(&dir).read()?;
            // `scorecard::runs` opens one row per `opened` line, in their order.
            let opened: Vec<String> = lines
                .iter()
                .filter(|line| line["event"] == "opened")
                .map(|line| day_of(line["at"].as_str()))
                .collect();
            if opened.is_empty() {
                days.entry((repository.clone(), UNKNOWN_DAY.to_owned()))
                    .or_default()
                    .add_tokens(None)?;
                unrecorded.push(dir.display().to_string());
            }
            let mut last_stop = None;
            for (run, day) in crate::scorecard::runs(&name, &lines).iter().zip(opened) {
                days.entry((repository.clone(), day.clone()))
                    .or_default()
                    .add_tokens(run.tokens)?;
                // A run with no stop line has an outcome no stop names ([`scorecard::outcome_of`]).
                if !matches!(run.stop.as_str(), "error" | "unfinished") {
                    last_stop = Some((day, run.stop == "accepted"));
                }
            }
            if let Some((day, true)) = last_stop {
                days.entry((repository.clone(), day))
                    .or_default()
                    .add(&DayFigures {
                        accepted: 1,
                        ..DayFigures::default()
                    })?;
            }
            let decisions = crate::case::Authority::read(&dir)?
                .map(|authority| authority.decisions)
                .unwrap_or_default();
            for decision in decisions
                .iter()
                .filter(|decision| is_person(&decision.approver))
            {
                days.entry((repository.clone(), day_of(Some(&decision.granted_at))))
                    .or_default()
                    .add(&DayFigures {
                        interventions: 1,
                        ..DayFigures::default()
                    })?;
            }
        }
    }
    let mut total = DayFigures::default();
    let mut rows = Vec::new();
    for ((repository, day), figures) in days {
        total.add(&figures)?;
        rows.push(DayRow {
            repository,
            day,
            figures,
        });
    }
    Ok(CaseSums {
        rows,
        total,
        unrecorded,
    })
}

/// A table cell's text: escaped ([`escape`]), a `|` as `\|`.
fn cell(text: &str) -> String {
    escape(text).replace('|', "\\|")
}

/// Prints `sums` as one markdown table, a row per repository and day and a total row; tokens
/// no record states in full print `unknown`, never a number, and a note says why, naming each
/// case that holds no run record ([`CaseSums::unrecorded`]).
///
/// # Errors
/// A write that fails.
pub fn print_case_sums(sums: &CaseSums, out: &mut dyn Write) -> std::io::Result<()> {
    let tokens = |figures: &DayFigures| {
        figures
            .tokens
            .map_or_else(|| "unknown".to_owned(), |tokens| tokens.to_string())
    };
    writeln!(
        out,
        "| repository | day (UTC) | units accepted | tokens | interventions |"
    )?;
    writeln!(out, "|---|---|---:|---:|---:|")?;
    for row in &sums.rows {
        writeln!(
            out,
            "| {} | {} | {} | {} | {} |",
            cell(&row.repository),
            cell(&row.day),
            row.figures.accepted,
            tokens(&row.figures),
            row.figures.interventions
        )?;
    }
    writeln!(
        out,
        "| total | | {} | {} | {} |",
        sums.total.accepted,
        tokens(&sums.total),
        sums.total.interventions
    )?;
    let a_step_unknown = sums
        .rows
        .iter()
        .any(|row| row.figures.tokens.is_none() && row.day != UNKNOWN_DAY);
    if a_step_unknown || (sums.total.tokens.is_none() && sums.unrecorded.is_empty()) {
        writeln!(
            out,
            "\ntokens `unknown`: a harness step of a run that day recorded no tokens, or 0, which is never read as 0."
        )?;
    }
    if !sums.unrecorded.is_empty() {
        writeln!(
            out,
            "\ntokens `unknown` on the day `unknown`: these cases hold no run record (no `events.jsonl`, or none with an `opened` line), so no record states their tokens:"
        )?;
        for case in &sums.unrecorded {
            writeln!(out, "- {}", escape(case))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn figures_print_as_the_record_reads_them() {
        assert_eq!(whole(130.109), "130");
        assert_eq!(whole(4.891), "5");
        assert_eq!(tenth(41.885), "41.9");
        assert_eq!(cost(Some(0.849_024)), "0.849");
        assert_eq!(cost(None), "-");
        assert_eq!(share(Some(130.109), Some(242.0)), "54%");
        assert_eq!(share(Some(1.0), None), "-");
        assert_eq!(share(None, Some(242.0)), "-");
        assert_eq!(basename("~/.cache/x/steps.jsonl"), "steps.jsonl");
    }

    #[test]
    fn a_sum_some_operations_cannot_join_prints_unknown_with_its_count() {
        let mut sum: Sum<f64> = Sum::default();
        sum.add(Some(1.5));
        sum.add(None);
        sum.add(Some(2.0));
        assert_eq!((sum.stated, sum.known, sum.of), (3.5, 2, 3));
        assert_eq!(sum.value(), None);
        assert_eq!(
            sum.print(&tenth(sum.stated), "call"),
            "unknown (3.5 over 2 of 3 calls)"
        );
        assert_eq!(over(&sum, "read"), "unknown (3.5 s over 2 of 3 reads)");
        let mut complete: Sum<u64> = Sum::default();
        complete.add(Some(7));
        assert_eq!(complete.value(), Some(7));
        assert_eq!(complete.print("7", "call"), "7");
        let none: Sum<f64> = Sum::default();
        assert_eq!(none.value(), Some(0.0));
        assert_eq!(over(&none, "read"), "0.0 s over 0 reads");
    }
}
