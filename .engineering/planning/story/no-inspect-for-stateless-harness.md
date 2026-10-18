---
format: aep.planning-md/3
id: story:no-inspect-for-stateless-harness
kind: story
status: draft
title: A stateless harness is not offered repository.inspect as a way forward
summary: read-only actions are not offered to a memoryless harness, or the prompt says they end the run with NoProgress
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
scope:
- confidence: cited
  path: crates/takt/src/harness.rs
- confidence: cited
  path: crates/takt/src/run.rs
- confidence: inferred
  path: crates/takt/tests/no_inspect_for_stateless_harness.rs
revision: 4
---
# Story: no-inspect-for-stateless-harness

## Outcome

A harness that keeps nothing between steps is not offered an action that can only end the run. Either read-only actions such as `repository.inspect` are not offered to a stateless harness, or the prompt says that choosing one ends the run with `NoProgress`.

## Context

Found by an independent code review of 2026-10-18, takt at 2a22093 (line numbers unchanged at 2ec88d4). Verified by the review with a fake harness; the operator's scenario logs show the same, a `repository.inspect` step right before `NoProgress`.

- `repository.inspect` is a protocol action with `effect: read` (`protocols/story-delivery/1.yaml:99-101`); the run offers every admissible action no validator owns (`crates/takt/src/run.rs:695-699`), and the prompt describes it as "read files to understand the code; change nothing" (`crates/takt/src/harness.rs:83`).
- Each `claude -p` step runs with `--no-session-persistence` (`harness.rs:189`): what an inspect step read is gone at the next step.
- A step that changes no file marks the state idle (`run.rs:1055-1058`); the next evaluation is identical, and the run returns `Stop::NoProgress`, exit 3 (`run.rs:683-685`).
- Overlaps: `story:no-model-step-for-approval` (a model step offered where none can help) and `story:ci-pending-waits` (the same idle path reached while waiting on the forge).

## Expected before

On takt at 2ec88d4, holds; after the fix, fails:

- A test with a fake harness that answers `repository.inspect` and changes nothing, on a fresh story case: the run stops with `Stop::NoProgress` after that one judgement step, and the prompt the fake received lists `repository.inspect`.

## Scope

- `crates/takt/src/run.rs`: the offered actions (cited)
- `crates/takt/src/harness.rs`: whether a harness is stateless, the prompt's description of read-only actions (cited)
- `crates/takt/tests/`: a fake-harness inspect test (inferred)

## Acceptance

With a stateless fake harness on a fresh story case, the prompt either does not offer `repository.inspect` or states that choosing it ends the run with `NoProgress`, and in the first case the run never stops with `NoProgress` from an inspect step.
