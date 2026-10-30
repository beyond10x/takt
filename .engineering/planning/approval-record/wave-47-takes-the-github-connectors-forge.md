---
format: aep.planning-md/3
id: approval-record:wave-47-takes-the-github-connectors-forge
kind: approval-record
status: approved
title: Wave 47 takes the github connectors forge as U3
summary: story:connectors-forge-speaks-github-at-an-endpoint joins wave 47 after U1, beside U2
relations:
- decides: story:connectors-forge-speaks-github-at-an-endpoint
revision: 3
transitions:
- {from: "draft", to: "approved", at: "2026-10-30T04:29:48Z", actor: "human:timo", revision: 3}
---
# Approval: wave 47 takes the github connectors forge as its third unit

## Decision

The orchestrator, under the owner's grant (2026-10-30): story:connectors-forge-speaks-github-at-an-endpoint
joins wave 47 as U3. It starts once U1 (story:takt-carries-no-organisations-own-route) is
committed, because both change the forge code, and it may run beside U2
(story:git-and-model-through-loopback-relays). The wave stays at three units. The story is on the
path to running takt in a pod whose agent container holds no credential: the forge's writes go
through a served Connectors instance on loopback, and takt holds no token for it.

It is built and tested against a loopback stand-in of the served wire (`GET /v1/describe`,
`POST /v1alpha2/invoke`); the Connectors release that serves the github operations follows
separately and is not a condition of this unit.

## Covered

| item | what |
|---|---|
| unit | U3 of wave 47: story:connectors-forge-speaks-github-at-an-endpoint, moved to active |
| order | after U1's commit; beside U2 |
| test | a loopback stand-in of the served wire only |
