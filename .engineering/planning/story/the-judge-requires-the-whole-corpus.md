---
format: aep.planning-md/3
id: story:the-judge-requires-the-whole-corpus
kind: story
status: active
title: A comparison missing a corpus scenario is refused, never no_regression
summary: 'trial 2: empty against empty exits 0'
tags:
- rsi
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-19T15:58:58Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-19T15:58:58Z", actor: "human:timo", revision: 3}
---
# Story: the-judge-requires-the-whole-corpus

## Outcome

`scorecard --against`, `scorecard.compare` and `takt promote` refuse a comparison where either
side lacks a scenario of the corpus or has none at all; a missing or empty scenario is never
`no_regression`.

## Context

RSI trial 2 verified with the CLI that an empty scorecard against an empty one exits 0, and that a
scenario only the candidate has is "no baseline" rather than worse (`scorecard.rs:1071`). The
protocol says the comparison covers every scenario of the corpus. The validator and `promote` use
the same `against()` (inferred).

## Acceptance

- Empty against empty, a missing scenario on either side, and a scenario with fewer cases than the
  rule's trials each refuse, naming the scenario: tests for the CLI, the validator and `promote`.
