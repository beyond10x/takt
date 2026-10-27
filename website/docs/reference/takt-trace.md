---
title: takt-trace/1
sidebar_position: 6
description: A case's run record rendered as one file in one vocabulary of operations, whatever harness ran.
---

# `takt-trace/1`

`takt trace --case <dir>` renders a case's [`events.jsonl`](./events-jsonl.md) as one JSON Lines
file: a header line on the run, then one line per operation. An operation is a step, a wait on the
forge, a refusal, a stop, or the resume after one.

A step runs from the line before it to its own line; a wait from its line to the next; a stop is
an instant, and the resume after it spans the gap. Token and cost figures come from the raw usage the harness reported and from nothing else: a
figure no source states is absent, never 0. Summaries and command text are classified, never
copied, so a trace carries no local path and can be published.

## Header

| field | meaning |
|---|---|
| `type` | `header` |
| `format` | `takt-trace/1` |
| `run`, `case` | the run's name (the case id, or `--run`) and the case id |
| `arm` | `takt` |
| `harness`, `model` | the harness's name from its steps; the models the calls reported, distinct, in order |
| `revision`, `binary` | takt's revision when the binary named one; the binary as `opened` named it |
| `protocol`, `story`, `scenario` | the scenario comes from a case id `scenario-<id>-trial-<n>` |
| `t0`, `t_end`, `wall_s` | the first `opened` line's instant, the last line's, and the seconds between |
| `stop`, `outcome` | the last stop verbatim, and the last run's outcome as the scorecard names it |
| `invocations`, `resumes`, `steps` | the `opened` lines, how many resumed the case, the steps of every run |
| `tokens` | `input`, `output`, `cache_read`, `cache_write`, `sum`: each absent when some call cannot join it |
| `cost_usd` | the calls' cost summed, when every call states one |
| `acceptance` | the held-out check; never in the record |
| `layers` | the run's layers as the scorecard reads them: the first `opened` line's `layers`, the `harness` layer from the versions the harness steps' lines state, the `model` layer from the models the calls reported; absent when the record has no `layers` (layers not recorded) |
| `sources`, `notes` | the files read, relative to the case directory or by file name, never an absolute path; what a steps file supplied |

## Operation

| field | meaning |
|---|---|
| `type` | `op` |
| `seq`, `step` | the operation's number from 1; the step it belongs to |
| `parent`, `tool_class` | reserved for a tool call inside a model call; never written yet |
| `actor` | `harness` (one model call), `validator` (the tests, the gates), `forge` (the pull request, its checks, the merge), `takt` (a refusal, a stop, the remainder of the wall time), `human` (from a stop to the resume after it), `idle` (a fixed sleep on the forge) |
| `intent` | `inspect`, `change`, `verify`, `research`, `wait`, `coordinate` |
| `kind` | the journey vocabulary (`model_call`, `read`, `edit`, `test`, `gate`, `push`, `pr_open`, `ci_wait`, `ci_pass`, `approval_wait`, `refusal`, `no_progress`, `resume`, `other`) plus `tool` |
| `action`, `name` | the protocol action; the validator's or harness's name, or who refused |
| `t_start`, `t_end` | seconds from `t0`, to the millisecond; absent where the record has no instant |
| `tokens`, `tokens_sum`, `model`, `cost_usd`, `turns`, `duration_ms`, `api_ms` | what the harness reported about the call |
| `outcome` | `pass`, `fail`, `pending`, `opened`, `pushed`, `merged`, `changed`, `unchanged`, `nothing`, `refused`, `approval`, `error`; for a stop, the outcome as the scorecard names it |
| `paths` | the paths a judgement step committed |
| `evidence` | `observed` (a record or exit status a validator observed) or `declared` (the model's own text) |
| `label` | a pattern mark: `approval-only call`, `seed pr.open`, `2nd CI`, `inspect, then NoProgress`, `repeated failure (as step N)` |
| `line` | the line of the case's `events.jsonl`, from 1, the operation was read from: the record `takt report` names for a figure (`events.jsonl:<n>`) |

## An example

The trace of the one-step case from [Case files](./case-files.md), `takt trace --case <case dir>`:
the header, then its two operations, one line each.

```json
{"type":"header","format":"takt-trace/1","run":"takt-20261026T231053Z","case":"takt-20261026T231053Z","arm":"takt","harness":null,"model":null,"revision":"9293a4b28f13a62545e51be1bca1327f552dacfb","binary":"9293a4b28f13a62545e51be1bca1327f552dacfb","protocol":"software.change/1","story":null,"scenario":null,"t0":"2026-10-26T23:10:53.956920Z","t_end":"2026-10-26T23:10:54.094498Z","wall_s":0.137,"stop":"Iterations (1)","outcome":"Iterations","invocations":1,"resumes":0,"steps":1,"tokens":{"input":null,"output":null,"cache_read":null,"cache_write":null,"sum":0},"cost_usd":null,"acceptance":null,"layers":{"takt":"9293a4b28f13a62545e51be1bca1327f552dacfb"},"sources":["events.jsonl"],"notes":[]}
{"type":"op","seq":1,"step":1,"actor":"validator","intent":"verify","kind":"test","action":"tests.run","name":"takt.tests-run","t_start":0.014,"t_end":0.108,"outcome":"pass","evidence":"observed","line":3}
{"type":"op","seq":2,"step":1,"actor":"takt","intent":"coordinate","kind":"other","t_start":0.137,"t_end":0.137,"outcome":"Iterations","evidence":"observed","line":6}
```

`takt trace` exits 0 when it wrote the file; 1 when the case has no `events.jsonl`, a file does not
read or the output cannot be written; 2 when the arguments do not parse.

A record written before takt recorded the model, cost, turns and durations per call can be
completed from a runner shim's `steps.jsonl` (`--steps`); the header's notes say which fields it
supplied.

Source: `crates/takt/src/trace.rs`.
