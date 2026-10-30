---
title: Read a run report
sidebar_position: 10
description: Turn a case's run record into a trace and a report of where its time and tokens went.
lede: takt trace and takt report read a case's events.jsonl back as one trace per run and a table of where the wall time and the tokens went.
source: takt trace --help and takt report --help at 0.14.0; crates/takt/src/trace.rs; crates/takt/src/report.rs; docs/metrics/README.md; takt trace and takt report on a story case of takt 0.14.0, 2026-10-26; takt report --cases on crates/takt/tests/fixtures/report-cases, 2026-10-30
---

# Read a run report

Every case keeps its run record in `events.jsonl`. Two commands read it back.

```sh
takt trace --case .takt/cases/<case> --out <case>.trace.jsonl
takt report --case .takt/cases/<case>
takt report --trace a.trace.jsonl --trace b.trace.jsonl
```

```console
$ takt trace --case .takt/cases/story-add-sub-20261026T231852Z --out runs/add-sub.trace.jsonl
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

## Sum many cases by repository and day

`takt report --cases` takes cases directories instead of runs, `<repo>/.takt/cases` or the older
`<repo>/.engineering/cases`, one or more, and prints one row per repository and UTC day, then a total:

```console
$ takt report --cases a/.engineering/cases b/.takt/cases
| repository | day (UTC) | units accepted | tokens | interventions |
|---|---|---:|---:|---:|
| a | 2026-10-01 | 1 | 3000 | 0 |
| a | 2026-10-02 | 1 | 500 | 1 |
| b | 2026-10-02 | 0 | unknown | 0 |
| total | | 2 | unknown | 1 |

tokens `unknown`: a harness step of a run that day recorded no tokens, or 0, which is never read as 0.
```

- **units accepted**: cases whose last `stop` line is `Completed (accepted)`, counted on the day of
  the run that wrote it. A scenario's held-out result does not change it.
- **tokens**: the harness's `tokens` on the `step` lines of the runs opened that day, read as
  `takt scorecard` reads them. A harness step with no count, or 0, makes the figure `unknown`;
  the `stop` line's total is never used in its place. A case with no run record (no
  `events.jsonl`, or none with an `opened` line) makes its repository's tokens `unknown` on the
  day `unknown`, and a note after the table names it.
- **interventions**: entries of each case's `authority.yaml` whose approver is `human:<name>`
  with a name `takt approve` accepts, granted or denied, on the day they were given. A grant by
  an agent role is not one, and neither is `human:` with no name.

A case is a directory in the cases directory holding `case.yaml` or `events.jsonl`; other
directories and files there are skipped. A cases directory holding no case, or holding a
symbolic link, is refused with exit 1, naming it. A cases directory given twice, however spelled,
is read once, under the first spelling.

The repository is the directory above `.takt/cases` or `.engineering/cases`, or the argument as
given when it ends in neither. The output is text only: `--format json` beside `--cases` is refused.

## Older records

A case recorded before takt stored model, cost, turns and durations per call can be completed from
a runner shim's `steps.jsonl` (one `claude --output-format json` result per call); the trace's
notes name the fields it supplied:

```sh
takt trace --case <case> --steps <steps.jsonl>
takt report --case <case> --steps <steps.jsonl>
```
