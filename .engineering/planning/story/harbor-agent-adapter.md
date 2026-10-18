---
format: aep.planning-md/3
id: story:harbor-agent-adapter
kind: story
status: draft
title: takt is a Terminal-Bench 2.0 agent through a Harbor adapter
summary: a Harbor adapter installs takt in the task container and runs the task's instruction under bench.task/1
relations:
- decomposes: epic:benchmark-integration
- serves: vision:safe-agentic-coding
- depends_on: story:bench-task-protocol
- depends_on: story:takt-in-container
scope:
- confidence: inferred
  path: docs/bench/harbor
revision: 3
---
# Story: harbor-agent-adapter

## Outcome

Terminal-Bench 2.0 tasks run with takt as the agent: a Harbor adapter installs takt in each task container, runs `takt run` on the task's instruction under `bench.task/1`, and leaves grading to the task's own test script.

## Context

Terminal-Bench 2.0 runs through the Harbor harness (`harbor run -d terminal-bench/terminal-bench-2 -a <agent>`); each task has an instruction, a test script and an oracle solution (design:benchmark-integration, sources there). Check the adapter interface against the official docs when this starts.

## Expected before

On takt at main, holds; after the change, fails:

- No Harbor adapter for takt exists in this repository.

## Scope

- `docs/bench/harbor` (inferred)

## Acceptance

`harbor run -d terminal-bench/terminal-bench-2 -a takt` on a pinned subset of 10 tasks completes every task with a verdict from the task's tests and a takt case directory per task.
