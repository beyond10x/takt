---
format: aep.planning-md/3
id: story:bench-run-export
kind: story
status: draft
title: takt exports each scenario or benchmark run as a run record of the organization's benchmark instrument
summary: a run's case, events and raw usage become a run record in the organization's benchmark instrument's shape
relations:
- decomposes: epic:benchmark-integration
- serves: vision:safe-agentic-coding
- depends_on: story:bench-nouns-in-ess
scope:
- confidence: inferred
  path: crates/takt-cli/src/main.rs
- confidence: inferred
  path: crates/takt/src/events.rs
revision: 10
---
# Story: bench-run-export

## Outcome

Every takt run on a scenario or a benchmark task can be exported as a run record of the organization's benchmark instrument (its arm, case, revision, verdict, raw usage and the path of its case directory), so the instrument's grader and fact table cover takt beside the other harnesses.

## Context

The instrument is the one design:benchmark-integration § The organization's instrument names. It judges runs in one vocabulary, while takt keeps its runs as case files and events. The mapping comes from story:bench-nouns-in-ess, which this story depends on.

"A missing figure is unknown, never 0" has a concrete form in the instrument's run shape. Its cost is an integer of micro-dollars that defaults to 0, written next to a `cost_basis` of `provider-reported`, `assumed` or `unknown`. So a cost takt does not know is exported as cost 0 with `cost_basis: unknown`. A run whose events carry a reported cost (story:bench-subset-and-budget) is exported as `provider-reported`. The run shape has no token field. The raw token usage goes into the entry's summary text until the instrument declares fields for it, which is the instrument's own work, filed in its own store.

The run shape requires a budget of at least one micro-dollar. It also refers to an arm, a case and a harness record. `takt export` takes these as arguments and does not invent them.

The instrument's validator checks a whole store, references included. One exported entry cannot pass or fail it on its own. Getting the entry accepted into an instrument store is story:takt-runs-in-bench. The check here runs offline on a fixture case.

## Expected before

On takt at main, holds; after the change, fails:

- `takt export --help` exits non-zero.

## Scope

- `crates/takt-cli/src/main.rs` (inferred)
- `crates/takt/src/events.rs` (inferred)

## Acceptance

On a fixture case whose `events.jsonl` reports tokens and no cost, `takt export --bench <case dir>` prints one run entry in the instrument's seed shape whose `finish` operation sets `cost_basis: unknown`, where at fb3c3bc `takt export` is not a command and exits 2.
