---
format: aep.planning-md/3
id: story:a-trial-records-the-revision-of-each-layer
kind: story
status: draft
title: A trial records the revision of each layer it ran in its record
summary: the trial agent writes the layers it ran beside the record; ADR 0101 makes them part of it
tags:
- rsi
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
revision: 1
---
# Story: a-trial-records-the-revision-of-each-layer

## Outcome

A scenario run's record names the revision of each layer the run used (takt itself, the harness
session layer, the model layer, the runtime, the tools), so a scorecard and a promotion can say
which combination they measured. The trial agent's separate layers file becomes part of the
record and is no longer needed beside it.

## Context

ADR 0101 (2026-10-20): self-improvement measures the stack, not takt alone; a trial must say
which revision of each layer it ran. The trial agent already writes the layers it ran to a file
beside its cycle. Comes from self-improvement work, so it carries `rsi`.

## Scope

- `crates/takt/src/run.rs` (the run record), `crates/takt/src/scorecard.rs` (reads it),
  `ess/domains/takt.yaml` (the record's new field, spec first), their tests.

## Acceptance

1. The run record of a scenario run holds a `layers` map, layer name to revision, each read from
   the binary or library that ran, never typed by a person; ESS declares it first.
2. `takt scorecard` prints the layers of the runs it scores and refuses to score runs whose
   layers differ, naming the layer, unless asked to compare across them.
3. A record without `layers` (written before this story) is read as "layers not recorded", not as
   any revision.
