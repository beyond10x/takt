//! The run report: where a run's seconds and tokens went, read from its trace
//! ([`crate::trace`]) and from nothing else. The wall time is split by actor: the harness's
//! seconds are the durations it reported for its calls when every call has one, else the
//! calls' spans in the record; a validator's, the forge's, an idle wait's and a person's are
//! their spans; takt's is the remainder (commits, decisions, process starts, the gaps between
//! steps). Printed as markdown: one table across the runs given, then per run the actors'
//! shares, the model calls' tokens and cost, a row per step, the defects (every pattern label,
//! refusal, `NoProgress` and error), what the sources did not record, and which source each
//! figure came from.

use std::collections::BTreeMap;
use std::io::Write;

use crate::trace::{Actor, Kind, Op, Trace};

/// What the harness's seconds rest on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Basis {
    /// Every call's duration as the harness reported it, summed.
    Reported,
    /// The calls' spans in the record, from the line before each call to its own: not every
    /// call reported a duration.
    Spans,
}

/// One run's attribution.
#[derive(Debug, Clone, PartialEq)]
pub struct Attribution {
    /// The run's wall time, when the record has instants.
    pub wall_s: Option<f64>,
    /// Seconds per actor; takt's the remainder of the wall time, 0 when that is unknown.
    pub seconds: BTreeMap<Actor, f64>,
    pub basis: Basis,
    /// The calls' time in the API summed, when every call states it.
    pub api_s: Option<f64>,
    pub steps: u64,
    pub model_calls: u64,
    /// The calls' turns summed, when every call states them.
    pub turns: Option<u64>,
    /// The `ci.read` steps the run took, every outcome counted.
    pub ci_reads: u64,
    /// Their spans summed, in seconds; the idle waits between them are not inside.
    pub ci_read_s: f64,
    /// The idle waits whose actions include `ci.read`, and their seconds summed.
    pub ci_waits: u64,
    pub ci_wait_s: f64,
    /// The CI cycles: each `pr.open` step that pushed or opened a pull request and was
    /// followed by a `ci.read` that passed before the next such step.
    pub ci_cycles: u64,
}

impl Attribution {
    /// What a `ci.read` costs the run, in seconds: the reads' spans and the idle waits on
    /// `ci.read` before them, summed, over the reads' count; none without a read.
    #[must_use]
    pub fn per_ci_read(&self) -> Option<f64> {
        #[allow(clippy::cast_precision_loss)]
        let reads = self.ci_reads as f64;
        (self.ci_reads > 0).then(|| (self.ci_read_s + self.ci_wait_s) / reads)
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

impl Attribution {
    /// The seconds of `actor`.
    #[must_use]
    pub fn of(&self, actor: Actor) -> f64 {
        self.seconds.get(&actor).copied().unwrap_or(0.0)
    }
}

/// Milliseconds as seconds.
fn ms(value: u64) -> f64 {
    #[allow(clippy::cast_precision_loss)]
    let value = value as f64;
    value / 1000.0
}

/// The attribution of `trace`.
#[must_use]
pub fn attribution(trace: &Trace) -> Attribution {
    let calls = trace.calls();
    let reported = !calls.is_empty() && calls.iter().all(|op| op.duration_ms.is_some());
    let mut seconds: BTreeMap<Actor, f64> = Actor::ALL.iter().map(|actor| (*actor, 0.0)).collect();
    for op in &trace.ops {
        if op.actor == Actor::Takt {
            continue;
        }
        let span = if op.actor == Actor::Harness && reported {
            ms(op.duration_ms.unwrap_or(0))
        } else {
            op.seconds().unwrap_or(0.0)
        };
        *seconds.entry(op.actor).or_insert(0.0) += span;
    }
    let measured: f64 = seconds
        .iter()
        .filter(|(actor, _)| **actor != Actor::Takt)
        .map(|(_, span)| *span)
        .sum();
    if let Some(wall) = trace.header.wall_s {
        seconds.insert(Actor::Takt, (wall - measured).max(0.0));
    }
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
    let reads: Vec<&Op> = trace
        .ops
        .iter()
        .filter(|op| op.is_step() && op.action.as_deref() == Some("ci.read"))
        .collect();
    let waits: Vec<&Op> = trace
        .ops
        .iter()
        .filter(|op| {
            op.actor == Actor::Idle
                && op
                    .action
                    .as_deref()
                    .is_some_and(|on| on.split(',').any(|action| action == "ci.read"))
        })
        .collect();
    let spans = |ops: &[&Op]| -> f64 {
        ops.iter()
            .map(|op| op.seconds().unwrap_or(0.0))
            .fold(0.0, |sum, span| sum + span)
    };
    Attribution {
        ci_reads: reads.len() as u64,
        ci_read_s: spans(&reads),
        ci_waits: waits.len() as u64,
        ci_wait_s: spans(&waits),
        ci_cycles: ci_cycles(&trace.ops),
        wall_s: trace.header.wall_s,
        seconds,
        basis: if reported {
            Basis::Reported
        } else {
            Basis::Spans
        },
        api_s: every(|op| op.api_ms).map(ms),
        steps: trace.header.steps,
        model_calls: calls.len() as u64,
        turns: every(|op| op.turns),
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

/// A text, `-` when absent.
fn dash(value: Option<&str>) -> &str {
    value.unwrap_or("-")
}

/// `part` as a share of `wall`, to the per cent; `-` when the wall time is unknown.
fn share(part: f64, wall: Option<f64>) -> String {
    match wall {
        Some(wall) if wall > 0.0 => format!("{}%", (part / wall * 100.0).round()),
        _ => "-".to_owned(),
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
        (Actor::Idle, Some(on)) => format!("waiting on {on}"),
        (Actor::Idle, None) => "waiting".to_owned(),
        (Actor::Human, _) => op.kind.name().to_owned(),
        (_, Some(action)) => action.to_owned(),
        (_, None) => op.kind.name().to_owned(),
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
            defects.push(format!("step {}: {label}{}", op.step, figures(op)));
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
    if header.model.is_none() {
        missing.push("model");
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

/// One run's section.
fn section(trace: &Trace, out: &mut dyn Write) -> std::io::Result<()> {
    let a = attribution(trace);
    let header = &trace.header;
    writeln!(out)?;
    writeln!(out, "### {}", header.run)?;
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
        let seconds = a.of(actor);
        writeln!(
            out,
            "| {} | {} | {} |",
            actor.name(),
            whole(seconds),
            share(seconds, a.wall_s)
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
    let tokens = header.tokens;
    match (
        tokens.input,
        tokens.output,
        tokens.cache_read,
        tokens.cache_write,
    ) {
        (Some(input), Some(output), Some(read), Some(write)) => writeln!(
            out,
            "tokens {}: input {input}, output {output}, cache read {read}, cache write {write}; cost {}",
            tokens.sum,
            header
                .cost_usd
                .map_or_else(|| "-".to_owned(), |cost| format!("${cost:.3}"))
        )?,
        _ => writeln!(
            out,
            "tokens {}: components not recorded; cost {}",
            tokens.sum,
            header
                .cost_usd
                .map_or_else(|| "-".to_owned(), |cost| format!("${cost:.3}"))
        )?,
    }
    writeln!(out, "ci reads: {}", a.ci_reads)?;
    writeln!(out, "ci cycles: {}", a.ci_cycles)?;
    writeln!(
        out,
        "seconds per ci read: {}; {} s over {} read{}; waits on ci.read between them: {}, {} s",
        a.per_ci_read().map_or_else(|| "-".to_owned(), tenth),
        tenth(a.ci_read_s),
        a.ci_reads,
        plural(a.ci_reads),
        a.ci_waits,
        tenth(a.ci_wait_s)
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
        sources.push(format!("{} (steps, times, tokens)", basename(events)));
    }
    let steps_name = header
        .sources
        .get(1)
        .map_or("steps file", |path| basename(path));
    for note in &header.notes {
        sources.push(note.replacen("steps file", steps_name, 1));
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
        "| run | protocol | story | outcome | wall s | steps | model calls | turns | tokens | cost $ | validator s | forge s | idle s | human s | harness s | api s | other s | ci reads | s per ci read |"
    )?;
    writeln!(
        out,
        "|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|"
    )?;
    for trace in traces {
        let a = attribution(trace);
        let header = &trace.header;
        let seconds = |actor: Actor| whole(a.of(actor));
        writeln!(
            out,
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
            header.run,
            dash(header.protocol.as_deref()),
            dash(header.story.as_deref()),
            header.outcome,
            a.wall_s.map_or_else(|| "-".to_owned(), whole),
            a.steps,
            a.model_calls,
            count(a.turns),
            header.tokens.sum,
            cost(header.cost_usd),
            seconds(Actor::Validator),
            seconds(Actor::Forge),
            seconds(Actor::Idle),
            seconds(Actor::Human),
            seconds(Actor::Harness),
            a.api_s.map_or_else(|| "-".to_owned(), whole),
            if a.wall_s.is_some() {
                seconds(Actor::Takt)
            } else {
                "-".to_owned()
            },
            a.ci_reads,
            a.per_ci_read().map_or_else(|| "-".to_owned(), tenth)
        )?;
    }
    for trace in traces {
        section(trace, out)?;
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
        assert_eq!(share(130.109, Some(242.0)), "54%");
        assert_eq!(share(1.0, None), "-");
        assert_eq!(basename("~/.cache/x/steps.jsonl"), "steps.jsonl");
    }
}
