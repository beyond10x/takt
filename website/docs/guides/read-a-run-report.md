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

`takt trace` writes one `takt-trace/1` file: a header line on the run, then one line per operation
(a step, a wait on the forge, a refusal, a stop, the resume after one) in one vocabulary, whatever
harness ran ([reference](../reference/takt-trace.md)). Summaries and command text are classified,
never copied, so a trace carries no local path and can be published.

`takt report` prints Markdown:

1. **One table across the runs given**: protocol, story, outcome, wall seconds, steps, model calls,
   turns, tokens, cost, and the wall time split by actor (validator, forge, idle, human, harness,
   API, other), with the CI reads.
2. **Per run**: the actors' shares of the wall time, then one row per step (actor, action,
   seconds, turns, tokens, cost).
3. **Defects** the report recognises, each with its step, seconds and tokens: a model call that
   only asks for the merge approval, a pull request opened before any file changed, CI read a
   second time, a `repository.inspect` step followed by `NoProgress`, a repeated failure.
4. **Not recorded**: every field the sources did not state. A missing figure is printed as `-`,
   never as 0.
5. **Sources**: the files each figure came from.

## Records written before takt recorded usage per call

Cases written before a harness step carried its model, cost, turns and durations can be completed
from a runner shim's `steps.jsonl` (one `claude --output-format json` result per call):

```sh
takt trace --case <case> --steps <steps.jsonl>
takt report --case <case> --steps <steps.jsonl>
```

The trace's notes say which fields the shim supplied.

## Token and cost figures

Token and cost figures come only from the raw usage the harness reported. Token totals include
cache reads, which are billed at about 1/20 of input, so compare cost beside tokens, not tokens
alone. Cost is the API list price the harness reports: notional.

Sources: `takt trace --help`, `takt report --help`, `crates/takt/src/trace.rs`,
`docs/metrics/README.md`.
