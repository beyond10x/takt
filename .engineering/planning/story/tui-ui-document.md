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
revision: 6
---
# Story: tui-ui-document

## Outcome

The terminal frontend exists as an `ess-ui/1` document, `ess/ui/takt.ui.yaml`, before any code: pages for the case list, case detail (frontier and evidence), run events, approvals pending, scorecard and journey; navigation between them; fixtures copied from real recorded cases; and `ess-ui-test/1` tests that pass headless against the terminal renderer. The document declares no command.

## Context

epic:terminal-frontend, step 2. Depends on story:tui-read-model for the views. The worked example of every construct is beyond10x/ess `examples/partner-portal/ui.yaml` with its `fixtures/` and `tests/`.

Fixture sources, copied and never edited by hand:
- case, decisions, evidence, authority: `docs/runs/2026-10-18-improvement-turn-1/case/` and `docs/runs/2026-10-19-improvement-turns-2-and-3/cases/`.
- run events: no `events.jsonl` is committed under `docs/runs/` (`find docs/runs -name events.jsonl` prints nothing). Record one from a `takt run` in a scratch repository and copy it into `ess/ui/fixtures/`, naming the takt revision that wrote it in the commit message.
- an approval pending: a case stopped at `ApprovalRequired (repository.merge)` from the same scratch run.

The run events page and the case detail are `live:` sections with a `refresh:` (polled; the served surface streams nothing).

The approvals page shows, per pending approval, the case, the capability, the bound revision and the text `takt approve --repo <repo> --by <you> <case> <capability>` (epic decision: no write route).

## Expected before

On takt at main, holds; after the change, fails:

- `ls ess/ui/takt.ui.yaml` exits non-zero.

## Scope

- `ess/ui/takt.ui.yaml`, `ess/ui/fixtures/**`, `ess/ui/tests/**` (inferred)

## Acceptance

- `ess ui check --path ess/ui/takt.ui.yaml --model ess` exits 0 with no error finding;
- `ess ui test --path ess/ui/takt.ui.yaml ess/ui/tests/*.yaml` exits 0, with one test per page: the case list names an improvement turn's case at outcome `accepted`; a case's detail page shows its claims, actions and outcomes from its latest decision; the run events page shows the fixture case's events; the approvals page shows the fixture's pending case with its `takt approve` line; the scorecard page and the journey page each show the fixture run's figures;
- `grep -c "command:" ess/ui/takt.ui.yaml` prints 0.
