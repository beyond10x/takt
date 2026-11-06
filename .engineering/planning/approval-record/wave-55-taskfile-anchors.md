---
format: aep.planning-md/3
id: approval-record:wave-55-taskfile-anchors
kind: approval-record
status: approved
title: 'Wave 55 unit b: specification prose and the strict anchor rule'
summary: the includes prose lands with unit b; a base Taskfile anchor or alias refuses the gate
relations:
- decides: story:the-gate-compares-taskfile-includes
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-11-06T06:07:08Z", actor: "human:timo", revision: 2}
---
# Decision: unit b of wave 55, the specification prose and the anchor rule

## Decision

The orchestrator, under the owner's delegation, 2026-11-06:

1. "Apply its spec prose patch ... in unit b's commit." The prose in `ess/domains/takt.yaml` that describes the gate's
   base comparison now names the Taskfiles pulled in through `includes:`.
2. "Anchor rule: keep the strict rule (any anchor or alias in a base Taskfile refuses the gate) unless a repository takt
   runs against today uses one. ... If either uses one, narrow to 'refuse only when the top level has includes or <<'
   in this unit; else keep strict and note it in the CHANGELOG."

## Check

Read at 2026-11-06 with `grep` for `&` anchors, `*` aliases and `<<:` merge keys: takt's own `Taskfile.yml` at
24658b5a1 holds none; todo-app-example's `main` holds no Taskfile. The strict rule stays, and the 0.33.0 CHANGELOG
says that a base Taskfile with an anchor or alias refuses the gate.
