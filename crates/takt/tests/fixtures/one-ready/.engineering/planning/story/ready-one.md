---
format: aep.planning-md/3
id: story:ready-one
kind: story
status: proposed
title: Make the check pass
summary: The one ready story
scope:
- confidence: cited
  path: check.txt
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-10-18T03:34:59Z", actor: "agent:fixture", revision: 2}
---
# Story: ready-one

## Outcome

The check in `check.txt` passes for whoever runs `./check.sh`.

## Acceptance

`./check.sh` exits 0.
