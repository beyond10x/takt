---
format: aep.planning-md/3
id: story:run-events-on-the-cli
kind: story
status: draft
title: takt run --events streams the run's events as JSONL while it runs
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
- depends_on: story:run-event-contract
- informed_by: story:secret-redaction
revision: 1
---
## Outcome

`takt run … --events -` writes `takt-events/1` lines to stdout as they happen, one flushed line per event, and the human lines move to stderr; `--events <path>` appends to a file; the case's `events.jsonl` is unchanged.

## Context

Owner decision of 2026-10-19 (story:run-event-contract): an integrating seam reads a running takt through the CLI and JSONL. The stream carries what `events.jsonl` carries, so story:secret-redaction's rule for `events.jsonl` covers it.

## Scope

- `crates/takt-cli/src/main.rs`: the `--events` flag and stdout/stderr split (inferred)
- `crates/takt/src/run.rs`: where each event is written (inferred)

## Acceptance

A run with the fake harness, read line by line from a pipe, yields the same lines as the case's `events.jsonl` in the same order, and each step's event arrives before the next step starts (the reader's receive time is earlier than the next event's `at`).
