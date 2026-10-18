---
format: aep.planning-md/3
id: story:bench-nouns-in-ess
kind: story
status: draft
title: takt's ESS model names the benchmark nouns, aligned with the organization's benchmark instrument's entities
summary: BenchTask, BenchRun, BenchArm and the export link to bench's run entity declared before code
relations:
- decomposes: epic:benchmark-integration
- serves: vision:safe-agentic-coding
scope:
- confidence: inferred
  path: ess/domains/takt.citations.tsv
- confidence: inferred
  path: ess/domains/takt.yaml
revision: 4
---
# Story: bench-nouns-in-ess

## Outcome

takt's specification declares what a benchmark run is in takt (the task, the arm, the run, its verdict and raw usage) and how it maps to the organization's benchmark instrument's entities (corpus, case, arm, run, fact), before any code builds on them.

## Context

Spec first (AGENTS.md § Rules). The organization's benchmark instrument types the comparison already (`docs/design/bench-entities-v0.1.md` there); takt's model must reference those, not invent a parallel set. A relation into another repository's model that nothing settles is `UNMAPPED:`.

## Expected before

On takt at main, holds; after the change, fails:

- `grep -c 'Bench' ess/domains/takt.yaml` prints 0.

## Scope

- `ess/domains/takt.yaml` (inferred)
- `ess/domains/takt.citations.tsv` (inferred)

## Acceptance

`ess specify validate --path ess` passes with BenchTask, BenchRun and BenchArm declared, each relation to a bench entity named or `UNMAPPED:`, and every `read from:` resolving.
