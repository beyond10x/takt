---
format: aep.planning-md/3
id: dependency-blocker:loom-runtime-stops
kind: dependency-blocker
status: open
title: Loom 0.31.0's run_until_blocked cannot end a run where takt's loop ends it (gaps A-E)
summary: approval gate, idle bound, token and time budgets, pending requests, completion
tags:
- loom
relations:
- blocks: story:the-loop-is-run-until-blocked
revision: 1
---
# Blocker: Loom 0.31.0's run_until_blocked cannot end a run where takt's loop ends it

story:the-loop-is-run-until-blocked hands takt's loop to Commission's `run_until_blocked` and keeps every stop kind and exit code (acceptance 2) without a stop rule of takt's own. Against loom 0.31.0 (tag `0.31.0`), five runtime behaviours differ from takt's stops. Clearing this needs a Loom release that closes A–D and settles E, or an owner decision that accepts the changed exit codes, which is a change to takt's documented `--help` exit table.

| gap | Loom 0.31.0 | takt today | how it was read |
|---|---|---|---|
| A, approval gate | `crates/loom-commission/src/runtime.rs:1307-1315`, `1604-1608`, `2644-2661`: the gate arms whenever an approval-required action is not granted, and an iteration with an unchanged frontier ends `AwaitingApproval`; `software.change/1` always lists `release.rollback` as approval-required | a step that changes nothing stops `NoProgress`, exit 3 | probe: one step that changed nothing ended `AwaitingApproval { actions: ["release.rollback"] }` after 1 step, with no step budget and with a budget of 8 |
| B, idle bound | `runtime.rs:1587-1594`: the idle bound applies only when there is no step budget | `NoProgress` after one idle harness call (`crates/takt/tests/no_model_step.rs:205-211`, `pr_opens_when_done.rs:446`, `528`) | probe on `story.delivery/1`: with a budget of 10, 10 executor calls, then `Suspended(Budget max_steps 10)`; with none, 3 calls, then `IdleBound { idle_iterations: 2 }` |
| C, token and time budgets | `ess/commission/domains/responsibility.yaml:500-513`, `runtime.rs:1320-1337`: no total-token limit; a step over a limit has its proposal discarded; duration counts charged usage | the step is kept and the run stops before the next one (`loop_gaps.rs:286-299`, `loop_gaps_adv1.rs:208-219`); `--hours` is wall time since open (`loop_gaps_adv1.rs:189-203`) | read in code |
| D, pending requests | `runtime.rs:417`, `1057`, `1200-1305`: a pending request is re-invoked at once, up to 64 times without a budget | a pause between forge observations; a local wait stops `Waiting` at once (`crates/takt/src/validator.rs:88-96`) | read in code |
| E, completion | `crates/loom-governor/src/lib.rs:1836-1853`: completes only when exactly one outcome is legitimate | completes on the first legitimate outcome (`crates/takt/src/run.rs:9803-9806`) | read in code; whether a takt protocol yields two at once is not known |

G5-G9 of the earlier gap list are closed in 0.31.0 (`crates/loom-commission/src/outcome.rs:147-214`, `runtime.rs:1194-1337`, `1463-1485`, `1587-1594`, `1650-1679`, `1948-1988`, `2425-2486`, `2868-2876`, `crates/loom-commission/src/ports/executor.rs:31-52`).

With the tree unchanged: b10x-takt 1141 passed, 5 ignored; b10x-takt-cli 477 passed, 1 ignored.
