---
format: aep.planning-md/3
id: approval-record:wave-56-excluded-files
kind: approval-record
status: approved
title: Accept story:git-excluded-files-are-not-step-changes for wave 56
summary: accepted after decision-blocker:excluded-files-and-user-excludes cleared
relations:
- decides: story:git-excluded-files-are-not-step-changes
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-11-06T04:03:39Z", actor: "human:timo", revision: 2}
---
# Approval: story:git-excluded-files-are-not-step-changes for wave 56

## Decision

The owner's standing delegation, through the orchestrator, 2026-11-06: "Accept
story:git-excluded-files-are-not-step-changes for wave 56."

decision-blocker:excluded-files-and-user-excludes is cleared (option 3: a deployment names its paths in the run file;
the guard reads no excludes file a step can write). The story is accepted now and joins wave 56, not wave 55: it is not
an `rsi` story, and wave 55 was fixed before it was accepted.
