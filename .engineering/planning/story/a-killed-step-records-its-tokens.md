---
format: aep.planning-md/3
id: story:a-killed-step-records-its-tokens
kind: story
status: implemented
title: A step killed by the hour budget or a harness error still records the tokens it spent
summary: a killed step records 0 tokens, so a candidate whose steps time out looks cheaper to the judge
tags:
- rsi
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
- decomposes: epic:takt-as-a-loom-agent
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-10-29T21:36:00Z", actor: "human:timo", revision: 5, decided_on: {"recorded":{"review_outcome":1}}}
- {from: "proposed", to: "active", at: "2026-10-29T21:36:00Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"review_outcome":1}}}
- {from: "active", to: "implemented", at: "2026-10-30T00:45:23Z", actor: "human:timo", revision: 7, decided_on: {"recorded":{"test_result":1,"review_outcome":1}}}
---
# Story: a-killed-step-records-its-tokens

## Outcome

A harness step that ends in an error or is killed by the hour budget records the tokens the harness
reported before it ended, in a step line or in the stop line's total, and never counts them as 0.

## Context

Found by the wave 15 transcripts review (2026-10-19): a killed or timed-out step writes no step line,
and the stop line counts its tokens as 0 (`run.rs`, the hour-budget and error paths; the base behaves
the same). The scorecard reads those totals, so a candidate whose steps time out looks cheaper to the
promotion judge. The stream-json output now carries per-message usage, so the spent tokens are known
up to the kill.

## Acceptance

1. A harness double that prints usage lines totalling 400,000 tokens and then hangs, under
   `--hours` small enough to kill it: the run's `stop` line in `events.jsonl` counts the 400,000
   tokens, not 0.
2. A harness double that prints usage lines totalling 400,000 tokens and then exits non-zero: the
   failed step's tokens are in the stop line's total, and with `--tokens 1000000` the third such
   step stops the run `Tokens (1000000)` (exit 3) instead of running a fourth.
3. A harness double that ends with no usage line: the step's tokens are recorded as unknown, and
   `takt scorecard --against <saved>` over that case exits 1 naming the case and the step, as it
   does today for a harness step that recorded no tokens (`crates/takt-cli/src/main.rs`, the
   scorecard's exit 1 text).

## Scope

- `crates/takt/src/run.rs`: the hour-budget and harness-error paths of a harness step: the tokens the session reported before it ended go into the run's count and the step's line (inferred)
- `crates/takt/src/harness.rs`: the usage the session reported up to its end, kept on the error (inferred)
- `crates/takt/tests/`: a harness double that prints usage and then hangs, and one that prints usage and then fails (inferred)

This is what makes the token budget hold for a step that ends in an error: a run whose steps fail
after spending tokens today counts them as 0 toward `--tokens` (the cluster trial of 2026-10-26
ended 7 of 41 harness steps this way).
