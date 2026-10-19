---
title: Methodology
description: Which figures are measured, which are derived, and what is not recorded.
---

# Methodology

Every figure on this site comes from a record in `docs/metrics/` or `docs/runs/` and names it. A
figure no record holds is left out.

## Measured

Read from a raw record a program wrote during the run.

| figure | read from |
|---|---|
| tokens, by input, output, cache read and cache write | the harness's own usage record: Claude Code's `--output-format json` result, Codex's `turn.completed` event; or takt's stop line, which sums the same records |
| cost | Claude's `total_cost_usd`, summed over calls; the API list price, notional. Codex reports none |
| model | Claude's `modelUsage` keys, per call; Codex states none |
| wall time | the runner's `result.json`, resumes included |
| steps, stop, exit code | takt's stop lines and exit status |
| held-out check | `acceptance.sh` on the pull request's head; for a variety run, the unit's test file on the case branch's tip |
| operation times | `events.jsonl` (1 s or, later, microsecond resolution); a runner shim's `started_at` and `recorded_at`; for the free arms, the arrival time of each line of their stream |

## Derived

Computed from measured records.

- Medians, minima and maxima per arm and revision in the [Runs](./runs.mdx) tables: from
  `scenario-runs.jsonl`; no program in the repository writes these tables.
- `journeys.jsonl` and the journey chart: `docs/metrics/journey.py`, from the raw files each run
  record names. It holds order, kind and time only; no token or cost figure is read from it.
- The timeline chart: `docs/metrics/timeline.py`.
- The README badges: `docs/metrics/badges.py`, from the planning store, these files and the
  citation check.
- The [Status](../status.mdx) table: `website/scripts/generate.mjs`, from the planning store at
  build time.

## Not recorded

A figure the source does not state is `null`, never 0. A 0 in a record is a 0 the source stated.

- The early takt trials state only the stop, the steps and the tokens; they are not timed.
- The variety runs kept no usage per call: no model, no cost, no token components.
- Claude Code runs without a stream record only a turn count; Codex without a stream records its
  items in order. Their lanes are "order only".
- For T-14..18 the takt revision is the operator's run note; the runs' own records name no binary.
- What no validator can observe: escaped defects, the value of a review, interventions the run's
  events do not record.

## Reading the numbers

Token sums include cache reads, which are billed at about 1/20 of input; compare cost beside
tokens, not tokens alone. n is small and one scenario carries most runs: first results, not a
benchmark.

Sources: `docs/metrics/README.md`, `design:recursive-self-improvement` § What earns each claim.
