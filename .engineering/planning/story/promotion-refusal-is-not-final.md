---
format: aep.planning-md/3
id: story:promotion-refusal-is-not-final
kind: story
status: implemented
title: A refused promotion can be judged again on new runs, and a new scenario cannot wedge the store
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
- informed_by: review-result:main-ae680d5-adversarial
scope:
- confidence: cited
  path: crates/takt/src/promotion.rs
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-10-19T02:52:54Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-19T02:52:54Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-10-19T06:14:33Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"test_result":1}}}
---
## Outcome

A revision refused by `takt promote` can be judged again when the operator supplies a scorecard of new runs (`--rejudge`, naming the earlier refusal in the record); and a candidate whose scorecard holds a scenario with fewer cases than the rule's trials is refused, so its copy can never become a baseline that every later promotion fails against.

## Context

From the adversarial review of main at bd20f7b on 2026-10-19: ten adversarial reviewers, each finding fact-checked against the code by a second reviewer (review-result:main-ae680d5-adversarial), findings A3.4 and A3.6. On takt's own corpus (scenario 1, baseline 5/5 ApprovalRequired, saved maximum 138,160) a candidate as good as the baseline is refused about 2-8% of the time (fact-checker's recomputation: tokens 1.9-5.8%, outcome 1.9%), and a refusal is final for that revision (`crates/takt/src/promotion.rs:1481-1501@bd20f7b`). `enough_trials` checks only the baseline's scenarios (`:1666-1677`); a promoted candidate with a new scenario under 5 cases becomes the baseline copy, after which every promotion stops at `enough_trials` and `--baseline-only` refuses because a baseline exists (`:1504-1512`).

## Scope

- `crates/takt/src/promotion.rs`: the refusal record and `enough_trials` (cited)

## Acceptance

A revision refused once is judged again with `--rejudge` on a scorecard of five new runs, and its record names the earlier refusal; a candidate scorecard with a scenario of one case is refused naming the scenario and the rule's trials.

## Landed (wave 13)

Read from `git diff --name-only 69b4d35^1 69b4d35` (the unit's merge into the wave branch) at the close of wave 13 (wave branch head 16ff976). --rejudge judges a refused revision again on new case ids; each refusal records its cases. Shared merge with story:promote-binds-scorecard-and-merge.

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
