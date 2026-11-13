---
format: aep.planning-md/3
id: approval-record:wave-58-command-action
kind: approval-record
status: approved
title: 'Wave 58: a command action'
summary: 'one unit toward connector actions: run-file-bound command actions'
relations:
- decides: story:a-command-action
revision: 4
transitions:
- {from: "draft", to: "approved", at: "2026-11-12T20:59:46Z", actor: "human:timo", revision: 2, executor: "agent:b10x-takt", correlation: "wave/58"}
---
# Approval: wave 58

## Decision

An instruction of 2026-11-12 from the session that asked for this epic, which it attributed to the owner: carry
every wave of epic:scheduled-runs-and-connector-actions through to the end, waves 57 to 62, one pull request per
wave, without stopping to ask between waves, each wave's branch stacked on the previous one while its merge waits.
No line typed by the owner with that content was found; it is recorded as that session's instruction. It opened the
wave and granted no merge: the merge waits for a grant at the pull request's head, and the record of that grant
names who granted.

Wave 58 takes one unit: story:a-command-action (design `docs/design/scheduled-runs-and-connector-actions.md` § 4 and
§ 7). `wave/58` branches from `wave/57` at f4616dbd while pull request 65 waits for its grant.

Spec first (`AGENTS.md` § Rules): `takt.work.CommandAction` and the field `command_actions` on
`takt.work.RunConfiguration` are declared in `ess/domains/takt.yaml` before the code.

The merge into `main` waits for the grant of the agent the owner authorized on 2026-11-12 to decide on the owner's
behalf; the delegation of `repository.merge.judge_surface` to the planner stays revoked (`.engineering/delegations.yaml`).

Its transition records `actor: "human:timo"` and `executor: "agent:b10x-takt"`. The store has no field for a session
that acts on an instruction attributed to the owner, so `actor` names on whose behalf the instruction was given, not
a line the owner typed; this text is the record of who gave it.
