---
format: aep.planning-md/3
id: story:scenario-run-gate-names-a-protocol-action
kind: story
status: draft
title: takt scenario run --gate names a protocol action
summary: the scenario run's --gate is checked against its protocol, as takt run's is
tags:
- finding
relations:
- serves: vision:safe-agentic-coding
- decomposes: epic:composable-agentic-work
revision: 2
---
## Outcome

`takt scenario run --gate <action>` names an approval-required action of the protocol the scenario's case runs on, or
is refused with exit 1 before the scenario checks anything out, as `takt run --gate` is.

## Context

Found while implementing story:gate-flag-names-a-protocol-action (wave 54). `takt scenario run` shares the `--gate`
flag (`LoopArgs`, `crates/takt-cli/src/main.rs`) but its run path does not call `check_gate`, so a typo there is still
a filter that never matches and the gate is never offered.

## Acceptance

`takt scenario run ... --gate no.such.action` exits 1 naming the protocol and its approval-required actions, and no
case directory and no trial branch exist afterwards.
