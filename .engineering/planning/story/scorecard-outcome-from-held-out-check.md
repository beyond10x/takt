---
format: aep.planning-md/3
id: story:scorecard-outcome-from-held-out-check
kind: story
status: implemented
title: A run's outcome in the scorecard comes from the held-out check, not from the candidate's own record
tags:
- priority:high
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
- informed_by: review-result:main-ae680d5-adversarial
scope:
- confidence: cited
  path: crates/takt/src/scorecard.rs
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-10-19T02:52:54Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-19T02:52:54Z", actor: "human:timo", revision: 4}
- {from: "active", to: "implemented", at: "2026-10-19T06:14:33Z", actor: "human:timo", revision: 7, decided_on: {"recorded":{"test_result":1}}}
---
## Outcome

`takt scorecard` takes each scenario run's outcome from the held-out acceptance check recorded beside the run (`held-out.txt`, the result of the scenario's held-out check, written by the runner, not by the takt binary under judgement; the check is `check.toml` over `heldout.patch` as story:scenario-corpus-realistic commits them, `acceptance-<id>.sh` in the runner of 2026-10-18), and refuses a run that has none; the binary's own `stop` line is reported beside it, and a disagreement is named.

## Context

From the adversarial review of main at bd20f7b on 2026-10-19: ten adversarial reviewers, each finding fact-checked against the code by a second reviewer (review-result:main-ae680d5-adversarial), finding A3.2, confirmed medium; raised to high here because the promotion rule is the judge of every improvement turn. The outcome comes from the candidate's own `stop` string (`crates/takt/src/scorecard.rs:309-318@bd20f7b`) and its tokens from its own step lines (`:371-375`); the held-out acceptance each run already produces (`result.json` `acceptance` in the runner, `~/.cache/takt-baseline/run-arm.sh`) is never read. Honest bugs of this kind exist already: a stop saying accepted while the store refused (review-of-main-lows R5.2), a failed step's tokens dropped (story:failed-step-tokens-are-recorded). The token figure stays the binary's until a harness-side source exists (story:step-records-per-call-usage records the harness's own usage).

## Scope

- `crates/takt/src/scorecard.rs`: the outcome source (cited)
- the scenario runner: where the held-out result is written beside each case (inferred)

## Acceptance

A candidate case whose `stop` says `Completed (accepted)` but whose held-out result says FAIL is scored as failed and named as a disagreement; a case with no held-out result is refused naming the case.

## Landed (wave 13)

Read from `git diff --name-only 69b4d35^1 69b4d35` (the unit's merge into the wave branch) at the close of wave 13 (wave branch head 16ff976). A scenario case's outcome reads its held-out result (held-out.txt, written by the runner); the outcome metric is version 2 and --rebaseline moves a baseline to it. Shared merge with story:promote-binds-scorecard-and-merge.

- `crates/takt-cli/src/main.rs`
- `crates/takt-cli/tests/promote_cli.rs`
- `crates/takt-cli/tests/scorecard_adv1.rs`
- `crates/takt-cli/tests/scorecard_cli.rs`
- `crates/takt/src/promotion.rs`
- `crates/takt/src/scorecard.rs`
- `crates/takt/tests/scorecard.rs`
- `crates/takt/tests/scorecard_adv1.rs`
- `crates/takt/tests/scorecard_adv2.rs`
- `crates/takt/tests/trace.rs`
- `ess/domains/takt.citations.tsv`
- `ess/domains/takt.yaml`
