---
format: aep.planning-md/3
id: approval-record:candidate-bb4072a3-2-merge-grant
kind: approval-record
status: approved
title: The candidate trial-cand/2026-10-21-bb4072a3-2 is merged in wave 31
relations:
- informed_by: decision-blocker:wave-29-merge-grant
- decides: story:codex-step-rereads-its-context
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-10-22T00:29:43Z", actor: "human:timo", revision: 2}
---
## Decision

On 2026-10-22 the owner's side granted, through the orchestrator under the owner's standing
delegation and as decision-blocker:wave-29-merge-grant sets out for later waves ("one grant per
merge, from the coordinator, on a clean verdict at its sha"):

1. Merge the candidate `trial-cand/2026-10-21-bb4072a3-2` at `f4f8d8d2` `--no-ff` into `wave/31`
   after the unit of story:codex-prompt-exceeds-launch-argument and
   story:codex-context-not-recorded, resolve the conflicts there, and merge the wave into `main`
   on a clean reviewer verdict at the merge sha.
2. If the conflict resolution changes the candidate's behaviour, the reviewer judges the merge
   sha, not `f4f8d8d2`.

Reason: one gate and one review instead of two over the same files (`crates/takt/src/harness.rs`,
`ess/domains/takt.yaml`).

## The candidate

- One commit on `86d0433a`, which is in `main`: "harness: name a Codex step's context files in
  its prompt". It fixes story:codex-step-rereads-its-context.
- Measured on Codex, scenario 1, 5 runs a side: median run tokens 84,984 at `f4f8d8d2` against
  101,585 at `cabc09a2`.
- The trial's reviewer approved at `f4f8d8d2` with 0 blocking findings.
