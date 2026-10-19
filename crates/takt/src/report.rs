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
        "| run | protocol | story | outcome | wall s | steps | model calls | turns | tokens | cost $ | validator s | forge s | idle s | human s | harness s | api s | other s | ci reads | s per ci read |"
    )?;
    writeln!(
        out,
        "|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|"
    )?;
    for trace in traces {
        let a = attribution(trace);
        let header = &trace.header;
        writeln!(
            out,
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
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
