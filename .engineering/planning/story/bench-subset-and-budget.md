---
format: aep.planning-md/3
id: story:bench-subset-and-budget
kind: story
status: draft
title: takt stops a run at a dollar budget given as --usd
summary: dataset version and task ids pinned per run; a token and dollar budget per task and per run
relations:
- decomposes: epic:benchmark-integration
- serves: vision:safe-agentic-coding
- depends_on: story:bench-task-protocol
scope:
- confidence: inferred
  path: docs/bench
revision: 9
---
# Story: bench-subset-and-budget

## Outcome

takt stops a run once the cost the harness reports reaches a dollar budget, given as `--usd <dollars>`, and the stop names that budget, the way `--tokens` stops on tokens. The dataset version and the task ids go to story:bench-subset-pinned. The budget for a whole run of many tasks belongs to the runners.

## Context

Benchmark runs cost real money per task. The scenario 2 to 6 runs cost $0.16 to $0.54 each (docs/metrics/README.md § Scenarios 2 to 6). product-requirements:takt R10 asks for bounded runs.

What is new is the dollar figure. takt already stops on tokens (`--tokens`, `crates/takt-cli/src/main.rs:224`). Claude's JSON result reports `total_cost_usd` (docs/metrics/README.md, the `cost_usd` row), but takt reads only `result` and `usage` from it (`crates/takt/src/harness.rs:245-263`). The cost is the harness's notional API list price, so a stop on it is a stop on what the harness reported, the same as tokens.

The new stop has to rank as a budget stop in the scorecard, next to `Iterations`, `Hours` and `Tokens` (`crates/takt/src/scorecard.rs:219`). Otherwise it ranks with `NoProgress`. That is a change to a scorecard definition, which is the owner's call (AGENTS.md § Roles), so `outcome` takes its next version in `METRICS` (`crates/takt/src/scorecard.rs:39`).

The budget for a whole run of many tasks is plain arithmetic in the runners (story:harbor-agent-adapter, story:swe-bench-mini-runner). Each task gets `--usd` set to the smaller of its own budget and what is left of the run's. A task that gets nothing is not started. A task stopped on a budget is recorded with the verdict `unknown`, never `fail` (story:bench-runs-schema). The token count restarts with every invocation (R10, `crates/takt/src/run.rs:400,443`), and the dollar count will too. A benchmark task runs as one invocation, so this does not affect it.

The check runs offline: a `claude` double reports a fixed cost on every step. No live run is needed.

## Expected before

On takt at main, holds; after the change, fails:

- `takt run --help | grep -c -- '--usd'` prints 0.

## Scope

- `crates/takt-cli/src/main.rs` (inferred)
- `crates/takt/src/harness.rs` (inferred)
- `crates/takt/src/run.rs` (inferred)
- `crates/takt/src/scorecard.rs` (inferred)

## Acceptance

With a `claude` double first on PATH that reports `"total_cost_usd": 0.02` on every step and never makes the fixture's `check.sh` pass, `takt run --repo <fixture> --intent "make check.sh pass" --test-cmd "sh check.sh" --harness claude --forge none --iterations 20 --usd 0.03` on the fixture task of story:bench-task-protocol exits 3 with a stop line that begins `stopped: Usd (0.03)`, where at fb3c3bc `--usd` does not parse and the command exits 2.
