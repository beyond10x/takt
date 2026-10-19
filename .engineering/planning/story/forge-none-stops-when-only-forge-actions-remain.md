---
format: aep.planning-md/3
id: story:forge-none-stops-when-only-forge-actions-remain
kind: story
status: active
title: Under --forge none a case stops by name when only forge actions remain
summary: 'trial 2: ~29k tokens a scenario run, one resume loop of 615k'
tags:
- rsi
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-19T15:58:58Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-19T15:58:58Z", actor: "human:timo", revision: 3}
---
# Story: forge-none-stops-when-only-forge-actions-remain

## Outcome

Under `--forge none`, a case whose only admissible actions are forge-owned (`pr.open`, CI, merge)
stops with a named stop and its own exit code before any model step, instead of offering the model
edit and test actions until NoProgress.

## Context

RSI trial 2 (2026-10-19, scratch state): every scenario run under `--forge none` reached
`change.read` "ahead", then took one more model step that answered "No change… CI and merge need
actions not offered" and stopped on NoProgress, about 29k tokens a run (`run.rs:6768-6777`,
`main.rs:911`). On resume one case invented tests three times to hand over again: 615,395 tokens
for a help-text candidate. The same class as story:delivery-refuses-an-obligation-nobody-performs.

## Acceptance

- A scenario run under `--forge none` that reaches "ahead" stops with a named stop, takes no model
  step after it, and a resume takes none either: a test with a harness double recording zero calls.
- With a forge, nothing changes.
