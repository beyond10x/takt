---
format: aep.planning-md/3
id: story:bench-in-scorecard
kind: story
status: draft
title: A pinned benchmark subset joins the scorecard's scenarios for promotion
summary: promotion is judged on benchmark tasks as well as the scratch scenarios
relations:
- decomposes: epic:benchmark-integration
- serves: vision:safe-agentic-coding
- depends_on: story:bench-free-arms
- depends_on: story:promotion
scope:
- confidence: inferred
  path: crates/takt/src/scorecard.rs
revision: 3
---
# Story: bench-in-scorecard

## Outcome

The scorecard that decides a promotion includes a pinned benchmark subset beside the scratch-repository scenarios, so a candidate that regresses on a benchmark task is not promoted.

## Context

design:recursive-self-improvement § Scorecard; its token rule and flakiness tolerance are open, and benchmark tasks make both questions sharper.

## Expected before

On takt at main, holds; after the change, fails:

- The scorecard reads only scenario runs.

## Scope

- `crates/takt/src/scorecard.rs` (inferred)

## Acceptance

A candidate whose pinned benchmark subset outcome is worse than the baseline's under the scorecard's dominance rule is reported worse, and promotion refuses it.
