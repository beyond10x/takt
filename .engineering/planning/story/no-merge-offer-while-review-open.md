---
format: aep.planning-md/3
id: story:no-merge-offer-while-review-open
kind: story
status: draft
title: The merge is not offered while the protocol's review is open
relations:
- decomposes: epic:composable-agentic-work
revision: 1
---
## Outcome

`repository.merge` is not offered as approval-required while the protocol's review obligation is open (`review.required` set, `implementation.reviewed` unknown, no `code_review` evidence); a model's own "no defect found" in an approval step writes no evidence and does not close it.

## Context

From the headstart session's variety runs on public beyond10x tasks (4 runs, analysed 2026-10-19; run records outside the repository). In 3 runs the merge was offered as approval-required while `review.required` was open and `implementation.reviewed` was unknown (wt-untagged-target, decision 0006); the approval step's self-review wrote no evidence, and 2 of those 3 runs failed their held-out tests. Related: story:merge-needs-the-storys-tests (the tests the acceptance names), story:no-merge-approval-without-a-change (no change at all), story:no-model-step-for-approval.

## Scope

- `crates/takt/src/case.rs`: the merge's preconditions (inferred)
- the built-in story and change protocols: the review obligation (inferred)

## Acceptance

On scenario 1 with a fake harness that edits and passes the tests, the decision that follows lists `repository.merge` as blocked naming the open review, until a `code_review` evidence record exists; with the record, the merge is offered as before.
