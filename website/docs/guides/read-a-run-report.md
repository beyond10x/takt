---
title: Read a run report
description: Turn a case's run record into a trace and a report of where its time and tokens went.
---

# Read a run report

Every case keeps its run record in `events.jsonl`. Two commands read it back.

```sh
takt trace --case .engineering/cases/<case> --out <case>.trace.jsonl
takt report --case .engineering/cases/<case>
takt report --trace a.trace.jsonl --trace b.trace.jsonl
```

`takt trace` writes one [`takt-trace/1`](../reference/takt-trace.md) file, one line per operation
whatever harness ran. It copies no summary or command text, so a trace can be published.

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

How to read the token and cost figures: [Methodology](../self-improvement/methodology.md).

## Older records

A case recorded before takt stored model, cost, turns and durations per call can be completed from
a runner shim's `steps.jsonl` (one `claude --output-format json` result per call); the trace's
notes name the fields it supplied:

```sh
takt trace --case <case> --steps <steps.jsonl>
takt report --case <case> --steps <steps.jsonl>
```

Sources: `takt trace --help`, `takt report --help`, `crates/takt/src/trace.rs`,
`crates/takt/src/report.rs`, `docs/metrics/README.md`.
