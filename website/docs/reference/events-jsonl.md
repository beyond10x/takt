---
title: events.jsonl
description: The run record of a case, one JSON object per line, appended and never rewritten.
---

# `events.jsonl`

The run record of a case: `.engineering/cases/<case>/events.jsonl`, one JSON object per line,
appended and never rewritten. Every run on a case appends to the same file, so a case resumed after
an approval continues its record. Only takt writes it
([Protocol and case](../concepts/protocol-and-case.md#case)).

## Every line

| field | meaning |
|---|---|
| `at` | the instant the line was written, UTC, to the microsecond (`2026-10-19T01:02:03.456789Z`); older lines have whole seconds |
| `step` | the step it belongs to, 0 before a run's first |
| `event` | the event, one of those below, with its fields beside it |

A field added to an event later is optional, so older lines still read.

## Events

| `event` | fields | written when |
|---|---|---|
| `opened` | `case`, `protocol`, `story`?, `inputs` (`name=value`), `base`? (the commit the case's branch forked from), `binary`? (as `takt --version` prints it: `<sha>`, `<sha>-dirty` or `unknown revision`) | a run opened or resumed the case |
| `decision` | `seq`, `file` (the decision file in the case), `frontier` | Canon evaluated the case |
| `step` | `by` (`validator` or `harness`), `name`, `action`, `summary`, `tokens`?, `usage`? (`input`, `output`, `cache_read`, `cache_creation`), `revision`? (a validator's observed revision), `model`?, `cost_usd`?, `duration_ms`?, `api_ms`?, `turns`?, `started_at`? | a step ended |
| `effect` | `paths`, `revision`? | a judgement step committed files, or changed none |
| `evidence` | `id`, `kind`, `result`?, `path` | a validator wrote an evidence record under the case |
| `store_evidence` | `story`, `kind`, `reference`, `observed_at`, `commit`? | evidence was written on the story in the planning store |
| `store_move` | `story`, `to`, `commit`? | the story moved in the planning store |
| `refusal` | `by` (`takt` or `store`), `reason` (verbatim) | a step was discarded, or a write refused |
| `waiting` | `on` (actions), `seconds` | the run paused on the forge before observing again |
| `error` | `error` | the run ended on an error, or met one while recording an accepted case |
| `skipped` | `line`, `reason` | an earlier line was cut short and is skipped on read |
| `stop` | `stop`, `steps`, `tokens` | the run stopped |

`?` marks a field that is absent, never 0, when its source did not state it.

`takt trace` renders this file as a [`takt-trace/1`](./takt-trace.md) file, and `takt scorecard`
reads its runs from it.

Source: `crates/takt/src/events.rs`.
