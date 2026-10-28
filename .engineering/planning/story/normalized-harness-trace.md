---
format: aep.planning-md/3
id: story:normalized-harness-trace
kind: story
status: draft
title: Tool calls and free-running runs in takt-trace/1, and runs compared by operation kind
summary: a step's tool calls as operations, a trace of a free-running harness, and takt report naming the operation kinds one run lacks; tokens from raw usage only
tags:
- priority:low
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
- depends_on: story:scorecard-reader
- depends_on: story:harness-journey-timeline
- depends_on: story:run-trace-and-report
- depends_on: story:trace-harness-rows-from-trace-ir
- depends_on: story:failed-gate-output-floods-prompt
scope:
- confidence: cited
  path: crates/takt-cli/src/main.rs
- confidence: inferred
  path: crates/takt/src/harness.rs
- confidence: inferred
  path: crates/takt/src/report.rs
- confidence: cited
  path: crates/takt/src/trace.rs
- confidence: inferred
  path: ess/domains/takt.yaml
revision: 8
---
# Story: normalized-harness-trace

## Outcome

`takt-trace/1` gains the three things it lacks for comparing runs on what they did:

1. A step's inner tool calls are operations of kind `tool`, for Claude Code and Codex alike, each with its intent in the shared vocabulary.
2. A run of a free-running harness, with no takt case, has a `takt-trace/1` of its own, from its session transcript.
3. `takt report` over two or more traces names the operation kinds one run has and another lacks.

Token and cost figures stay as `takt-trace/1` keeps them today, from each harness's raw usage only.

## Context

Owner decision of 2026-10-18, lower priority than every story planned so far. A fix that lowers tokens while dropping the verify step should show as a behaviour change, not only as a cheaper run (design:recursive-self-improvement § 5, story:second-harness-adapter).

Delivered already, not this story's:

- story:run-trace-and-report (implemented): `takt trace` writes one `takt-trace/1` per case, every operation in one vocabulary of actor, intent and kind, token and cost figures from raw usage only (`crates/takt/src/trace.rs:1-16`); `takt report` prints where a run's time and tokens went (`crates/takt/src/report.rs:1-14`).
- story:harness-journey-timeline (implemented): runs of takt, Claude Code and Codex on one timeline in one vocabulary.

Still missing, at 338082d:

- `takt.work.TraceKind` reserves `tool` "for a step's inner tool call once one is recorded (none is today: the step's transcript holds its tool calls, and the trace does not read it)" (`ess/domains/takt.yaml:3398-3400`). `Kind::Tool` is named at `trace.rs:131` and built nowhere.
- `takt trace` requires `--case`, a case directory (`crates/takt-cli/src/main.rs:437-440`), and every trace's arm is `takt` (`trace.rs:32-33`): a free-running run has no trace.
- `takt report` prints one table across the runs given and a section per run; it compares no operation kinds between runs.

Claude Code and Codex name their tools differently (Edit, Write, MultiEdit against apply_patch) and count cached tokens differently: Claude Code reports `input_tokens`, `cache_read_input_tokens` and `cache_creation_input_tokens` apart, Codex counts cached tokens inside `input_tokens`. The operation vocabulary is shared; the token counters are kept as each harness reported them.

The session trace format is metaharness's `trace-ir/1` (ADR 0096 § 7). story:trace-harness-rows-from-trace-ir fills harness rows from its `trace-ir-summary/1` projection, and story:takt-reads-harness-traces-as-trace-ir reads it for the report's harness layer. This story reads tool calls from the same source, after the first of them (`depends_on`), and carries no adapter of its own.

## Ordering

- `crates/takt/src/harness.rs` is wave 39's file (story:failed-gate-output-floods-prompt); this story waits for wave 39 to merge (`depends_on`).
- `crates/takt/src/trace.rs` and `crates/takt-cli/src/main.rs` are also story:trace-harness-rows-from-trace-ir's, which goes first (`depends_on`).

## Expected before

On takt at 338082d, holds; after the change, fails:

- `grep -c "Kind::Tool" crates/takt/src/trace.rs` prints 1, the name mapping at `:131`.
- `takt trace` without `--case` exits 2.

## Scope

- `crates/takt/src/trace.rs`: tool operations, a trace without a case (cited)
- `crates/takt/src/report.rs`: the comparison of operation kinds (inferred)
- `crates/takt-cli/src/main.rs`: `TraceArgs`, a session transcript as a run's source (cited, `:437-452`)
- `crates/takt/src/harness.rs`: the step's session trace reference, if story:trace-harness-rows-from-trace-ir leaves none (inferred)
- `ess/domains/takt.yaml`: `takt.work.TraceKind` and the trace's source (cited, `:3395-3420`; spec first)
- `crates/takt/tests/`: the test and its committed session fixtures (inferred)

## Acceptance

Offline, over committed fixtures: a takt case whose step's Claude Code session holds a Read, an Edit and a test command, and a free-running Codex session of the same scenario that holds an apply_patch and no test command.

1. `takt trace` of the case writes a `takt-trace/1` whose harness step has three operations of kind `tool`, with the intents `inspect`, `change` and `verify`.
2. `takt trace` of the Codex session writes a `takt-trace/1` whose arm is not `takt` and whose operations use the same vocabulary.
3. `takt report` over both traces names `verify` as present in the first and absent from the second.
4. Each token figure in that report equals the fixture's raw usage figure, Codex's cached tokens kept inside its `input_tokens`.

## Domain relations

- Trace → TraceOperation: takt.work.Trace; Trace → Run and the vocabulary UNMAPPED there.
