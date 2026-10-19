---
format: aep.planning-md/3
id: story:run-events-in-the-library
kind: story
status: draft
title: The takt library takes an event sink; the CLI stream is one sink
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
- depends_on: story:run-event-contract
revision: 1
---
## Outcome

The `b10x-takt` library exposes an event sink on the run options: a trait with one method taking `&Event`. `events.jsonl` and the CLI stream (story:run-events-on-the-cli) are two sinks; a sink that fails does not stop the run and is reported once.

## Context

Owner decision of 2026-10-19 (story:run-event-contract): an integrating seam reads a running takt through the library.

## Scope

- `crates/takt/src/run.rs`: the run options and the event write (inferred)
- `crates/takt/src/events.rs`: the sink trait (inferred)
- `crates/takt/src/lib.rs`: the export (inferred)

## Acceptance

A library test runs the fake harness with a collecting sink and gets the same sequence as `events.jsonl`; a sink that returns an error leaves the run's outcome and record unchanged, and the error is reported once.
