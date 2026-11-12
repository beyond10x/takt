---
format: aep.planning-md/3
id: approval-record:wave-56-link-refusal
kind: approval-record
status: approved
title: 'Wave 56: refuse a link at a deployment path in this wave'
summary: the security blocker's remaining half pulled into wave 56, fail-closed
relations:
- decides: story:a-step-may-not-commit-a-link-at-a-deployment-path
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-11-06T20:08:11Z", actor: "human:timo", revision: 2}
---
# Approval: story:a-step-may-not-commit-a-link-at-a-deployment-path in wave 56

## Decision

The owner's standing delegation, through the orchestrator, 2026-11-06, after the wave 56 security review: a step that
puts a link at a deployment path is not shippable while only part of it is closed; fix it in this wave, or refuse such a
link fail-closed, before the gate.

The wave lead pulled the draft into wave 56 and chose fail-closed: a step whose change adds or replaces a link at a path
`deployment_paths` names, or at a directory above one inside the tree, is refused whatever its action and put back.
Built in 242ea0649.
