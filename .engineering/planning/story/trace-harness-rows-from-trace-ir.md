---
format: aep.planning-md/3
id: story:trace-harness-rows-from-trace-ir
kind: story
status: draft
title: takt-trace/1 harness-step rows are a projection of trace-ir/1
relations:
- serves: vision:safe-agentic-coding
revision: 1
---
# Story: takt-trace/1 harness-step rows are a projection of trace-ir/1

## Outcome

`takt-trace/1` stays takt's case-level record. Its harness operations (the `harness` actor's rows) are filled
from `trace-ir-summary/1`, the projection metaharness declares over each step's `trace-ir/1` document, instead
of from Claude's `--output-format json` result and a runner shim's `steps.jsonl`.

## Context

- Atlas ADR 0096 decision 7 (lines 71-73): metaharness owns the session format; other trace formats become
  projections of `trace-ir/1` or are retired. metaharness `story:retire-trace-formats-beside-trace-ir`,
  decision row "`takt-trace/1`": survives, its harness-step rows become a projection.
- Today (takt `0ad8de9`, branch `wave/29`): `crates/takt/src/trace.rs:14` says harness operations are
  completed from a runner shim's `steps.jsonl` (one `claude --output-format json` result per step);
  `:927` completes them; `:1185` `read_steps`; `:1204` `Trace::from_case(case_dir, steps, run)`.
- The projection: `metaharness project [--from <source>] --to trace-ir-summary <record>` (metaharness release
  0.13.0). Fields include model, turns, input/output/cache-read/cache-creation tokens,
  `tokens_basis` (`session`, `sum-of-requests` or `null`), cost_usd, `models` (one entry per model with
  its own tokens and cost), duration_ms, duration_api_ms, first/last timestamp_ms, is_error, stop_reason,
  tool_calls, each `null` when the session's record does not state it. `input_tokens` counts cache
  differently per vendor (Claude excludes it, Codex includes it); a42 states which. Field list is
  amendment a42 of metaharness `docs/design/metaharness-protocol-v0.1.md`.
- Pin the `metaharness` binary at that release; no metaharness crate is a dependency (metaharness
  `AGENTS.md`: binary first).

## Acceptance

- expected_before: for a case with a harness step, `takt` fills that step's model, tokens, cost, turns
  and durations from `steps.jsonl`; a case without `steps.jsonl` has them absent. After: the same fields
  come from the step's `trace-ir-summary/1`, a field the summary has as `null` stays absent in the op row
  (never 0), and `read_steps` and the `steps` parameter of `from_case` are removed or no longer read for
  harness rows.
- A test over a committed recorded session (Claude stream-json or Codex rollout) shows the op row's values
  equal the summary's, field by field.
- Non-harness actors (validator, forge, takt, human, idle), the case header and acceptance are unchanged.

## Scope

takt: `crates/takt/src/trace.rs`, `crates/takt-cli/src/main.rs`, their tests and fixtures, the takt-trace/1
documentation (`website/docs/reference/takt-trace.md`), `CHANGELOG.md`.

## Readers of takt-trace/1

The op rows keep their fields; their values for a harness step come from another source, and a value the
summary does not state is absent where `steps.jsonl` may have given one. Each reader is checked against that:

- `takt report` (`crates/takt/src/report.rs:19`) and `takt view` (`crates/takt/src/view.rs:26`, `:289`,
  `:361-366`): they read harness rows' tokens and durations.
- The trial agent's rollouts (`.agents/trial-agent.md:88-93`, steps 3 and 4): each cycle directory keeps a run's
  `takt-trace/1` file, which `takt view` renders; a cycle kept before the change renders from its old rows. Input tokens of a Codex step
  include cache reads in the summary (amendment a42), so a token figure across the change is not comparable
  without `tokens_basis`; a scorecard across it says so.

## Order

Independent of `epic:takt-on-loom-sdk`: it touches neither the loop nor the confinement seam, so it may be
scheduled while the epic waits on Loom. takt's session executor depends on the metaharness crates at 0.11.0
(`Cargo.toml:18-19`); whether the projection is read through the `metaharness` binary at 0.13.0 or through
the crates moved to 0.13.0 is the first decision of the unit that takes it.
