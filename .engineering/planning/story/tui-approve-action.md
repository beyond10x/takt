---
format: aep.planning-md/3
id: story:tui-approve-action
kind: story
status: draft
title: A person grants a pending approval from the terminal frontend
summary: 'the one write: approve through takt::case::grant as human:<id>, refused for any other approver'
relations:
- decomposes: epic:terminal-frontend
- serves: vision:safe-agentic-coding
- depends_on: story:takt-tui-crate
scope:
- confidence: inferred
  path: crates/takt-serve
- confidence: inferred
  path: ess/domains/takt.yaml
- confidence: inferred
  path: ess/ui/takt.ui.yaml
revision: 3
---
# Story: tui-approve-action

## Outcome

A person grants a pending approval from the terminal frontend: the approvals page lists each case at `ApprovalRequired` with the capability and the revision it would bind to, and the approve command writes the grant through `takt::case::grant` under the person's identity, with the same checks `takt approve` applies.

## Context

epic:terminal-frontend, step 5, last because it is the one write. Depends on story:takt-tui-crate. The command is declared in the model and the UI document first, then served by `takt serve` as its only write route, bound to localhost, refusing any grant whose approver is not `human:<id>` taken from the person running the app (never from the request body), and refusing a judge-surface capability unless story:judge-guard's rules admit it.

## Expected before

On takt at main, holds; after the change, fails:

- The approvals page of `ess/ui/takt.ui.yaml` declares no command.

## Scope

- `ess/domains/takt.yaml`: the approve command (inferred)
- `ess/ui/takt.ui.yaml`: the approve action (inferred)
- the read-only server: one write route (inferred)

## Acceptance

From the approvals page, approving a case at `ApprovalRequired (repository.merge)` writes a grant at the case's bound revision with approver `human:<id>`, after which the next `takt run` on that case merges; a request naming another approver is refused with nothing written, and `ess verify conform` covers the command.
