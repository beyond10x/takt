---
format: aep.planning-md/3
id: story:scorecard-against-needs-the-trials
kind: story
status: active
title: takt scorecard --against gives no verdict on fewer cases than the rule's trials
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
- informed_by: review-result:main-fe93286-areas
scope:
- confidence: cited
  path: crates/takt/src/scorecard.rs
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-19T02:52:53Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-19T02:52:54Z", actor: "human:timo", revision: 3}
---
## Outcome

`takt scorecard --against` reports a scenario with fewer cases than the rule's `trials` as `too few cases (n of trials)` and exits non-zero, as promotion already refuses; the verdict is given only at `n >= trials`.

## Context

From the review of main at 9cc158b on 2026-10-19: ten independent reviewers, one per area, each finding then fact-checked by a second reviewer against the code (review-result:main-fe93286-areas), finding T3, partly confirmed: `against()` prints the rule's trials (`scorecard.rs:657-662@9cc158b`) but never enforces them; with margin 1 any single-case candidate passes the outcome check (1 errored case against 5/5 accepted: not worse, exit 0, `main.rs:393`). Promotion is protected by `enough_trials` (`promotion.rs:1656-1690`). The reviewer's own one-trial measurements read this verdict.

## Scope

- `crates/takt/src/scorecard.rs`: `against` (cited)

## Acceptance

A candidate scorecard with one case (outcome error) against a saved 5/5 baseline under the default rule prints `too few cases (1 of 5)` for the scenario and exits non-zero; with five cases the verdict is as today.
