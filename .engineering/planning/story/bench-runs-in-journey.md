---
format: aep.planning-md/3
id: story:bench-runs-in-journey
kind: story
status: draft
title: The journey view draws benchmark runs beside the scenario runs
relations:
- informed_by: review-result:benchmark-acceptance-round-1
- decomposes: epic:benchmark-integration
- serves: vision:safe-agentic-coding
- depends_on: story:bench-runs-schema
revision: 2
---
## Outcome

`docs/metrics/journey.py` reads `docs/metrics/bench-runs.jsonl` as well as `scenario-runs.jsonl`, and draws one lane per benchmark run in `journey.svg`, grouped by arm, using the same operation vocabulary. A benchmark comparison can then be read on the same timeline as the scenario runs.

## Context

This was split from story:bench-free-arms, whose earlier acceptance joined the rows and the drawing. design:benchmark-integration § How takt fits a benchmark: "the journey view draws them (story:harness-journey-timeline)". Today the generator reads only `scenario-runs.jsonl` (`docs/metrics/journey.py:21`), and only scenario 1 (`docs/metrics/journey.py:811`). It takes no arguments, so a check could only run on the committed records.

The input paths become arguments, with the committed files as the defaults, so the check runs offline on a fixture. No token or cost figure is taken from the journeys (`docs/metrics/journey.py:10`); figures stay in the rows.

## Expected before

On takt at main, holds; after the change, fails:

- `grep -c bench-runs docs/metrics/journey.py` prints 0.

## Scope

- `docs/metrics/journey.py` (inferred)
- `docs/metrics/fixtures/` (inferred)

## Acceptance

With `docs/metrics/fixtures/bench-runs-one.jsonl` (one takt-arm row whose `case_dir` names a committed fixture case), `python3 -I docs/metrics/journey.py --bench-runs docs/metrics/fixtures/bench-runs-one.jsonl --out <dir>` writes `<dir>/journeys.jsonl` holding at least one line whose `run` is that row's `id`, where at d17bc32 `journey.py` takes no arguments and writes no such line.

The written SVG also holds one lane per run, grouped by arm: for the fixture's one row, `grep -c 'data-arm="takt"' <out>` prints 1.
