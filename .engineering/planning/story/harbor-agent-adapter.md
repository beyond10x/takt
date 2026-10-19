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
- depends_on: story:bench-subset-pinned
- depends_on: story:takt-static-binary
scope:
- confidence: inferred
  path: docs/bench/harbor
revision: 6
---
# Story: harbor-agent-adapter

## Outcome

Terminal-Bench 2.0 tasks run with takt as the agent: a Harbor adapter installs takt in each task container, runs `takt run` on the task's instruction under `bench.task/1`, and leaves grading to the task's own test script.

## Context

Terminal-Bench 2.0 runs through the Harbor harness (`harbor run -d terminal-bench/terminal-bench-2 -a <agent>`). Each task has an instruction, a test script and an oracle solution (design:benchmark-integration, sources there). Check the adapter interface against the official documentation when the story starts, including how Harbor loads an agent that is not built in (an import path or a registered name).

The adapter has two parts. The Harbor agent installs the static `takt` (story:takt-static-binary) and the harness CLI in the task container, then calls `run-takt.sh` with the task's instruction. `run-takt.sh` does the takt side, and it is what the acceptance checks offline:

- When the task directory is not a git repository, it runs `git init` and commits the starting tree, because takt commits implementation revisions.
- It runs `takt run --protocol protocols/bench-task/1.yaml --forge none --intent <instruction>` with `--case-id scenario-tb2-<task>-trial-<n>` (story:bench-in-scorecard).
- It passes the task's visible test command as `--test-cmd`, taken from the subset file (story:bench-subset-pinned), or `true` when the task exposes none. Terminal-Bench copies its grading tests in after the agent finishes, so many tasks have no visible test command at all. A row for such a task says so (story:bench-runs-schema).
- It copies the case directory into the logs directory, which outlives the container. Check that Harbor keeps the agent's logs directory once the container is removed.

The live run is not part of the acceptance. `harbor run` on the ten pinned task ids of story:bench-subset-pinned needs Docker, the dataset and a model login. Its verdicts come from the tasks' own tests, and they are recorded as rows of story:bench-runs-schema.

## Expected before

On takt at main, holds; after the change, fails:

- No Harbor adapter for takt exists in this repository.

## Scope

- `docs/bench/harbor/run-takt.sh` (inferred)
- `docs/bench/harbor/takt_agent.py` (inferred)
- `docs/bench/harbor/fixtures/hello/` (inferred)

## Acceptance

With a `claude` double first on PATH, `docs/bench/harbor/run-takt.sh <task dir> <logs>` (the command the adapter runs inside a task container) on the fixture task `docs/bench/harbor/fixtures/hello/` (an `instruction.md`, no git repository, no tests) leaves `<logs>/case/events.jsonl` ending in a `stop` event whose `stop` is `Completed (completed)`, where at fb3c3bc `docs/bench/harbor/run-takt.sh` does not exist.
