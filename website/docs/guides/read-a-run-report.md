---
title: Read a run report
sidebar_position: 10
description: Turn a case's run record into a trace and a report of where its time and tokens went.
lede: takt trace and takt report read a case's events.jsonl back as one trace per run and a table of where the wall time and the tokens went.
source: takt trace --help and takt report --help at 0.14.0; crates/takt/src/trace.rs; crates/takt/src/report.rs; docs/metrics/README.md; takt trace and takt report on a story case of takt 0.14.0, 2026-10-26
---

# Read a run report

Every case keeps its run record in `events.jsonl`. Two commands read it back.

```sh
takt trace --case .engineering/cases/<case> --out <case>.trace.jsonl
takt report --case .engineering/cases/<case>
takt report --trace a.trace.jsonl --trace b.trace.jsonl
```

```console
$ takt trace --case .engineering/cases/story-add-sub-20261026T231852Z --out runs/add-sub.trace.jsonl
written: runs/add-sub.trace.jsonl (8 operations, takt-trace/1)
```

`takt trace` writes one [`takt-trace/1`](../reference/takt-trace.md) file, one line per operation
whatever harness ran. It copies no summary or command text, so a trace can be published.
`takt view` draws traces on a time axis ([Read a case](./read-a-case.md#what-the-model-was-told-and-did)).

`takt report` prints Markdown:

1. **One table across the runs given**: protocol, story, outcome, wall seconds, steps, model calls,
   turns, tokens, cost, and the wall time split by actor (validator, forge, idle, human, harness,
   API, other), with the CI reads.
2. **Per run**: the actors' shares of the wall time, one row per step (actor, action, seconds,
   turns, tokens, cost), then the defects the report recognises (a model call that only asks for
   the merge approval, a pull request opened before any file changed, CI read a second time, a
   `repository.inspect` step followed by `NoProgress`, a repeated failure), each with its step,
   seconds and tokens; the fields the sources did not state (`-`, never 0); and the files each
   figure came from.

A run's step table, from the story case of [Run a story](./run-a-story.md):

```text
| step | actor | action | seconds | harness s | turns | tokens | cost $ |
|---|---|---|---|---|---|---|---|
| 1 | validator | tests.run | 0.2 | - | - | - | - |
| 2 | validator | gate.run | 0.8 | - | - | - | - |
| 3 | harness | implementation.edit | 16.2 | 14.3 | 1 | 60817 | - |
| 4 | validator | tests.run | 0.2 | - | - | - | - |
| 5 | validator | gate.run | 0.7 | - | - | - | - |
| 6 | harness | change.handover | 6.5 | 5.1 | 1 | 15122 | - |
| 7 | validator | change.read | 0.1 | - | - | - | - |
```

Cost is `-` there: the record states none, and a figure it lacks is never 0. How to read the token and
cost figures:
[Methodology](../self-improvement/methodology.md).

## Older records

A case recorded before takt stored model, cost, turns and durations per call can be completed from
a runner shim's `steps.jsonl` (one `claude --output-format json` result per call); the trace's
notes name the fields it supplied:

```sh
takt trace --case <case> --steps <steps.jsonl>
takt report --case <case> --steps <steps.jsonl>
```
