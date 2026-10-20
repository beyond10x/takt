---
format: aep.planning-md/3
id: story:the-judge-compares-the-steps-a-candidate-targets
kind: story
status: implemented
title: The judge compares the steps a candidate targets, with the whole run kept not worse
summary: a candidate that halved its targeted step's tokens was rejected because unrelated steps moved the run median
tags:
- rsi
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-10-20T04:01:42Z", actor: "human:timo", revision: 4}
- {from: "proposed", to: "active", at: "2026-10-20T04:01:42Z", actor: "human:timo", revision: 5}
- {from: "active", to: "implemented", at: "2026-10-20T05:16:24Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"test_result":1}}}
---
# Story: the-judge-compares-the-steps-a-candidate-targets

## Outcome

A candidate that makes the steps its finding names cheaper is judged better on those steps, and
not only on the whole run's token median: it is better when every run's targeted steps cost fewer
tokens than the baseline's cheapest such step (or a rank test at five runs a side says so), while
the whole run stays not worse (outcomes held, token median under the baseline's saved maximum).

## Context

The trial agent's cycle on release 0.3.0 rejected its first candidate (branch
`trial-cand/2026-10-20-bee77822-1`, kept): `change.handover` took 8405 to 8588 tokens in five runs
of five against 15875 to 15962 at the trigger, but extra requests in steps 3 and 6 in two of the
five runs, which the candidate does not change, moved the run median above the baseline's, so the
whole-run rule rejected it. A rule that judges only the run median cannot tell a targeted
improvement from noise elsewhere in the run. Comes from self-improvement work, so it carries
`rsi`.

## Scope

- `.agents/trial-agent.md` (the parking rule), `crates/takt/src/scorecard.rs` (the comparison rule), `crates/takt/src/promotion.rs` (how a
  promotion records the rule it was judged under), `ess/domains/takt.yaml`, tests.

## Acceptance

1. `takt scorecard --against` with a candidate's targeted steps (the actions its finding's
   predicate names) reports, per targeted step, the candidate's tokens per run and the baseline's
   cheapest, and judges better when every candidate run is below, or a rank test at five runs a
   side rejects equality.
2. The whole run is still judged not worse: outcomes by rank held, token median under the
   baseline's saved maximum; a candidate better on its steps but worse on the run is refused,
   naming both.
3. Re-judging candidate 1 of the 0.3.0 cycle from its recorded rollouts, with no new runs, reports
   it better on `change.handover` and states the run-level result: a test on those records copied
   into a fixture, or a recorded check in the review.
4. The rule has a definition version of its own, so a scorecard judged under it is refused by a
   takt that does not know it.
5. Before a re-trial, the trial agent takes the baseline set's middle three runs (sorted, second
   to fourth by tokens) and lowers each by the candidate's low estimate: when all three fall under
   the re-trial's "better" threshold, the re-trial runs; otherwise the candidate is reviewed and
   built, its probe kept as evidence, and it is parked for this story's per-step judge.
   `.agents/trial-agent.md` states the rule. Example from the trial cycle on 0.4.0:
   claude-step-carries-the-git-status-snapshot, low estimate 1,290 tokens fewer per run; middle
   runs 66,929, 66,949 and 66,976 lowered to 65,639 to 65,686, all under 66,421, so it is
   re-trialled.
