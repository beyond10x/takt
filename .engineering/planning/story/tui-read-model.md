---
format: aep.planning-md/3
id: story:tui-read-model
kind: story
status: draft
title: takt's ESS model declares the read views a frontend needs, with their HTTP surface
summary: six views (cases, case detail, run events, approvals pending, scorecard, journey) declared before any is served
relations:
- decomposes: epic:terminal-frontend
- serves: vision:safe-agentic-coding
scope:
- confidence: inferred
  path: ess/domains/takt.citations.tsv
- confidence: inferred
  path: ess/domains/takt.yaml
revision: 3
---
# Story: tui-read-model

## Outcome

takt's ESS model declares the read views a frontend needs, with their HTTP surface, before any of them is served or drawn: the cases of a repository, one case with its latest decision (claims, actions, outcomes), its run events, the approvals pending, the scorecard and the journey of a run.

## Context

epic:terminal-frontend, step 1 (spec first). The nouns exist in `ess/domains/takt.yaml` (Case, Run, RunEvent, Decision, Grant and others, some added by the ESS hardening of 2026-10-18); what is missing is the views over them and the component that serves them (`reached_by: network`), which `ess ui check --model` and `ess ui run --model` resolve. Follow the `ess:specifying` skill; every relation or cardinality not read from code is `UNMAPPED:`.

## Expected before

On takt at main, holds; after the change, fails:

- `ess specify http validate` (or the 0.60.0 equivalent) over `ess/` lists no view named for cases or case detail.

## Scope

- `ess/domains/takt.yaml`: views and their HTTP surface (inferred)
- `ess/domains/takt.citations.tsv`: rows for new `read from:` citations (inferred)

## Acceptance

`ess specify validate --path ess` passes with six declared views (case list, case detail, run events, approvals pending, scorecard, journey), each with its HTTP method and path and the component that serves it, and `python3 -I scripts/check-citations --require-digests` resolves every citation.
