---
format: aep.planning-md/3
id: story:trial-numbers-are-unique-across-checkouts
kind: story
status: implemented
title: Scenario runs from fresh checkouts never share a trial number
summary: 'trial 2: four aborted runs, 239,745 tokens'
tags:
- rsi
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-10-19T15:58:59Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-19T15:58:59Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-10-19T23:14:31Z", actor: "human:timo", revision: 5, decided_on: {"recorded":{"test_result":1}}}
---
# Story: trial-numbers-are-unique-across-checkouts

## Outcome

Two `takt scenario run`s started from fresh checkouts never cut the same `trial/<n>` and never
share a case id, so their cases go into one scorecard.

## Context

RSI trial 2: the trial number is one past the highest `trial/<n>` the checkout knows
(`main.rs:1277-1303`), so five fresh clones all cut `trial/0`; four runs had to be aborted
(239,745 tokens) and each later run needed a private origin seeded by hand.

## Acceptance

- Five scenario runs from five fresh clones of one origin get five distinct trial numbers and case
  ids, and `takt scorecard --save` accepts all five: a test.
