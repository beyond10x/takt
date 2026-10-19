---
format: aep.planning-md/3
id: story:run-event-contract
kind: story
status: draft
title: Every takt event follows one versioned schema, takt-events/1
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
- informed_by: story:run-trace-and-report
revision: 1
---
## Outcome

Every event takt writes (to the case's `events.jsonl` today, to a stream after story:run-events-on-the-cli and story:run-events-in-the-library) follows one versioned schema, `takt-events/1`, declared in `ess/domains/takt.yaml`: each line carries `schema`, `seq` (gapless per run), `at` (RFC 3339, milliseconds), `case`, `run` and `type`; the existing `Event` variants (`crates/takt/src/events.rs:62`) map onto types; a consumer skips a type it does not know; a field is never renamed within a schema version.

## Context

Owner decision of 2026-10-19: "running takt must emit events so integrating seams (via lib, or cli+jsonl) ... emit events while CI is progressing ... later same thing with gitlab". The noun exists: `takt.work.RunEvent` (`ess/domains/takt.yaml:409`, one line of a case's `events.jsonl`, `:367`); this story versions it rather than declaring a second one. Related: story:run-trace-and-report (`takt-trace/1`, a per-run summary read from these events), story:normalized-harness-trace.

## Scope

- `crates/takt/src/events.rs`: the envelope fields and the type names (inferred)
- `ess/domains/takt.yaml`: `takt.work.RunEvent` gains the envelope and the version (inferred; spec first)
- `docs/metrics/README.md`: the schema's reader notes (inferred)

## Acceptance

Every fixture `events.jsonl` in `crates/takt/tests/fixtures/` converts; the lines of a run written with the fake harness validate against the declared schema, with `seq` gapless from 1; a record written before the schema still reads.
