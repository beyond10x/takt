---
format: aep.planning-md/3
id: story:metrics-over-time
kind: story
status: draft
title: Scenario runs and improvement turns are kept as a committed time series
summary: docs/metrics holds one record per scenario run and per improvement turn, raw usage only
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
scope:
- confidence: inferred
  path: docs/metrics/README.md
- confidence: inferred
  path: docs/metrics/improvement-turns.jsonl
- confidence: inferred
  path: docs/metrics/scenario-runs.jsonl
revision: 3
---
# Story: metrics-over-time

## Outcome

Every scenario run and every improvement turn adds one record to a committed time series, so takt's figures can be read across revisions: which takt revision, which arm (takt, or a harness running free), which model, wall time, steps, invocations and resumes, tokens as the harness reported them (input, output, cache read, cache write, and takt's sum), cost where the harness states it, the stop and exit code, and the held-out acceptance result. Each improvement turn adds its finding, its before and after result and the revision it was promoted at.

## Context

Owner request of 2026-10-18: "we do want to track metrics over time, and display a timeline of improvements". Today the figures live in scratch files outside the repository: the operator's trial logs and the baseline comparison's `result.json` files (the timed takt arm on wave 08 at 0a776d2 reached the merge approval in 127 s with 0 resumes and 106,003 tokens, where the same scenario before wave 08 needed several minutes and 1–2 resumes). Nothing in the repository keeps them, so no later run can be compared with an earlier one, and the pitch deck and the public site cannot show a trend.

story:scorecard-reader computes a scorecard per revision from case events; this story keeps the series those scorecards and the operator's runs append to. Raw usage is the only source of token and cost figures (story:normalized-harness-trace keeps semantic comparison apart).

## Expected before

On takt at 4da25e0, holds; after the change, fails:

- `ls docs/metrics/` exits non-zero: no series is committed.

## Scope

- `docs/metrics/scenario-runs.jsonl`: one record per scenario run (inferred)
- `docs/metrics/improvement-turns.jsonl`: one record per improvement turn (inferred)
- `docs/metrics/README.md`: the record fields and where each figure is read from (inferred)

## Acceptance

`docs/metrics/scenario-runs.jsonl` holds one record per scenario run recorded so far (the trial runs of 2026-10-18, the three free-running arms and the timed takt arm), each with the fields above and the path of the record it was read from, and `docs/metrics/improvement-turns.jsonl` holds the first turn (story:scenario-run-help-exit-codes, `reproduced` at 0a776d2, `not_reproduced` at 447423c); a figure not stated by its source is `null`, never 0.
