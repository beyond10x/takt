---
format: aep.planning-md/3
id: story:bench-subset-and-budget
kind: story
status: draft
title: A benchmark run names its pinned subset and stops at its budget
summary: dataset version and task ids pinned per run; a token and dollar budget per task and per run
relations:
- decomposes: epic:benchmark-integration
- serves: vision:safe-agentic-coding
- depends_on: story:harbor-agent-adapter
scope:
- confidence: inferred
  path: docs/bench
revision: 3
---
# Story: bench-subset-and-budget

## Outcome

Every benchmark run names its dataset version and the task ids it ran, so it can be repeated, and stops a task or the whole run when a token or dollar budget is reached, recording the stop.

## Context

Benchmark runs cost real money per task (tens of thousands of tokens per scenario run, docs/metrics). product-requirements:takt R10 (bounds hold per unit across resumes).

## Expected before

On takt at main, holds; after the change, fails:

- A benchmark run has no budget setting.

## Scope

- `docs/bench` (inferred)

## Acceptance

A run configured with a budget below one task's cost stops that task with a named budget stop and an unknown verdict, and the run record lists the dataset version and every task id with its state.
