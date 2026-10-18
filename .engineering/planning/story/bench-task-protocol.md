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
revision: 3
---
# Story: bench-task-protocol

## Outcome

takt runs a benchmark task under its own `protocol/1` document, `bench.task/1`: one claim `tests.pass` on the task's visible test command, the judgement actions to inspect, edit and test, no forge, pull request or merge, and outcomes `completed` and `abandoned`. The benchmark's own grading tests stay outside the case, as a held-out acceptance check does.

## Context

design:benchmark-integration § How takt fits a benchmark. Protocols are data (AGENTS.md § Rules): this is a protocol file, not Rust. The viewer at /takt/protocols/ shows it once it lands.

## Expected before

On takt at main, holds; after the change, fails:

- `ls protocols/bench-task/1.yaml` exits non-zero.

## Scope

- `protocols/bench-task/1.yaml` (inferred)

## Acceptance

`takt run --protocol protocols/bench-task/1.yaml --intent <task> --test-cmd <visible tests> --forge none` on a fixture task reaches `completed` once the visible tests pass, and offers no forge action.
