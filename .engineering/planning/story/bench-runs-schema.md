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
- depends_on: story:baseline-runner-in-repo
revision: 2
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

`takt-bench check-runs <file>`, a subcommand of the `takt-bench` binary (crate `crates/takt-bench`, story:baseline-runner-in-repo, clap derive), refuses a row that breaks the schema. For a takt-arm row, it also refuses figures that differ from the stop events of the case the row names.

## Context

design:benchmark-integration § How takt fits a benchmark: per task and arm, the raw usage, wall time, the harness's verdict, the case directory and events are appended to `docs/metrics/bench-runs.jsonl`. No story wrote that schema or said how a takt-arm row is read.

The rules are the ones `docs/metrics/README.md` § The rule already sets:

- a figure the source does not state is `null`, never 0;
- `tokens.sum` for takt is the sum of the stop lines' tokens.

The schema adds two rules:

- every row names its `dataset_version` (story:bench-subset-pinned);
- a task stopped on a budget before its grader ran has verdict `unknown`, not `fail` (story:bench-subset-and-budget).

The schema follows `BenchRun` in takt's specification, so spec comes first (story:bench-nouns-in-ess, AGENTS.md § Rules). The check is offline, against a committed fixture case.

The checker was planned as `docs/metrics/check-bench-runs.py`. Running code committed to this repository is Rust with clap derive (approval-record:store-replan-2026-10-28), so it is a `takt-bench` subcommand instead; the crate is story:baseline-runner-in-repo's, and this story adds the subcommand to it.

## Expected before

On takt at main, holds; after the change, fails:

- `grep -rqs check-runs crates/takt-bench/src || echo no-check-runs` prints `no-check-runs`.

## Scope

- `crates/takt-bench/src/main.rs`: the `check-runs` subcommand (inferred)
- `crates/takt-bench/tests/check_runs.rs` (inferred)
- `docs/metrics/README.md` (inferred)
- `docs/metrics/fixtures/` (inferred)

## Acceptance

On the fixture `docs/metrics/fixtures/bench-runs-takt-mismatch.jsonl` (a takt-arm row with `tokens.sum` 900 whose `case_dir` names a committed fixture case with two `stop` events of 1000 and 500 tokens), `takt-bench check-runs docs/metrics/fixtures/bench-runs-takt-mismatch.jsonl` exits 1 printing `line 1: tokens.sum 900, the case's stop events sum 1500`, checked in `crates/takt-bench/tests/check_runs.rs` under `task test-library`, where at fb3c3bc no `check-runs` subcommand exists.
