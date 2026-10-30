---
title: events.jsonl
sidebar_position: 5
description: The run record of a case, one JSON object per line, appended and never rewritten.
---

# `events.jsonl`

The run record of a [case](/docs/concepts/glossary#case): `.takt/cases/<case>/events.jsonl`, one JSON object per line,
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
| `opened` | `case`, `protocol`, `story`?, `inputs` (`name=value`), `base`? (the commit the case's branch forked from), `binary`? (as `takt --version` prints it: `<sha>`, `<sha>-dirty` or `unknown revision`), `layers`? (layer name to revision, read from what ran and never typed: `takt`, the binary's revision; written, empty or not, by every run from 0.6.0 on; a line without it reads as layers not recorded), `confinement`? (`none` or `substrate`, the adapter every child of the run starts through), `substrate`? (the [Substrate](/docs/concepts/glossary#substrate) adapter the test command runs through: `adapter`, `inner`? (the one `systemd` wraps), `families`, `facts`, and `declared_insecure`? with its reason under `none`; a line without it was written before it), `endpoint`? (a `--harness harness` run only: `base_url`, `wire`, `model` and the credential by reference, a token file's path as `~/…` under HOME and its pointer, a secret's namespace and name, or a variable's name; never a token), `records`? (`default` or `trial`, the profile the case keeps its decisions under; a scenario run or an improvement case says `trial` whatever was asked; a line without it was written before the profile and reads as every decision kept), `scan`? (the scan adapter, `unset` or `none`; a line of 0.23.0 or earlier may say `gates`) | a run opened or resumed the case |
| `decision` | `seq`, `file` (the decision file in the case), `frontier` | [Canon](/docs/concepts/glossary#canon) evaluated the case |
| `step` | `by` (`validator` or `harness`), `name`, `action`, `summary`, `tokens`?, `usage`? (`input`, `output`, `cache_read`, `cache_creation`), `revision`? (a [validator](/docs/concepts/glossary#validator)'s observed revision), `model`?, `version`? (the [harness](/docs/concepts/glossary#harness) program's own version, as its stream stated it), `cost_usd`?, `duration_ms`?, `api_ms`?, `turns`?, `started_at`?, `transcript`? (the step's transcript, relative to the transcripts directory: `<repo>/<case>/run-<r>/step-<n>.jsonl`), `transcript_refused`? (why takt kept none, such as a file already at that path or a link on the way) | a step ended |
| `effect` | `paths`, `revision`? | a judgement step committed files, or changed none |
| `evidence` | `id`, `kind`, `result`?, `path` | a validator wrote an [evidence](/docs/concepts/glossary#evidence) record under the case |
| `store_evidence` | `story`, `kind`, `reference`, `observed_at`, `commit`? | evidence was written on the [story](/docs/concepts/glossary#story) in the planning store |
| `store_move` | `story`, `to`, `commit`? | the story moved in the planning store |
| `refusal` | `by` (`takt` or `store`), `reason` (verbatim) | a step was discarded, or a write refused |
| `decline` | `reason` (the step's summary), `case_revision` (`r<n>`) | a judgement step named `decline`, changing no file; a record line, not evidence |
| `waiting` | `on` (actions), `seconds` | the run paused on the [forge](/docs/concepts/glossary#forge) before observing again |
| `error` | `error` | the run ended on an error, or met one while recording an accepted case |
| `skipped` | `line`, `reason` | an earlier line was cut short and is skipped on read |
| `stop` | `stop` (the stop as printed: its name, then its detail), `steps`, `tokens`, `completed`? (whether the stop is a completion, as the binary that stopped knows its stops; a reader that does not know the stop takes this word for it) | the run stopped |

`?` marks a field that is absent, never 0, when its source did not state it.

## The stop line

`stop` opens with the stop's name, followed by its detail in parentheses. 0.14.0 added no event
kind: its two new stops are `stop` lines like the others, and so is 0.19.0's
`RepeatedGateFailure`.

| name | the run stopped because | exit |
|---|---|---|
| `Completed` | the [protocol](/docs/concepts/glossary#protocol)'s outcome is legitimate, `Completed (<outcome>)`; the only stop with `completed: true` | 0 |
| `Iterations`, `Hours`, `Tokens`, `ActiveTime` | the budget ended | 3 |
| `NothingAdmissible`, `StepRefused`, `NoProgress` | no admissible step, a step refused, or nothing changed | 3 |
| `RedBase` | the case's first `tests.run` failed before any step changed the base | 3 |
| `RepeatedTestFailure` | two `tests.run` results in a row, each at a new revision, failed the same tests ([what counts](../concepts/how-a-run-ends.md#what-counts-as-an-attempt)) | 3 |
| `RepeatedGateFailure` | three `gate.run` results in a row, each at a new revision, failed the same gates (0.19.0) | 3 |
| `HarnessUnqualified`, `SetupFailed` | the harness binary does not qualify under the run's hermetic mode, or a `setup` command of the run file did not end 0 | 1 |
| `ApprovalRequired` | a capability needs a [grant](/docs/concepts/glossary#grant) (`takt approve`) | 4 |
| `Waiting` | the forge or a local file the run waits on is not ready and the budget ended | 6 |
| `Unperformable` | the only action that decides a claim has no performer in this run | 7 |
| `DiskLow` | less free disk than `--min-free-gib` | 8 |
| `Declined` | a judgement step declined the story, `Declined (<reason>)`, after a `decline` line | 9 |

A run that ends on an error ends with an `error` line and no `stop` line, and exits 5. A red base as `takt run` writes it:

```json
{"at":"2026-10-26T23:11:31.321393Z","step":1,"event":"stop","stop":"RedBase (the tests fail at the case's base 3f0ec13b, before any step changed it: the output names no failed test)","steps":1,"tokens":0,"completed":false}
```

[How a run ends](../concepts/how-a-run-ends.md) says what each stop means and what to do.
The exit codes are in [Exit codes](./exit-codes.md).

`takt trace` renders this file as a [`takt-trace/1`](./takt-trace.md) file, and `takt scorecard`
reads its runs from it.

Source: `crates/takt/src/events.rs`.
