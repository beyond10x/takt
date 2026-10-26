---
format: aep.planning-md/3
id: story:a-harness-step-without-an-action-is-retried
kind: story
status: draft
title: A judgement step whose session names no action is retried before the run ends
summary: seven runs ended exit 5 on a session without an action line, four of them at their first step (second kind-cluster trial)
tags:
- loom-agent
relations:
- decomposes: epic:takt-as-a-loom-agent
- serves: vision:safe-agentic-coding
revision: 2
---
## Outcome

A judgement step whose session ends without the line naming its action is tried again within the
run, a bounded number of times, before the run ends on the error, so one such session does not
end a run of many steps.

## Context

- The second kind-cluster trial, 2026-10-26 (takt 0.13.1, `--harness harness`, b10x-harness 0.20.0 on an openai-responses
  endpoint): runs 6 to 12 all ended exit 5, "the harness failed ... the session ended without the
  JSON line naming its action". Runs 6, 7 and 8 had 6, 42 and 44 steps; runs 9 to 12 failed at
  their first step, 1 step each, 2,205,919 tokens together, no change.
- Each restart by the pod resumed the case into the same failing judgement; the case and the
  tree were unchanged by the failed steps ("everything the step changed was discarded").
- The three transcripts read end right after a `conversation-compacted` warning (109,063,
  125,439 and 120,978 of 128,000 declared tokens), with no output in the next turn; nine other
  sessions in the same runs were compacted and finished. The harness side is not this story.

## Acceptance

1. A step whose session ends without an action line is retried up to a stated bound within the
   same run, each try recorded with its tokens; the run ends exit 5 only after the bound.
2. A retry starts from the state the failed try was discarded to, and a refused or reverted step
   is not retried.
