---
format: aep.planning-md/3
id: story:scorecard-outcome-from-held-out-check
kind: story
status: draft
title: A run's outcome in the scorecard comes from the held-out check, not from the candidate's own record
tags:
- priority:high
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
- informed_by: review-result:main-ae680d5-adversarial
revision: 2
---
## Outcome

`takt scorecard` takes each scenario run's outcome from the held-out acceptance check recorded beside the run (the scenario's `acceptance.sh` result, written by the runner, not by the takt binary under judgement), and refuses a run that has none; the binary's own `stop` line is reported beside it, and a disagreement is named.

## Context

From the adversarial review of main at ae680d5 on 2026-10-19: ten adversarial reviewers, each finding fact-checked against the code by a second reviewer (review-result:main-ae680d5-adversarial), finding A3.2, confirmed medium; raised to high here because the promotion rule is the judge of every improvement turn. The outcome comes from the candidate's own `stop` string (`crates/takt/src/scorecard.rs:309-318@ae680d5`) and its tokens from its own step lines (`:371-375`); the held-out acceptance each run already produces (`result.json` `acceptance` in the runner, `~/.cache/takt-baseline/run-arm.sh`) is never read. Honest bugs of this kind exist already: a stop saying accepted while the store refused (review-of-main-lows R5.2), a failed step's tokens dropped (story:failed-step-tokens-are-recorded). The token figure stays the binary's until a harness-side source exists (story:step-records-per-call-usage records the harness's own usage).

## Scope

- `crates/takt/src/scorecard.rs`: the outcome source (cited)
- the scenario runner: where the held-out result is written beside each case (inferred)

## Acceptance

A candidate case whose `stop` says `Completed (accepted)` but whose held-out result says FAIL is scored as failed and named as a disagreement; a case with no held-out result is refused naming the case.
