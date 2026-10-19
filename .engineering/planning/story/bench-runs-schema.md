---
format: aep.planning-md/3
id: story:bench-runs-schema
kind: story
status: draft
title: docs/metrics/bench-runs.jsonl has a documented schema and a checker, and a takt row's figures are the ones its case recorded
relations:
- informed_by: review-result:benchmark-acceptance-round-1
- decomposes: epic:benchmark-integration
- serves: vision:safe-agentic-coding
- depends_on: story:bench-nouns-in-ess
revision: 1
---
## Outcome

`docs/metrics/README.md` documents `bench-runs.jsonl` the way it documents `scenario-runs.jsonl`. Each row is one attempt at one task under one arm, with these fields:

- `id`, `date`, `corpus`, `dataset_version`, `task`, `arm`
- `takt_revision`, `model`, `wall_seconds`, `stop`, `exit_code`
- `verdict`: `pass`, `fail` or `unknown`, from the benchmark's held-out tests
- `visible_tests`: the command, or `none`
- `tokens`: four parts and a sum, each `null` when the source does not state it
- `cost_usd`, `budget_usd`
- `case_dir`, for the takt arm
- `source`

`docs/metrics/check-bench-runs.py` refuses a row that breaks the schema. For a takt-arm row, it also refuses figures that differ from the stop events of the case the row names.

## Context

design:benchmark-integration § How takt fits a benchmark: per task and arm, the raw usage, wall time, the harness's verdict, the case directory and events are appended to `docs/metrics/bench-runs.jsonl`. No story wrote that schema or said how a takt-arm row is read.

The rules are the ones `docs/metrics/README.md` § The rule already sets:

- a figure the source does not state is `null`, never 0;
- `tokens.sum` for takt is the sum of the stop lines' tokens.

The schema adds two rules:

- every row names its `dataset_version` (story:bench-subset-pinned);
- a task stopped on a budget before its grader ran has verdict `unknown`, not `fail` (story:bench-subset-and-budget).

The schema follows `BenchRun` in takt's specification, so spec comes first (story:bench-nouns-in-ess, AGENTS.md § Rules). The check is offline, against a committed fixture case.

## Expected before

On takt at main, holds; after the change, fails:

- `ls docs/metrics/check-bench-runs.py` exits non-zero.

## Scope

- `docs/metrics/README.md` (inferred)
- `docs/metrics/check-bench-runs.py` (inferred)
- `docs/metrics/fixtures/` (inferred)

## Acceptance

On the fixture `docs/metrics/fixtures/bench-runs-takt-mismatch.jsonl` (a takt-arm row with `tokens.sum` 900 whose `case_dir` names a committed fixture case with two `stop` events of 1000 and 500 tokens), `python3 -I docs/metrics/check-bench-runs.py docs/metrics/fixtures/bench-runs-takt-mismatch.jsonl` exits 1 printing `line 1: tokens.sum 900, the case's stop events sum 1500`, where at fb3c3bc `docs/metrics/check-bench-runs.py` does not exist.
