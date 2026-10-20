---
format: aep.planning-md/3
id: story:the-scorecard-names-no-metric-it-did-not-compute
kind: story
status: implemented
title: takt scorecard prints a median only for a metric it computed, never another metric's figures under its name
summary: number()'s fallback to wall_ms printed wall-time medians as median step_tokens against a v3 baseline
tags:
- rsi
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
scope:
- confidence: cited
  path: crates/takt/src/scorecard.rs
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-10-20T14:29:00Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-20T14:29:00Z", actor: "human:timo", revision: 4}
- {from: "active", to: "implemented", at: "2026-10-20T14:29:01Z", actor: "human:timo", revision: 5, decided_on: {"recorded":{"test_result":1}}}
---
# Story: the-scorecard-names-no-metric-it-did-not-compute

## Outcome

`takt scorecard --against` prints a `median <metric>` line only for a metric whose per-case figure
it computed; a metric it has no figure for (`step_tokens`, which is a map by action, or any metric
added later) prints no median, so no line shows another metric's numbers under its name.

## Context

Found by the trial agent on 0.5.0 (`crates/takt/src/scorecard.rs:667-676`): `number()` matches
`outcome`, `steps`, `tokens`, `refusals`, `no_progress` and `errors`, then ends in
`_ => case.wall_ms`. The loop at `scorecard.rs:1430` prints `median {metric}` for every metric
whose definition changed, through `number()`, so against a version 3 baseline it printed
`scenario 1  median step_tokens v- 121000 v1 83883`: the wall-time medians (the baseline's trial-81
has `wall_ms` 121000) under the name of a metric that has no scalar. Reproduced with the 0.5.0
binary on the 0.4.0 trigger set against the 7ae5b05b rebaseline scorecard, exit 0. No verdict
changes: outcome and tokens are named and judged as before. Comes from self-improvement work, so
it carries `rsi`.

## Scope

- `crates/takt/src/scorecard.rs` (`number()` and the changed-metric loop), its tests.

## Acceptance

1. `number()` has no fallback: a metric it cannot give a per-case figure for is `None`, and the
   changed-metric loop prints nothing for it: a test on a saved scorecard whose `step_tokens`
   definition differs from the compared one shows no `median step_tokens` line.
2. `median wall_ms` prints only for `wall_ms`; `outcome`, `steps`, `tokens`, `refusals`,
   `no_progress` and `errors` print as before: a test.
3. A metric added later without a scalar is refused at compile time or by a test that walks
   `METRICS` and asserts each is either scalar or known to have none.
