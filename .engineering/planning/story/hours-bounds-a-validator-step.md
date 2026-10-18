---
format: aep.planning-md/3
id: story:hours-bounds-a-validator-step
kind: story
status: draft
title: --hours bounds a validator step too
relations:
- depends_on: story:hours-bounds-a-harness-step
revision: 1
---
## Outcome

`takt run --hours <h>` also ends a validator step when the budget runs out: a validator's child process (the test command, the gates, a forge call, `scripts/check-finding`) is given the remaining budget as its timeout, so no step of either kind runs past the run's end.

## Context

story:hours-bounds-a-harness-step bounds harness steps only. A story-scoper pass on story:self-improvement-protocol (2026-10-18) found validator steps that run long: `scripts/check-finding` builds takt and may run up to 900 s (`scripts/check-finding:90-93`), and `ci.read` waits on CI (story:ci-pending-waits). Kept apart from the harness story so that one stays the small fix of improvement turn 3.

## Scope

- `crates/takt/src/validator.rs`: the `Validator` trait and each built-in validator's child process (inferred)
- `crates/takt/src/run.rs`: the remaining budget passed to the validator step (inferred)

## Acceptance

With a test command that sleeps 20 s and `--hours 0.001`, `takt run` stops within the budget plus a fixed grace of at most 10 s, kills the child and records no evidence for the step.
