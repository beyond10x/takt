---
format: aep.planning-md/3
id: story:run-records-harness-model
kind: story
status: draft
title: Every run records the model each harness step ran on
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
scope:
- confidence: inferred
  path: crates/takt/src/events.rs
- confidence: inferred
  path: crates/takt/src/harness.rs
revision: 2
---
# Story: run-records-harness-model

## Outcome

Every run records the model each harness step ran on, as the harness reports it, in the step's event; a run started with `--model` records the requested model beside the reported one, and a scorecard or comparison can group runs by model.

## Context

Trial round 3 on scenario 1 (2026-10-18): with no `--model` given, takt runs T-11 and T-13 ran every model call on claude-fable-5-1 and T-12 switched from fable to opus mid-run, while the other arms ran on opus; the cause is not known. Only a shim outside takt (the step JSON's `modelUsage`) showed it; takt's own events carry no model, so a cost comparison across rounds was silently invalid. product-requirements:takt R9 lists the model version among a run's pinned inputs.

## Expected before

On takt at main, holds; after the change, fails:

- `grep -c model .engineering/cases/<case>/events.jsonl` prints 0 for any run.

## Scope

- `crates/takt/src/harness.rs`: read the reported model per step (inferred)
- `crates/takt/src/events.rs`: the step event's model fields (inferred)

## Acceptance

A run's every harness-step event names the model the harness reported for it and, when `--model` was given, the requested one; a step whose reported model differs from the requested one is visible in `takt scorecard`.
