---
format: aep.planning-md/3
id: approval-record:proof-2-defects-first
kind: approval-record
status: approved
title: The defects of the second real-story proof run go first
summary: two rsi stories first in the next wave; proof 2 again after their release
relations:
- decides: story:a-handover-after-red-is-not-the-next-step
- decides: story:a-revision-without-code-is-no-new-attempt
- decides: story:the-default-confinement-is-named-as-the-default
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-10-28T22:31:47Z", actor: "human:timo", revision: 2}
---
# Approval: the defects of the second real-story proof run first

## Decision

The orchestrator, under the owner's grant, 2026-10-28, after the proofs on takt 0.18.1: proof 1
(beyond10x/tools, story:fs-climb-through-missing-refused) completed and merged, 740k tokens,
15 steps; proof 2 (beyond10x/llm, story:context-length-refusal-is-typed) stopped twice
`RepeatedTestFailure`, 3.44M tokens in run 1, nothing pushed.

| item | decision |
|---|---|
| first in the next wave, `rsi` | story:a-handover-after-red-is-not-the-next-step, story:a-revision-without-code-is-no-new-attempt |
| also filed | story:the-default-confinement-is-named-as-the-default |
| the takt-probe files | 20 `~/.cargo/bin/takt-probe-N` files refused a proof step; checked against story:an-unrelated-program-install-refuses-no-step |
| trial | the `[TRIAL takt]` pass for 0.18.0 and 0.18.1 is owed by the trial agent |
| proof 2 again | when the two stories above are released, proof 2 is run again on the same story and its result reported to the orchestrator |
