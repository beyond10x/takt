---
format: aep.planning-md/3
id: story:bench-free-arms
kind: story
status: draft
title: The same benchmark tasks run under the free-running harnesses, for comparison
summary: Claude Code, Claude Code isolated and Codex on the same pinned tasks, so a takt result is a comparison
relations:
- decomposes: epic:benchmark-integration
- serves: vision:safe-agentic-coding
- depends_on: story:harbor-agent-adapter
- depends_on: story:swe-bench-mini-runner
scope:
- confidence: inferred
  path: docs/metrics/bench-runs.jsonl
revision: 3
---
# Story: bench-free-arms

## Outcome

Each pinned benchmark subset runs under the free-running arms of the baseline method as well (Claude Code with its operator configuration, Claude Code isolated as takt isolates it, Codex), using the benchmarks' own agents where they exist, so each takt result has its comparison.

## Context

The scenario 1 comparison showed that isolation explained most of the earlier token gap (docs/metrics/README.md); a benchmark result without the isolated arm would repeat that mistake.

## Expected before

On takt at main, holds; after the change, fails:

- No free-arm run of a benchmark task is recorded.

## Scope

- `docs/metrics/bench-runs.jsonl` (inferred)

## Acceptance

For each pinned subset, docs/metrics/bench-runs.jsonl holds one record per task and arm with the harness verdict and raw usage, and the journey view draws them.
