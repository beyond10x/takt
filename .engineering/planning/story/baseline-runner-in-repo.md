---
format: aep.planning-md/3
id: story:baseline-runner-in-repo
kind: story
status: draft
title: The four-arm baseline runner is part of the repository
summary: run-arm.sh, the token shim and the acceptance convention move into the repository so any run can be reproduced
relations:
- decomposes: epic:benchmark-integration
- serves: vision:safe-agentic-coding
scope:
- confidence: inferred
  path: docs/metrics/runner
revision: 3
---
# Story: baseline-runner-in-repo

## Outcome

The comparison method (four arms per scenario, one task text for the free arms, a held-out acceptance check, raw usage per run, timestamped streams) lives in the repository as a documented runner, so a reader can reproduce any figure in docs/metrics from its commands.

## Context

The runner of 2026-10-18 lives outside the repository (the operator's scratch directory), so the figures in docs/metrics name sources nobody else can run. It is a docs-and-tooling move, kept out of the judged code paths.

## Expected before

On takt at main, holds; after the change, fails:

- `ls docs/metrics/runner` exits non-zero.

## Scope

- `docs/metrics/runner` (inferred)

## Acceptance

`docs/metrics/runner/run-arm.sh <arm> <i>` reproduces a scenario 1 run of any arm from a clean machine with the documented prerequisites, writing the same result fields docs/metrics/scenario-runs.jsonl records.
