---
title: takt-trace/1
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
| `sources`, `notes` | the files read, relative to the case directory or by file name, never an absolute path; what a steps file supplied |

## Operation

| field | meaning |
|---|---|
| `seq`, `step` | the operation's number from 1; the step it belongs to |
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

A record written before takt recorded the model, cost, turns and durations per call can be
completed from a runner shim's `steps.jsonl` (`--steps`); the header's notes say which fields it
supplied.

Source: `crates/takt/src/trace.rs`.
