---
format: aep.planning-md/3
id: approval-record:delivery-in-batches
kind: approval-record
status: approved
title: 'Delivery in batches: one full gate and one pull request per batch'
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-10-28T21:13:53Z", actor: "human:timo", revision: 2, executor: "agent:planner"}
---
# Approval: delivery in batches, one full gate and one pull request per batch

## Decision

The owner's general rule for every session (2026-10-28), relayed by the orchestrator: work in
waves or batches; several stories land on one integration branch; expensive full gates do not run
after every small change; a unit runs only the targeted tests of what it implements; when
everything is in the integration branch, one local full gate runs, then one pull request to the
default branch, set to merge automatically.

## Covered

| item | what |
|---|---|
| units | only the tests of what the unit changes; no `task check` per unit or per wave branch |
| gate | one local `task check` on `integration/takt` after the batch's waves are in |
| delivery | one pull request from `integration/takt` to `main` through the bot route, set to merge on its required checks, then one release |
| record | `AGENTS.md` § Rules and `.agents/dispatcher-agent.md` steps 7 and 8.5 to 8.6 |
