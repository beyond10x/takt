---
format: aep.planning-md/3
id: story:no-model-step-for-approval
kind: story
status: active
title: A run takes no model step when only a validator or a grant can advance the case
summary: 'R19: stop ApprovalRequired with no harness call when only an ungranted capability stands between the case and its outcome'
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
scope:
- confidence: inferred
  path: crates/takt/src/run.rs
- confidence: inferred
  path: crates/takt/tests/no_model_step.rs
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-10-18T18:27:21Z", actor: "human:timo", revision: 4}
- {from: "proposed", to: "active", at: "2026-10-18T18:27:21Z", actor: "human:timo", revision: 5}
---
# Story: no-model-step-for-approval

## Outcome

When the only actions that could advance a case are deterministic (a validator) or wait for a grant, the run takes no judgement step: it runs the validator, or stops with `ApprovalRequired` at once, and spends no model tokens on asking for the approval.

## Context

Owner decision of 2026-10-18 (requirement R19 in product-requirements:takt). Observed in the operator's scenario 1 runs on the scratch repository:

- `~/.cache/takt-trial-2026-10-18/run5.log:13-16`: step 1 `pr.open` and step 2 `ci.read` are validators; step 3 is `repository.merge (claude-code)` with effect `none; repository.merge needs an approval`; the run stopped `ApprovalRequired (repository.merge); steps 3, tokens 29908`. All 29,908 tokens went to the one model step that only requested the approval.
- The same shape at `scenario-a.log:81` and `scenario-b.log:79-82` (`tokens 29896`).

A frontier whose only open claim is reached through an action that `requires` a capability not granted is decided by the protocol, not by the model.

## Expected before

On takt at 7b5aca3, holds; after the fix, fails:

- A test with a protocol whose remaining claim needs `repository.merge` (requires `repository.merge`, not granted) and a recording harness: the run calls the harness once before stopping `ApprovalRequired`.
- Scenario 1 resumed after CI passes: the stop line of that invocation reports tokens above 0 while every step before the stop is a validator.

## Scope

- `crates/takt/src/run.rs`: the step choice before a judgement step (inferred)
- `crates/takt/tests/`: a no-model-step test (inferred)

## Acceptance

With every claim but those behind an ungranted capability held, the run stops `ApprovalRequired` naming the capability with 0 harness calls in that invocation; a case with an admissible judgement action that could still change a claim still offers it.
