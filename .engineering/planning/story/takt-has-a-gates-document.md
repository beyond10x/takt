---
format: aep.planning-md/3
id: story:takt-has-a-gates-document
kind: story
status: active
title: takt's repository names its gate so delivery on takt reaches pr.open
summary: 'trials 1 and 2: delivery stalled on gate.green'
tags:
- rsi
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-10-19T15:58:59Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-19T15:58:59Z", actor: "human:timo", revision: 3}
---
# Story: takt-has-a-gates-document

## Outcome

takt's own repository has `.engineering/gates.yaml` naming its gate (`task check`), so a
`story.delivery/1` case on takt can reach `gate.green` and `pr.open`.

## Context

RSI trials 1 and 2: delivery on takt stalled because no gates document registers a gate validator
(`main.rs:892`), so `gate.green` never holds; trial 1 lost 457k tokens to the loop, trial 2 handed
over twice. story:delivery-refuses-an-obligation-nobody-performs makes the stall a named stop;
this gives takt the gate.

## Acceptance

- `takt run` on a takt story offers `gate.run` after the implementation and reaches `pr.open`
  when `task check` passes: a scenario or test against a copy of the repository.
- The gate is the four fast commands of `task check` (specification, citations, format, lint)
  run through `scripts/gate-exec`; the workspace tests run as the delivery's test command with
  1800 s (planner, 2026-10-19: the gate's per-command limit is 300 s).
