---
format: aep.planning-md/3
id: story:takt-reads-harness-traces-as-trace-ir
kind: story
status: draft
title: takt reads harness session traces as trace-ir/1, the format the metaharness repository declares
summary: the report's harness layer has no reader for the session trace; metaharness owns trace-ir/1 as its ESS domain metaharness.trace (ADR 0096 § 7)
tags:
- rsi
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
- depends_on: story:harness-proposes-takt-performs
revision: 1
---
# Story: takt-reads-harness-traces-as-trace-ir

## Outcome

`takt report` reads the harness session layer's own trace, in the `trace-ir/1` format the
metaharness repository declares as its ESS domain `metaharness.trace`, and attributes that
layer's cost, durations and failures from it, each figure naming the trace record it came from;
until a session's trace is present the layer prints `not reported`, as it does today.

## Context

ADR 0101 attributes cost and failures to layers from each layer's own records; ADR 0096 § 7 makes
the metaharness repository the owner of the session trace format `trace-ir/1` (its ESS domain
`metaharness.trace`, on its main at 205125c). Wave 22's report (story:the-report-attributes-cost-
and-failures-to-layers) reads the model, harness-call and takt layers from takt's own records and
has no reader for the session trace. This story follows story:harness-proposes-takt-performs,
which runs the session through the metaharness library and so has the trace to read. Comes from
self-improvement work, so it carries `rsi`.

## Scope

- `crates/takt/src/report.rs` (the trace-ir reader), `crates/takt/src/trace.rs`,
  `ess/domains/takt.yaml` (the relation of a run's harness layer to its trace), tests and a
  fixture trace in the declared format.

## Acceptance

1. A run whose session wrote a `trace-ir/1` trace has its harness layer's tokens, durations and
   failures reported from that trace, each naming the trace record (file and record id), checked
   against a fixture trace that validates against the metaharness domain's schema.
2. A trace the reader cannot validate against that domain is refused for attribution, naming the
   file and the first failing field; the layer prints `not reported`, never a partial figure.
3. A run with no session trace is reported as today.
4. The trace format is pinned to the metaharness release takt depends on, so a format change is a
   pin bump, never a silent re-read.
