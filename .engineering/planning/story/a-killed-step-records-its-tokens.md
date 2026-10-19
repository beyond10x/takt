---
format: aep.planning-md/3
id: story:a-killed-step-records-its-tokens
kind: story
status: draft
title: A step killed by the hour budget or a harness error still records the tokens it spent
summary: a killed step records 0 tokens, so a candidate whose steps time out looks cheaper to the judge
tags:
- rsi
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
revision: 2
---
# Story: a-killed-step-records-its-tokens

## Outcome

A harness step that ends in an error or is killed by the hour budget records the tokens the harness
reported before it ended, in a step line or in the stop line's total, and never counts them as 0.

## Context

Found by the wave 15 transcripts review (2026-10-19): a killed or timed-out step writes no step line,
and the stop line counts its tokens as 0 (`run.rs`, the hour-budget and error paths; the base behaves
the same). The scorecard reads those totals, so a candidate whose steps time out looks cheaper to the
promotion judge. The stream-json output now carries per-message usage, so the spent tokens are known
up to the kill.

## Acceptance

- A step killed by the hour budget after the harness reported usage records that usage: a test with a
  fake harness that prints usage lines and then hangs.
- A harness error after usage records it the same way.
- A step with no usage reported records tokens as unknown, not 0, and the scorecard treats unknown as
  unknown.
