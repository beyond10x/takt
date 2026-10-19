---
format: aep.planning-md/3
id: story:bench-task-protocol
kind: story
status: draft
title: 'A protocol for a benchmark task: visible tests, no forge, the benchmark''s own tests held out'
summary: 'protocols/bench-task/1.yaml: tests.pass on the visible test command, outcome completed, no pull request or merge'
relations:
- decomposes: epic:benchmark-integration
- serves: vision:safe-agentic-coding
scope:
- confidence: inferred
  path: protocols/bench-task/1.yaml
revision: 6
---
# Story: bench-task-protocol

## Outcome

takt runs a benchmark task under its own `protocol/1` document, `bench.task/1`: one claim `tests.pass` on the task's visible test command, the judgement actions to inspect, edit and test, no forge, pull request or merge, and outcomes `completed` and `abandoned`. The benchmark's own grading tests stay outside the case, as a held-out acceptance check does.

## Context

design:benchmark-integration § How takt fits a benchmark. Protocols are data (AGENTS.md § Rules): this is a protocol file, not Rust. The viewer at /takt/protocols/ shows it once it lands.

The check runs offline. `--harness` takes `claude` or `none` (`crates/takt-cli/src/main.rs:617-626`), and `none` cannot edit toward a passing test, so the fixture uses a `claude` double on PATH, the same kind of double the scenario tests use. A model run on a real benchmark task is the work of story:harbor-agent-adapter and story:swe-bench-mini-runner, not this story's.

"No forge" is a property of the file, not a second acceptance. With `--forge none`, `pr.open`, `ci.read` and `repository.merge` are left to the harness (`crates/takt-cli/src/main.rs:248-252`), so a protocol that declared those actions would hand them to the model. `bench.task/1` declares none of them, and the reviewer checks that by reading the file.

The outcome is named `completed`, not `accepted`. Until story:bench-in-scorecard lands, the scorecard ranks it with `NoProgress` (`crates/takt/src/scorecard.rs:215-222`).

## Expected before

On takt at main, holds; after the change, fails:

- `ls protocols/bench-task/1.yaml` exits non-zero.

## Scope

- `protocols/bench-task/1.yaml` (inferred)
- `crates/takt/tests/fixtures/bench-task/` (inferred)
- `crates/takt-cli/tests/bench_task_cli.rs` (inferred)

## Acceptance

On the fixture task `crates/takt/tests/fixtures/bench-task/` (a git repository whose `check.sh` exits 0 only once `work.txt` exists), with a `claude` double first on PATH whose first step writes `work.txt`, built as `crates/takt-cli/tests/scenario_run.rs:260-270` builds one, `takt run --repo <fixture> --protocol protocols/bench-task/1.yaml --intent "make check.sh pass" --test-cmd "sh check.sh" --harness claude --forge none` exits 0 with the stop line `stopped: Completed (completed)`, where at fb3c3bc the same command exits 1 because `protocols/bench-task/1.yaml` does not exist.
