---
format: aep.planning-md/3
id: story:the-token-rule-judges-request-counts
kind: story
status: draft
title: The token rule also judges how many requests a case makes
tags:
- from-finding
- rsi
relations:
- informed_by: review-result:w49-adversary-c-pass-1
- decomposes: epic:recursive-self-improvement
revision: 1
---
# Story: the token rule also judges how many requests a case makes

## Outcome

`takt promote` and `takt scorecard --against` refuse a candidate whose cases make materially more
model requests than the baseline's, even when each request costs the same or less, and a saved
scorecard records the token tolerance its rule applied.

## Context

From review-result:w49-adversary-c-pass-1, findings 6 and 7 (wave 49, out of the unit's scope):

- Since 0.28.0 a scenario is worse on tokens only when its median tokens per model request exceed
  the saved median by more than 5 % (`crates/takt/src/scorecard.rs`, `worse_against`). A candidate
  that makes more requests at the same or lower cost each passes with more total tokens; nothing
  judges totals or request counts (adversary finding 6, `scorecard.rs:1790`).
- The 5 % is the constant `TOKEN_TOLERANCE_PERCENT` (`scorecard.rs:140`), not a field of `Rule`, so
  a saved scorecard does not carry it and the "saved under another rule" check cannot see it
  change (finding 7).

## Acceptance

- A candidate whose median requests per case exceed the saved side's by more than a stated
  tolerance is refused, naming the figures; an unchanged binary with one extra request in a
  minority of cases is not: tests.
- The token tolerance is a field of the scorecard's rule, saved with it, and a comparison under a
  different tolerance is named as such.
