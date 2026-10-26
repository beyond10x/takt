---
format: aep.planning-md/3
id: story:a-red-base-stops-at-open
kind: story
status: active
title: A case whose base already fails its tests says so before the first model call
relations:
- decomposes: epic:composable-agentic-work
- informed_by: review-result:variety-ops-a29d8c7
- serves: vision:safe-agentic-coding
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-10-26T20:53:25Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-26T20:53:54Z", actor: "human:timo", revision: 3}
---
## Outcome

When the first `tests.run` of a case, before any effect, fails, the run stops with its own stop reason (exit 3) naming the failing tests and the runner's rerun hint, or, where the protocol opts in, continues with the failure recorded as the base's, so that a later pass is judged against it. Either way no model call is spent before the operator learns the base is red.

## Context

From the headstart session's variety runs on public beyond10x tasks (4 runs, analysed 2026-10-19; run records outside the repository). On a canon task the seed's own tests failed at step 1, before any edit (`events.jsonl` line 3: `a_map_key_changed_in_the_compiled_spec_is_caught_beside_a_stale_copy`); `tests.pass` stayed false, `repository.merge` was blocked in all 13 decisions, cargo's fail-fast kept every later `tests.run` from reaching the story's own test, and the run ended NoProgress after 934 s and about $3.86. The red test is inferred, not verified, to come from the machine's `ess` (0.60.0) against the version the task pins. The planner's operator-path round reproduced the shape on scenario 1 (review-result:variety-ops-a29d8c7, run H: a red base, 9 steps, NoProgress). Related: story:harness-sees-what-failed (what the step is told after a failure).

## Scope

- `crates/takt/src/run.rs`: the first validator observation in `run_held`, a new `Stop` (inferred)
- `crates/takt/src/events.rs`: the stop's record (inferred)

## Acceptance

On scenario 1 with `done()` broken and committed at the base, `takt run` with a fake harness stops before the first harness step with exit 3, names `test_done_marks_one_item`, and records 0 model tokens; on the unbroken seed the run is unchanged.
