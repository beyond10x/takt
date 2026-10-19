---
format: aep.planning-md/3
id: story:run-record-survives-a-torn-character
kind: story
status: implemented
title: A run record with a torn multi-byte character still reads
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
- informed_by: review-result:main-fe93286-areas
scope:
- confidence: cited
  path: crates/takt/src/events.rs
- confidence: inferred
  path: crates/takt/tests/run_record_torn.rs
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-10-19T02:31:02Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-19T02:31:02Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-10-19T04:15:44Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"test_result":1}}}
---
## Outcome

`Events::read` reads a record line by line as bytes, skips a line that is not valid UTF-8 the way it skips a torn line, and names it, so one torn write never makes the whole record unreadable.

## Context

From the review of main at 9cc158b on 2026-10-19: ten independent reviewers, one per area, each finding then fact-checked by a second reviewer against the code (review-result:main-fe93286-areas), finding E8.1, confirmed: `events.rs:258@9cc158b` reads with `read_to_string`, which fails the whole file on invalid UTF-8 (Rust std: "If the contents of the file are not valid UTF-8, then an error will also be returned"); a write cut inside a multi-byte character on a full disk, the case the module names, leaves a record that `record_accepted` (`run.rs:3011`) and the scorecard (`scorecard.rs:470`) then fail on. Probe: `torn_utf8.py` in the fact-check's directory.

## Scope

- `crates/takt/src/events.rs`: `read` (cited)
- `crates/takt/tests/run_record.rs`: the row (inferred)

## Acceptance

A record of a valid line, then `{"summary":"gr\xc3` cut, then a newline and a valid line, reads as two events with one skipped line named; `takt scorecard` over it exits 0.
