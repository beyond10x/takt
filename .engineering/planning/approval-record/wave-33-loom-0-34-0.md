---
format: aep.planning-md/3
id: approval-record:wave-33-loom-0-34-0
kind: approval-record
status: approved
title: Wave 33 moves takt to loom-sdk 0.34.0 and the loop onto run_until_blocked, option A on the exit order
relations:
- decides: story:the-loop-is-run-until-blocked
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-10-22T12:24:44Z", actor: "human:timo", revision: 2}
---
# Approval: wave 33, loom-sdk 0.34.0 and the loop on run_until_blocked

## Decision

The orchestrator, under the owner's delegation (2026-10-22): "Wave 33 approved (U1 pin loom-sdk
0.34.0, U2 story:the-loop-is-run-until-blocked); later waves in your order, harness-child-confined
stays blocked. Fork: A, with 2 conditions. (1) The design registers exit 7 as `Paused`
(.engineering/planning/design/continuous-orchestration.md:108,118). If that number is free in the
code today, record in the CHANGELOG and design § 5 which stop owns 7. If 7 would collide, use the
code § 5 assigns to NoPerformableAction. (2) .agents/trial-agent.md:76 reads exit 3 as the Hours
outcome: update that row so a trial with both Hours and Unperformable counts the new code the same
way. Add a test that pins the order."

## The fork it answers

Loom 0.34.0 checks `NoPerformableAction` before the deadline (`crates/loom-commission/src/runtime.rs:1594`
against `:1740` at tag 0.34.0). Option A: where Hours and Unperformable both hold, the run ends
Unperformable (exit 7) where takt's own loop gave Hours (exit 3). Option B, not taken: keep
dependency-blocker:loom-runtime-stops open for a Loom change.

## How the conditions are met

- Exit 7 is not free: `crates/takt-cli/src/main.rs:741` already gives 7 to Unperformable, and `:744`
  gives 8 to DiskLow, while design § 5 proposes 7 for `Paused` and 8 for `FrontierExhausted`, neither
  built. The stop that owns 7 stays Unperformable, so no stop changes its code; § 5 registers 6, 7
  and 8 as the code assigns them and moves the two proposed stops to 9 and 10. The CHANGELOG names
  the new order.
- `.agents/trial-agent.md` counts a run that stops Unperformable while over its hours the way it
  counts an Hours stop.
- A test pins the order: with both holding, the run stops Unperformable, exit 7.
