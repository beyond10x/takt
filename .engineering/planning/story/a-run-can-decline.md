---
format: aep.planning-md/3
id: story:a-run-can-decline
kind: story
status: draft
title: A judgement step can decline or abandon a case, recorded as an outcome
relations:
- informed_by: review-result:scenario-corpus-round-0
- decomposes: epic:composable-agentic-work
revision: 1
---
## Outcome

A judgement step can end a case as declined (the task is forbidden by the rules it was given) or abandoned (it cannot be done inside the story's scope), and the case records that outcome with the step's reason, instead of ending as NoProgress.

## Context

From the first runs of the realistic scenario corpus (.engineering/planning/review-result/scenario-corpus-round-0.md), takt at the promoted a0b8576: `story.delivery/1`'s `abandoned` outcome needs `decision: explicitly_abandoned` (`protocols/story-delivery/1.yaml:155-158`), but no action produces it, and the only exits offered to the harness are approvals (`crates/takt/src/harness.rs:130`).
- Scenario 6 (a task the rules forbid): the model refused correctly at step 5 and the run ended NoProgress, exit 3; four resumes repeated it (167,776 tokens in all). A scorecard cannot tell a correct refusal from a stuck run.
- Scenario 5 (a second gate that needs a file outside the story's scope): the model saw the gate could not pass inside the scope and stopped NoProgress; each resume spent about 47k tokens on the same inspect (324,164 tokens in all).

## Scope

- `protocols/story-delivery/1.yaml`: an action producing the decision for `abandoned`, and a `declined` outcome if the protocol lacks one (cited)
- `crates/takt/src/harness.rs`: the exits offered to the harness (cited)
- `crates/takt/src/run.rs`: the stop line and exit code for these outcomes (inferred)

## Acceptance

On scenarios 5 and 6 of the scratch repository, the run ends with the case outcome `abandoned` or `declined`, a stop line naming it and the step's reason, an exit code distinct from NoProgress, and no resume repeats the step.
