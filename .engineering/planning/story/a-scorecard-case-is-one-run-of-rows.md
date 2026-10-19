---
format: aep.planning-md/3
id: story:a-scorecard-case-is-one-run-of-rows
kind: story
status: draft
title: The scorecard refuses a case whose rows are split up
summary: interleaved rows of two cases count as five trials; promote --dry-run would install
tags:
- rsi
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
revision: 2
---
# Story: a-scorecard-case-is-one-run-of-rows

## Outcome

Loading a scorecard refuses a case whose rows are not contiguous, naming the case, so two cases cannot
be counted as more trials than they are.

## Context

Found by the wave 15 judge review (2026-10-19): `cases()` counts each run of consecutive rows with the
same case as a separate case, and `load` does not check that a case's rows are together
(`scorecard.rs:728-740`, `:914`). Two cases interleaved into 5 rows count as 5 trials, and
`promote --dry-run` would install. Reachable only by editing a scorecard file by hand.

## Acceptance

- A scorecard with two cases' rows interleaved is refused on load, naming the case: a test.
- A contiguous scorecard loads as today.
