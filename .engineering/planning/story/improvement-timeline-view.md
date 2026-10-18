---
format: aep.planning-md/3
id: story:improvement-timeline-view
kind: story
status: implemented
title: A generated timeline of takt's figures per revision, for the deck and the site
summary: a generator draws tokens, time and resumes per revision with waves and turns marked; deck and site embed it
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
- depends_on: story:metrics-over-time
scope:
- confidence: inferred
  path: docs/metrics/timeline.py
- confidence: inferred
  path: docs/metrics/timeline.svg
- confidence: inferred
  path: docs/pitch/index.html
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-10-18T14:33:46Z", actor: "human:timo", revision: 4}
- {from: "proposed", to: "active", at: "2026-10-18T14:33:46Z", actor: "human:timo", revision: 5}
- {from: "active", to: "implemented", at: "2026-10-18T14:34:06Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"test_result":1}}}
---
# Story: improvement-timeline-view

## Outcome

One generated view turns the metrics series into a timeline: per scenario, tokens, wall time and resumes per takt revision as a chart, with each wave and each verified improvement turn marked on the axis, and the free-running arms as reference lines. The pitch deck and the public site both embed it, so neither carries hand-copied numbers.

## Context

Owner request of 2026-10-18: "display a timeline of improvements and visualize this in the pitch deck". The pitch deck (draft 1, outside the repository) shows one table of first results typed in by hand; the public site shows no figures. A generated view keeps both current with `docs/metrics/` (story:metrics-over-time) and states n and the source of every point.

## Expected before

On takt at 4da25e0, holds; after the change, fails:

- No file under `docs/` contains an SVG chart generated from `docs/metrics/`: `ls docs/metrics/timeline.svg` exits non-zero.

## Scope

- `docs/metrics/timeline.svg`: the generated chart (inferred)
- `docs/pitch/index.html`: the pitch deck, moved into the repository, with a timeline slide (inferred)
- the generator: a script under `docs/metrics/` with no dependency beyond the standard library (inferred)

## Acceptance

Running the generator on `docs/metrics/` writes `timeline.svg` with one point per recorded takt run on scenario 1, the waves and verified turns marked, and the free-running medians as reference lines; the pitch deck's timeline slide and the site's measurements section embed that file, and adding one record and re-running the generator moves the chart without a hand edit.
