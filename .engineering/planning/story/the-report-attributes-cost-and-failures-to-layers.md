---
format: aep.planning-md/3
id: story:the-report-attributes-cost-and-failures-to-layers
kind: story
status: draft
title: takt report attributes cost and failures to the layer that spent or failed
summary: 'ADR 0101: a report says which layer a cost or a failure belongs to, from the records each layer writes'
tags:
- rsi
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
revision: 1
---
# Story: the-report-attributes-cost-and-failures-to-layers

## Outcome

`takt report` attributes each run's cost (tokens, money, wall time) and each failure to the layer
it belongs to: the model layer's usage, the harness session layer's trace, the runtime's decision
records, takt's own validators, so a proposal for improvement names the layer to change.

## Context

ADR 0101 (2026-10-20): cost and failures are attributed across layers from their own records
(the session trace format `trace-ir/1`, the model layer's usage records, the runtime's decision
records), never inferred from a model's text. Comes from self-improvement work, so it carries
`rsi`.

## Scope

- `crates/takt/src/report.rs`, `crates/takt/src/trace.rs`, readers for the other layers'
  records, `ess/domains/takt.yaml`, tests.

## Acceptance

1. `takt report` prints, per run and in total, cost by layer and failures by layer; a figure a
   layer's record does not state is shown as not reported, never 0.
2. Each attribution names the record it came from (file and line or id).
3. A run whose layers wrote no records is reported as before, with the layers column "not
   recorded".
