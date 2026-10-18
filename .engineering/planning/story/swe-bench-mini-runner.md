---
format: aep.planning-md/3
id: story:swe-bench-mini-runner
kind: story
status: draft
title: takt runs SWE-bench Verified Mini tasks and hands each patch to the SWE-bench harness
summary: the issue text becomes the intent, the repository diff becomes the patch, the SWE-bench harness grades it
relations:
- decomposes: epic:benchmark-integration
- serves: vision:safe-agentic-coding
- depends_on: story:bench-task-protocol
- depends_on: story:takt-in-container
scope:
- confidence: inferred
  path: docs/bench/swe-bench
revision: 3
---
# Story: swe-bench-mini-runner

## Outcome

takt runs the 50 tasks of SWE-bench Verified Mini: per task, the issue text is the intent, the run works in the task's environment under `bench.task/1`, the diff against the base commit is the patch, and the SWE-bench harness grades it with the task's tests.

## Context

SWE-bench Verified Mini is a random 50-task subset of SWE-bench Verified (Hugging Face MariusHobbhahn/swe-bench-verified-mini; HAL leaderboard); the SWE-bench harness evaluates patches in Docker environments (design:benchmark-integration).

## Expected before

On takt at main, holds; after the change, fails:

- No runner for SWE-bench tasks exists in this repository.

## Scope

- `docs/bench/swe-bench` (inferred)

## Acceptance

On a pinned subset of 10 Mini tasks, every task yields a patch file and a harness verdict, recorded with the run's raw usage.
