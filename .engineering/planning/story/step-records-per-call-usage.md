---
format: aep.planning-md/3
id: story:step-records-per-call-usage
kind: story
status: active
title: A harness step's record carries its model, cost, duration and turns
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
scope:
- confidence: cited
  path: crates/takt/src/events.rs
- confidence: cited
  path: crates/takt/src/harness.rs
- confidence: inferred
  path: crates/takt/src/run.rs
- confidence: inferred
  path: docs/metrics/README.md
- confidence: inferred
  path: ess/domains/takt.yaml
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-10-18T22:43:23Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-18T22:43:23Z", actor: "human:timo", revision: 3}
---
## Outcome

Each `step` line a harness step writes to `events.jsonl` carries, besides `usage`, what the harness reported for that call: `model`, `cost_usd`, `duration_ms`, `api_ms`, `turns` and `started_at`, each absent (never `0`) when the harness did not state it. An `events.jsonl` written before the change still reads.

## Context

From the owner's ask of 2026-10-19 (several reporters over one trace, the journey image on the public site, a report per run, more self-improvement on the site, the beyond10x docs style with a sidebar) and the read-only design written for it the same day. `Event::Step` has `by`, `name`, `action`, `summary`, `tokens`, `usage` and `revision` (`crates/takt/src/events.rs:88-99`); the harness reads only `usage` from the JSON result (`crates/takt/src/harness.rs:246-263`), although Claude's result states `total_cost_usd`, `modelUsage`, `num_turns`, `duration_ms` and `duration_api_ms`. Today those figures exist only in a runner shim's `steps.jsonl` outside the repository, so a run's cost and model cannot be read from its own record. Recording only; the loop does not change.

## Scope

- `crates/takt/src/events.rs`: `Event::Step`, optional fields (cited)
- `crates/takt/src/harness.rs`: reading the result, stamping the start (cited)
- `crates/takt/src/run.rs`: emitting the step (inferred)
- `ess/domains/takt.yaml`: the Step declaration (inferred; spec first)
- `docs/metrics/README.md`: the field table (inferred)

## Acceptance

A run with a fake harness that prints `total_cost_usd`, `num_turns`, `duration_ms`, `duration_api_ms` and a `modelUsage` key writes a `step` line whose `cost_usd`, `turns`, `duration_ms`, `api_ms` and `model` equal what the fake printed, and whose `started_at` precedes its `at`; a fake that prints none of them writes a `step` line without those keys; the T-16 `events.jsonl` (written at a0b8576) still loads.
