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
  path: ess/http/takt.http.yaml
- confidence: inferred
  path: ess/ui/takt.ui.yaml
revision: 5
---
# Story: tui-approve-action

## Outcome

A person grants a pending approval from the terminal frontend, and no process takt starts for a case can do the same: the approve command writes the grant through `takt::case::grant` under the identity fixed when the person started `takt tui`, with the checks `takt approve` applies.

## Context

epic:terminal-frontend, deferred step 5. Blocked by dependency-blocker:approve-route-credential; the epic's decision of 2026-10-19 says why.

Before it can be built, both must hold:
1. The generated app takes its credential from somewhere a process of the same user cannot read: ess 0.60.0 reads it only from the environment variable `ESS_UI_AUTHORIZATION` (`crates/ui/ess-ui-tui/src/generate.rs:9,226`), and `/proc/<pid>/environ` is readable by the same user. Needs an ESS release that takes it from an inherited file descriptor or a prompt, or a takt-side design that shows the same.
2. Code an agent wrote cannot reach the write route: the test command keeps the host network today (story:test-command-reaches-no-forge-login, stated not done). Either the route is not on the network (ESS 0.60.0 accepts `http://` base URLs only, `http.rs:171-172`) or test commands run without the host network.

Then: the command is declared in the model, the `ess-http/1` binding and the UI document; `takt serve` serves it only when started by `takt tui` with a per-launch credential; the approver is `human:<id>` from `takt tui --by`, never from the request; a judge-surface capability is refused unless story:judge-guard's rules admit it.

## Expected before

On takt at main, holds; after the change, fails:

- The approvals page of `ess/ui/takt.ui.yaml` declares no command.

## Scope

- `ess/domains/takt.yaml`, `ess/http/takt.http.yaml`: the approve command (inferred)
- `ess/ui/takt.ui.yaml`: the approve action (inferred)
- the server: one write route (inferred)

## Acceptance

From the approvals page, approving a case at `ApprovalRequired (repository.merge)` writes a grant at the case's bound revision with the launch identity, after which the next `takt run` on that case merges; a request without the launch credential, or naming another approver, is refused with nothing written; a test command in a story case that reads every `/proc/*/environ` and `/proc/*/cmdline` it can and posts to every 127.0.0.1 port it finds writes no grant; and `ess verify conform` covers the command.
