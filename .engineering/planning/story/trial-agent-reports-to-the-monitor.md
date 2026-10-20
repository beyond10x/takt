---
format: aep.planning-md/3
id: story:trial-agent-reports-to-the-monitor
kind: story
status: draft
title: The trial agent reports to the owner's monitor and hands it proposals for other repositories
summary: the trial profile lost its reporting line; ADR 0101 adds that a proposal for another repository goes to the monitor
tags:
- process
- rsi
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
scope:
- confidence: cited
  path: .agents/trial-agent.md
revision: 3
---
# Story: trial-agent-reports-to-the-monitor

## Outcome

`.agents/trial-agent.md` tells the trial agent to report to the owner's monitor, when the owner
names one: one message of at most five lines at each step boundary of a cycle and on any
deviation, and a proposal that concerns another repository sent to the monitor as one message
instead of acted on.

## Context

The profile written in wave 17 left out the reporting line its source had, so a trial session
following it sends nothing between cycles. The owner's architecture decision (ADR 0101) widens
what the monitor receives: a proposal for another repository goes to it as one message. Not
from self-improvement findings, so not tagged `rsi`.

## Scope

- `.agents/trial-agent.md`

## Acceptance

1. The profile states the report: to the monitor the owner names, at most five lines, at each
   step boundary of the cycle and on any deviation.
2. The profile states that a proposal concerning another repository goes to the monitor as one
   message and is not acted on by the trial agent.
3. Committed text names no session, home path, person or organization;
   `b10x-gates scan-text` passes.
