---
format: aep.planning-md/3
id: story:scenario-corpus-realistic
kind: story
status: draft
title: Five realistic scenarios join the corpus, each with a held-out acceptance check
summary: multi-file feature, hidden-test bug fix, behaviour-keeping refactor, gate-needing change, a task to refuse
relations:
- decomposes: epic:benchmark-integration
- serves: vision:safe-agentic-coding
scope:
- confidence: inferred
  path: docs/metrics/README.md
revision: 3
---
# Story: scenario-corpus-realistic

## Outcome

The scenario corpus grows from one toy story to six scenarios that agents can fail: a multi-file feature, a bug whose fix a hidden test checks, a refactor that must keep behaviour, a change the repository's gate must pass, and a task that should be refused; each declared in `.engineering/scenarios.conf` of a scratch repository at a seed commit, with a held-out acceptance script no arm sees.

## Context

Owner request of 2026-10-18: more realistic use cases, our own benchmark first. Scenario 1 could not tell the arms apart: every arm passed its held-out check 3 of 3 (docs/metrics/README.md). Uses story:scenario-corpus's `takt scenario run`.

## Expected before

On takt at main, holds; after the change, fails:

- The scratch repository's `.engineering/scenarios.conf` at origin/main declares one scenario.

## Scope

- `docs/metrics/README.md` (inferred)

## Acceptance

Six scenarios are declared at pinned seeds, each with an acceptance script that passes on a reference solution and fails on the seed, and one round of the four arms has run on each, recorded in docs/metrics/scenario-runs.jsonl.
