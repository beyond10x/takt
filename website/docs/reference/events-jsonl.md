---
title: events.jsonl
description: The run record of a case, one JSON object per line, appended and never rewritten.
---

# `events.jsonl`

The run record of a case: `.engineering/cases/<case>/events.jsonl`, one JSON object per line,
appended and never rewritten. Every run on a case appends to the same file, so a case resumed after
its gate continues its record. What a run prints is for the person watching it; this file is what a
person, or a program, reads back afterwards.

The case directory is one a judgement step and a validator's command may not write: a line in it
was written by takt.

## Every line

| field | meaning |
|---|---|
| `at` | the instant the line was written, UTC, to the microsecond (`2026-10-19T01:02:03.456789Z`); lines written before had whole seconds |
| `step` | the step it belongs to, 0 before a run's first |
| `event` | the event, one of those below, with its fields beside it |

A field added to an event later is optional: a line written before it reads as it did.

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
| `refusal` | `by` (takt or the store), `reason` (verbatim) | a step was discarded, or a write refused |
| `waiting` | `on` (actions), `seconds` | the run paused on the forge before observing again |
| `error` | `error` | the run ended on an error, or met one while recording an accepted case |
| `skipped` | `line`, `reason` | an earlier line was cut short and is skipped on read |
| `stop` | `stop`, `steps`, `tokens` | the run stopped |

`?` marks a field absent when the source did not state it. A harness step's model, cost, duration
and turns are each absent when the harness did not report them, never 0.

`takt trace` renders this file as a [`takt-trace/1`](./takt-trace.md) file, and `takt scorecard`
reads its runs from it.

Source: `crates/takt/src/events.rs`.
