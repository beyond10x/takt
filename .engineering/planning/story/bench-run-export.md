---
format: aep.planning-md/3
id: story:bench-run-export
kind: story
status: draft
title: takt exports each scenario or benchmark run as a run record of the organization's benchmark instrument
summary: a run's case, events and raw usage become a bench run in bench's record shape, so bench's fact table holds takt
relations:
- decomposes: epic:benchmark-integration
- serves: vision:safe-agentic-coding
- depends_on: story:bench-nouns-in-ess
scope:
- confidence: inferred
  path: crates/takt-cli/src/main.rs
- confidence: inferred
  path: crates/takt/src/events.rs
revision: 4
---
# Story: bench-run-export

## Outcome

Every takt run on a scenario or a benchmark task can be exported as a run record of the organization's benchmark instrument (its arm, case, revision, verdict, raw usage and the path of its case directory), so bench's grader and fact table cover takt beside the other harnesses.

## Context

The organization's benchmark instrument judges runs in one vocabulary (`docs/design/bench-entities-v0.1.md` there); takt keeps its runs as case files and events. Depends on story:bench-nouns-in-ess for the mapping.

## Expected before

On takt at main, holds; after the change, fails:

- `takt export --help` exits non-zero.

## Scope

- `crates/takt-cli/src/main.rs` (inferred)
- `crates/takt/src/events.rs` (inferred)

## Acceptance

`takt export --bench <case>` writes one record that bench's validator accepts, with every figure taken from the case's events and raw usage and a missing figure written as unknown, never 0.
