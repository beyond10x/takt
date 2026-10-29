---
format: aep.planning-md/3
id: story:closed-pull-request-stops-the-run
kind: story
status: draft
title: A run whose pull request was closed stops at once
summary: pr.open reading the case's pull request closed is a stop that names it, not more model steps
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
- depends_on: story:failed-gate-output-floods-prompt
- depends_on: story:unqualified-stop-records-no-layer
- depends_on: story:abandon-a-case
scope:
- confidence: cited
  path: crates/takt/src/run.rs
- confidence: cited
  path: crates/takt/src/validator.rs
- confidence: inferred
  path: crates/takt/tests/closed_pull_request_stops.rs
- confidence: inferred
  path: ess/domains/takt.yaml
revision: 5
---
# Story: closed-pull-request-stops-the-run

## Outcome

A run whose `pr.open` reads the case's pull request `closed` (closed unmerged) stops at once, with a stop that names the closed pull request, instead of spending its budget on model steps. The operator then abandons the case (story:abandon-a-case).

## Context

Split from story:abandon-a-case in plan-critic round 1 (review-result:frontier-1-acceptance-round-1). Found by an independent code review of 2026-10-18, takt at 2a22093; the lines below are re-read at 338082d.

- `pr.open` reads an open case's pull request through the forge and records its state as the record's value, `pull_request closed` for a closed one (`crates/takt/src/validator.rs:3359-3386`, the record at `:3404-3415`).
- Nothing turns that state into a stop, so model steps follow until the budget ends (inferred from the code, not run).
- The merge validator already reads `closed` as an end (`validator.rs:4576`, `:4588`); `pr.open` does not.

## Ordering

- This story lands on `crates/takt/src/validator.rs` and `crates/takt/src/run.rs`, which wave 39 edits (story:failed-gate-output-floods-prompt scopes `validator.rs`, story:unqualified-stop-records-no-layer scopes `run.rs`). It waits for wave 39 to merge: `depends_on` both.
- `crates/takt/src/run.rs` is also scoped by story:abandon-a-case and story:wall-time-test-without-sleep. story:abandon-a-case goes first (`depends_on`): this story's stop sends the operator to `takt abandon`, which that story creates. story:wall-time-test-without-sleep is tagged `rsi` and is scheduled first by that rule; its hunk is the step's cleanup path (`run.rs:12269-12502`), apart from where this story ends the run, so no edge, and `aep plan artifact waves` keeps the two in different waves.

## Expected before

On takt at 338082d, holds; after the change, fails:

- `grep -n "PullRequestState::Closed" crates/takt/src/validator.rs` prints only the merge validator's two lines (`4576`, `4588`), none inside `impl Validator for PrOpen` (`:3192-3477`).

## Scope

- `crates/takt/src/validator.rs`: `pr.open`'s `closed` state as a stop (cited, `:3359-3415`)
- `crates/takt/src/run.rs`: the stop ends the run (cited)
- `ess/domains/takt.yaml`: the new stop, declared before the code (inferred)
- `crates/takt/tests/closed_pull_request_stops.rs`: the test (inferred)

## Acceptance

With a forge double that reports the case's pull request `closed` on the read after the push, a resumed `takt run` exits non-zero right after that `pr.open` step, writes no later step line to the case's `events.jsonl`, and its stop line names the pull request's number, `closed` and `takt abandon <case>` with the case's id. With the double reporting `open`, the same run goes on to its next step.
