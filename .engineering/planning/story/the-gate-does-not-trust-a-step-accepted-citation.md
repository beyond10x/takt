---
format: aep.planning-md/3
id: story:the-gate-does-not-trust-a-step-accepted-citation
kind: story
status: draft
title: gate.green does not hold on citation digests or specification inputs a step changed
summary: a step can add a needle row and run check-citations accept itself, and the citation gate passes
tags:
- rsi
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
revision: 1
---
# Story: the-gate-does-not-trust-a-step-accepted-citation

## Outcome

`gate.green` does not hold when a step changed what the specification and citation commands trust:
a digest recorded in `ess/domains/*.citations.tsv`, or a domain dropped from `ess/ess-inputs.yaml` or
`ess/system.yaml`. A digest says a person read the lines; a step is not that person.

## Context

Found by the review of story:the-gate-compares-its-scripts-and-tool-config-at-the-base
(2026-10-19): a step can write a needle row and run `scripts/check-citations accept` itself, and
`--require-digests` then exits 0; dropping `domains/takt.yaml` from `ess-inputs.yaml` makes
`ess specify validate --path ess` exit 0 while the file does not parse. The merge of `ess/**` still
needs the owner's grant, but the gate's evidence should not claim what nobody checked.

## Acceptance

- A step that adds or changes a digest in a citations file does not record `gate.green`: a test.
- A step that removes an entry from `ess/ess-inputs.yaml` or `ess/system.yaml` does not record
  `gate.green`: a test.
