---
format: aep.planning-md/3
id: story:promotion-refusal-is-not-final
kind: story
status: draft
title: A refused promotion can be judged again on new runs, and a new scenario cannot wedge the store
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
- informed_by: review-result:main-ae680d5-adversarial
revision: 1
---
## Outcome

A revision refused by `takt promote` can be judged again when the operator supplies a scorecard of new runs (`--rejudge`, naming the earlier refusal in the record); and a candidate whose scorecard holds a scenario with fewer cases than the rule's trials is refused, so its copy can never become a baseline that every later promotion fails against.

## Context

From the adversarial review of main at ae680d5 on 2026-10-19: ten adversarial reviewers, each finding fact-checked against the code by a second reviewer (review-result:main-ae680d5-adversarial), findings A3.4 and A3.6. On takt's own corpus (scenario 1, baseline 5/5 ApprovalRequired, saved maximum 138,160) a candidate as good as the baseline is refused about 2-8% of the time (fact-checker's recomputation: tokens 1.9-5.8%, outcome 1.9%), and a refusal is final for that revision (`crates/takt/src/promotion.rs:1481-1501@ae680d5`). `enough_trials` checks only the baseline's scenarios (`:1666-1677`); a promoted candidate with a new scenario under 5 cases becomes the baseline copy, after which every promotion stops at `enough_trials` and `--baseline-only` refuses because a baseline exists (`:1504-1512`).

## Scope

- `crates/takt/src/promotion.rs`: the refusal record and `enough_trials` (cited)

## Acceptance

A revision refused once is judged again with `--rejudge` on a scorecard of five new runs, and its record names the earlier refusal; a candidate scorecard with a scenario of one case is refused naming the scenario and the rule's trials.
