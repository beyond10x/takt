---
format: aep.planning-md/3
id: story:the-loop-comparison-needs-a-before-side
kind: story
status: draft
title: LoopCompare refuses a before side with fewer turns than the minimum
summary: an empty retained measurement records improved with 0 turns before
tags:
- rsi
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
revision: 2
---
# Story: the-loop-comparison-needs-a-before-side

## Outcome

`loop.compare` refuses a retained measurement with fewer than `MIN_TURNS` turns on the before side, as
`loop.measure` refuses it on the after side.

## Context

Found by the wave 15 judge review (2026-10-19): `LoopCompare` does not hold the before side to
`MIN_TURNS` (`loop_scorecard.rs:1614`, `:1690-1725`). A retained measurement with `rows: []` makes
every metric count as below target and records `improved` with 0 turns before. Reachable only by
editing the case's artifact by hand, since `LoopMeasure` refuses fewer than 3 turns.

## Acceptance

- A retained measurement with no rows, or fewer than `MIN_TURNS`, is refused by `loop.compare`,
  naming the count: a test.
