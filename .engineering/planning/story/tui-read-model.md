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
- depends_on: story:story-delivery-tests-first
- depends_on: story:scorecard-save-checks-binary
- depends_on: story:promotion-chain-head-anchored
- depends_on: story:check-finding-run-step-confined
- depends_on: story:self-improvement-protocol
- depends_on: story:judge-guard
scope:
- confidence: inferred
  path: ess/domains/takt.citations.tsv
- confidence: inferred
  path: ess/domains/takt.yaml
- confidence: inferred
  path: ess/ess-inputs.yaml
- confidence: inferred
  path: ess/http/takt.http.yaml
revision: 5
---
# Story: tui-read-model

## Outcome

takt's ESS model declares the read views a frontend needs, and an `ess-http/1` binding gives each its HTTP method and path, before any of them is served or drawn: the cases of a repository, one case with its latest decision (claims, actions, outcomes), its run events, the approvals pending, the scorecard and the journey of a run.

## Context

epic:terminal-frontend, step 1 (spec first). The nouns exist in `ess/domains/takt.yaml` (Case, Run, RunEvent, Decision, Grant and others). Missing: the views over them, a component that serves them with `reached_by: network` (`ess/domains/takt.yaml` has no `reached_by` today: `grep -c reached_by` prints 0), and the HTTP binding, which in ESS 0.60.0 is a separate `ess-http/1` document (`ess specify http validate --path <spec> --binding <file>`). `ess ui check --model` and `ess ui run --model` resolve through that component. Follow the `ess:specifying` skill; every relation or cardinality not read from code is `UNMAPPED:`.

Each view names the files it reads, all under the repository given to `takt serve`: cases from `.engineering/cases/*/case.yaml`; case detail from the latest `decisions/` file; run events from `events.jsonl`; approvals pending from cases whose latest decision is `ApprovalRequired`, with the capability and bound revision; scorecard and journey from the case's evidence and `docs/metrics/*.jsonl`. The promotion store outside the repository (`~/.local/state/takt/promotions/`) is not read.

No command is declared: the epic's frontend writes nothing.

## Expected before

On takt at main, holds; after the change, fails:

- `grep -c reached_by ess/domains/takt.yaml` prints 0.

## Scope

- `ess/domains/takt.yaml`: six views and the serving component (inferred)
- `ess/http/takt.http.yaml`: the `ess-http/1` binding (inferred; the path follows the ESS example the developer cites)
- `ess/ess-inputs.yaml`: the binding listed if ESS requires it (inferred)
- `ess/domains/takt.citations.tsv`: rows for new `read from:` citations (inferred)

## Acceptance

`ess specify validate --path ess` passes with six declared views and no command added; `ess specify http validate --path ess --binding ess/http/takt.http.yaml` passes with one GET route per view and no other method; and `python3 -I scripts/check-citations --require-digests` resolves every citation.
