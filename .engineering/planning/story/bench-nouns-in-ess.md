---
format: aep.planning-md/3
id: story:bench-nouns-in-ess
kind: story
status: implemented
title: takt's ESS model names the benchmark nouns, aligned with the benchmark instrument's entities
summary: BenchTask, BenchRun, BenchArm and the export link to the instrument's run entity declared before code
relations:
- decomposes: epic:benchmark-integration
- serves: vision:safe-agentic-coding
scope:
- confidence: inferred
  path: ess/domains/takt.citations.tsv
- confidence: inferred
  path: ess/domains/takt.yaml
revision: 11
transitions:
- {from: "draft", to: "proposed", at: "2026-10-18T20:13:47Z", actor: "human:timo", revision: 5}
- {from: "proposed", to: "active", at: "2026-10-18T20:13:47Z", actor: "human:timo", revision: 6}
- {from: "active", to: "implemented", at: "2026-10-19T04:04:07Z", actor: "human:timo", revision: 10, decided_on: {"recorded":{"test_result":1,"review_outcome":1}}}
---
# Story: bench-nouns-in-ess

## Outcome

takt's specification declares what a benchmark run is in takt (the task, the arm, the run, its verdict and raw usage) and how it maps to the benchmark instrument's entities (corpus, case, arm, run, fact), before any code builds on them.

## Context

Spec first (AGENTS.md § Rules). the organization's benchmark instrument types the comparison already; takt's model must reference those, not invent a parallel set. A relation into another repository's model that nothing settles is `UNMAPPED:`.

## Expected before

On takt at main, holds; after the change, fails:

- `grep -c 'Bench' ess/domains/takt.yaml` prints 0.

## Scope

- `ess/domains/takt.yaml` (inferred)
- `ess/domains/takt.citations.tsv` (inferred)

## Acceptance

`ess specify validate --path ess` passes with BenchTask, BenchRun and BenchArm declared, and `python3 -I scripts/check-citations --require-digests` exits 0 over their `read from:` lines; each relation to an entity of the instrument is a `relations:` entry or carries an `UNMAPPED:` comment beside the entity.

## Landed (wave 12)

Read from `git diff --name-only 6447eab^1 6447eab` (the unit's merge into the wave branch) at the close of wave 12 (wave branch head 63a9d02). BenchTask, BenchRun and BenchArm declared in ESS, each relation to a bench entity named or UNMAPPED. Merged before its grant in error; reviewed and granted after. The benchmark instrument is named without its repository since 7518a71.

- `ess/domains/takt.citations.tsv`
- `ess/domains/takt.yaml`
