---
format: aep.planning-md/3
id: approval-record:wave-59-connector-read-action
kind: approval-record
status: approved
title: 'Wave 59: a connector read action'
summary: 'one unit toward connector actions: run-file-bound Connectors reads'
relations:
- decides: story:a-connector-read-action
revision: 4
transitions:
- {from: "draft", to: "approved", at: "2026-11-12T23:18:12Z", actor: "human:timo", revision: 2, executor: "agent:b10x-takt", correlation: "wave/59"}
---
# Approval: wave 59

## Decision

An instruction of 2026-11-12 from the session that asked for this epic, which it attributed to the owner: carry
every wave of epic:scheduled-runs-and-connector-actions through to the end, waves 57 to 62, one pull request per
wave, without stopping to ask between waves, each wave's branch stacked on the previous one while its merge waits.
No line typed by the owner with that content was found; it is recorded as that session's instruction. It opened the
wave and granted no merge: the merge waits for a grant at the pull request's head, and the record of that grant
names who granted.

Wave 59 takes one unit: story:a-connector-read-action (design `docs/design/scheduled-runs-and-connector-actions.md` § 4
"Read action" and § 7). It reuses the client of story:one-served-connectors-client and the registration of run-file-bound
validators of story:a-command-action, with its kind check on open, on resume and at each observation. `wave/59`
branches from `wave/58` at 67eea205 while pull request 66 waits for its grant.

Spec first (`AGENTS.md` § Rules): `takt.work.ConnectorAction`, `takt.work.ConnectorsEndpoint` and the fields
`connectors` and `connector_actions` on `takt.work.RunConfiguration` are declared in `ess/domains/takt.yaml` before the
code.

The merge into `main` waits for the grant of the agent the owner authorized on 2026-11-12 to decide on the owner's
behalf; the delegation of `repository.merge.judge_surface` to the planner stays revoked (`.engineering/delegations.yaml`).

Its transition records `actor: "human:timo"` and `executor: "agent:b10x-takt"`. The store has no field for a session
that acts on an instruction attributed to the owner, so `actor` names on whose behalf the instruction was given, not
a line the owner typed; this text is the record of who gave it.

The merge itself was granted by the owner's own answer of 2026-11-13, 03:16 CET, at head b47913d3
(approval-record:pr-67-merge).
