---
format: aep.planning-md/3
id: story:run-tokens-unknown-is-not-zero
kind: story
status: draft
title: A run whose harness step recorded no tokens has unknown tokens, never a sum counting it 0
summary: the run-level tokens metric reads a missing step token record as 0, a saving a promotion can trust
tags:
- rsi
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
revision: 1
---
# Story: run-tokens-unknown-is-not-zero

## Outcome

A run whose harness step recorded no tokens has unknown run-level tokens, never a sum that counts
that step as 0, and `takt scorecard --against` and `takt promote` refuse to judge tokens on a set
holding such a run, naming the case and the step.

## Context

Found while fixing the same defect in the per-step judge (wave 20, story:the-judge-compares-the-steps-a-candidate-targets):
the run-level `tokens` metric still reads a harness step without `tokens` as 0 (`crates/takt/src/scorecard.rs`,
the `step` arm of `runs()`). With one candidate step's tokens removed from the 0.3.0 trial fixture,
the candidate's median fell from 67628 to 62554 and its difference against the saved median went
from +598 to -4476: a missing record reads as a saving. A promotion judges tokens at run level, so
this can let a worse candidate through. Comes from self-improvement work, so it carries `rsi`.

## Scope

- `crates/takt/src/scorecard.rs`, `crates/takt/src/promotion.rs`, `ess/domains/takt.yaml`, tests.

## Acceptance

1. A run with a harness step that recorded no tokens has unknown run tokens; a validator step
   still counts 0: a test.
2. `takt scorecard --against` and `takt promote` refuse, exit 1, naming the case and the step,
   when a scenario case of either side has unknown run tokens; nothing is judged or recorded.
3. The tokens metric's definition version moves, so an older takt refuses a scorecard that can
   hold an unknown; a stored baseline is compared only after `takt promote --rebaseline`.
4. The 0.3.0 fixture with one step's tokens removed is refused, not judged better.
