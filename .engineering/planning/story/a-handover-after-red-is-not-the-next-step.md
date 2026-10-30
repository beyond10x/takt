---
format: aep.planning-md/3
id: story:a-handover-after-red-is-not-the-next-step
kind: story
status: active
title: A step after a red test run hands over instead of acting on the failure
summary: a harness chose change.handover at a revision whose tests failed, then a doc-only edit; the step does not lead with the failure
tags:
- finding
- priority:high
- rsi
refs:
- provider: finding
  reference: a-handover-after-red-is-not-the-next-step
relations:
- serves: vision:safe-agentic-coding
- depends_on: story:story-delivery-3-is-the-default
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-10-28T23:15:14Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-28T23:15:14Z", actor: "human:timo", revision: 3}
---
# A step after a red test run hands over instead of acting on the failure

## What is wrong

In the proof run of takt 0.18.1 on a story of beyond10x/llm
(story:context-length-refusal-is-typed, 2026-10-28), the visible test
`every_other_400_maps_as_the_status_table_says_with_no_cause` failed. `tests.write` and
`implementation.edit` stayed admissible, yet the harness chose `change.handover` at the failing
revision and then made an edit that changed documentation only. The run stopped
`RepeatedTestFailure` twice; run 1 spent 3.44M tokens.

The judgement step does not lead with the last failing tests, and a handover at a revision whose
tests fail is accepted as a choice like any other.

## Check

Not runnable by `scripts/check-finding`: it needs a harness that hands over at a red revision.
A library test with a scripted harness stands in.

```expected_before
command: cargo test -p b10x-takt --test handover_after_red -- --exact a_handover_at_a_red_revision_is_not_the_next_step
exit: 101
healthy_exit: 0
output: error: no test target named `handover_after_red`
source: crates/takt/src/harness.rs
```

## Outcome

After a `tests.run` that failed at the current revision, the next judgement step's prompt opens
with the failing tests and their assertion locations, and a `change.handover` chosen at that
revision is not accepted as the next step: the run tells the harness that the tests fail and
offers the step again with the writes the frontier admits.

## Acceptance

1. A scripted harness that hands over at a red revision gets the step again, its prompt naming
   the failing tests; the run does not record the handover.
2. A handover at a revision whose tests pass is recorded as today.
3. A protocol without a tests claim keeps today's behaviour.

## Scope

- `crates/takt/src/harness.rs` (inferred)
- `crates/takt/src/run.rs` (inferred)
