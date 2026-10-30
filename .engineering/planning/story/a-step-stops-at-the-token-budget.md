---
format: aep.planning-md/3
id: story:a-step-stops-at-the-token-budget
kind: story
status: draft
title: A step's session stops at the token budget, not only between steps
summary: the overshoot is bounded by one model turn, not one step
tags:
- finding
- priority:medium
- trial-0.24.0
relations:
- informed_by: story:a-run-stops-at-a-default-token-budget
revision: 1
---
## Outcome

A run's token budget bounds the step in progress too: the harness session of a step is stopped
once the budget is reached, so a run's tokens at the stop exceed the budget by at most one model
turn, not by a whole step.

## Context

The budget is checked only between steps (`budget_spent`, `crates/takt/src/run.rs:10475-10491`
at 0.24.0, compares the tokens spent before a step). The harness input carries `time_left`
(`crates/takt/src/harness.rs:42-45`) but no tokens left, so a session is not bounded by the
budget (inferred). In cluster trial runs on 0.24.0:
- one run stopped `Tokens (3000000) … tokens 4012613`, 34% over: its last step, 1.2M tokens,
  started with about 2.8M spent;
- another stopped at 2,318,718 of 2,000,000 (one step alone was 838.3k), and its successor at
  5,185,629 of 5,000,000.

A resumed case past its budget then needs a larger budget to continue. Related:
story:a-run-stops-at-a-default-token-budget (implemented), whose acceptance accepts "3,000,000
or more".

```expected_before
command: takt run with a harness double that reports 0.6 * T tokens per step and --tokens T, then read the run's tokens at its Tokens stop
output: the tokens at the stop are 1.2 * T, over T by a whole step
```

## Acceptance

1. takt passes the tokens left to the harness beside `time_left`, and the session is ended once
   its tokens reach them (by the harness's own limit, or by takt stopping the session after the
   turn that crossed it).
2. With a harness double reporting 0.6 × T per step and `--tokens T`, the run stops `Tokens` with
   its tokens at most T plus one model turn's tokens, and the step cut short is recorded as such,
   its tokens counted.
3. A step that stops at the budget leaves the case resumable as a Tokens stop does today; nothing
   from the cut step is accepted as an action.
