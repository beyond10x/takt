---
format: aep.planning-md/3
id: story:tui-ui-document
kind: story
status: draft
title: The terminal frontend as an ess-ui/1 document with fixtures and headless tests
summary: ess/ui/takt.ui.yaml checked against the model, fixtures from real cases, ess-ui-test passing
relations:
- decomposes: epic:terminal-frontend
- serves: vision:safe-agentic-coding
- depends_on: story:tui-read-model
scope:
- confidence: inferred
  path: ess/ui/fixtures
- confidence: inferred
  path: ess/ui/takt.ui.yaml
- confidence: inferred
  path: ess/ui/tests
revision: 3
---
# Story: tui-ui-document

## Outcome

The terminal frontend exists as an `ess-ui/1` document, `ess/ui/takt.ui.yaml`, before any code: pages for the case list, case detail (frontier and evidence), run events, approvals pending, scorecard and journey; navigation between them; fixtures read from real recorded cases; and `ess-ui-test/1` tests that pass headless against the terminal renderer.

## Context

epic:terminal-frontend, step 2. Depends on story:tui-read-model for the views the document reads. The worked example of every construct is beyond10x/ess `examples/partner-portal/ui.yaml` with its `fixtures/` and `tests/`. Fixtures come from the scenario 1 cases recorded on 2026-10-18 (docs/runs/2026-10-18-improvement-turn-1/case/ and the timed takt runs), copied, never edited by hand.

## Expected before

On takt at main, holds; after the change, fails:

- `ls ess/ui/takt.ui.yaml` exits non-zero.

## Scope

- `ess/ui/takt.ui.yaml`, `ess/ui/fixtures/**`, `ess/ui/tests/**` (inferred)

## Acceptance

`ess ui check --model ess ess/ui/takt.ui.yaml` reports no error, `ess ui test` passes every test in `ess/ui/tests/` headless, and `ess ui run --tui --path ess/ui/takt.ui.yaml` shows the improvement turn's case at outcome `accepted` from its fixture.
