---
format: aep.planning-md/3
id: story:run-comparison-page
kind: story
status: draft
title: One page compares runs by arm and revision, with every run's spread visible
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
- depends_on: story:run-trace-and-report
- depends_on: story:docs-site-with-sidebar
revision: 1
---
## Outcome

The site has a comparison page generated from all traces: one row per (scenario, arm, revision, model) with n, median and min–max of tokens, cost, wall time, steps and resumes, and acceptance passes out of n; a dot strip per row shows every run as a dot and the median as a bar; each row links its run pages; a model change starts a new row.

## Context

From the owner's ask of 2026-10-19 (several reporters over one trace, the journey image on the public site, a report per run, more self-improvement on the site, the beyond10x docs style with a sidebar) and the read-only design written for it the same day. The figures sit today in `docs/metrics/scenario-runs.jsonl` (68 runs) and in hand-made tables; nothing on the site shows the spread of a row.

## Scope

- `crates/takt-docs/`: the comparison renderer (inferred)
- `website/docs/improvement/runs.md` and its SVG (generated) (inferred)

## Acceptance

For scenario 1, the takt row at a0b8576 states n 5, median 122,361 and maximum 138,160 tokens (the baseline T-14..18, `docs/metrics/scenario-runs.jsonl`), and its dot strip has 5 dots; every row's n equals the count of its traces.
