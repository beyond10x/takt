---
format: aep.planning-md/3
id: story:rejudge-is-bounded
kind: story
status: draft
title: A revision is judged again at most twice
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
revision: 1
---
## Outcome

`takt promote --rejudge` judges a refused revision at most twice more; a third rejudge is refused, naming the earlier refusals.

## Context

From the grant review of the wave-13 promotion stories (E2, 2026-10-19), note N8: each `--rejudge` on new runs is a fresh draw against a false refusal rate of about 2-8% on takt's own corpus (story:promotion-refusal-is-not-final), and the reverse holds too: a candidate that is in truth worse passes with a probability that grows with each draw. Without a bound, enough rejudges promote any candidate whose draws vary.

## Scope

- `crates/takt/src/promotion.rs`: the rejudge rule (inferred)

## Acceptance

A revision refused three times is refused on its third `--rejudge`, naming its three refusals.
