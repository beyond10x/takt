---
format: aep.planning-md/3
id: dependency-blocker:loom-runtime-stops
kind: dependency-blocker
status: cleared
title: 'Loom''s run_until_blocked cannot end a run where takt''s loop ends it (0.33.0: the low-disk stop comes before approval)'
summary: approval gate, idle bound, token and time budgets, pending requests, completion
tags:
- loom
relations:
- blocks: story:the-loop-is-run-until-blocked
revision: 11
transitions:
- {from: "open", to: "cleared", at: "2026-10-22T12:24:54Z", actor: "human:timo", revision: 11}
---
# Blocker: Loom's run_until_blocked cannot end a run where takt's loop ends it

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

## Against Loom 0.32.0 (2026-10-22)

Read at tag `0.32.0` (a238395b); paths in `crates/loom-commission/src/` unless named. One probe on `software-change@1` over a `CanonGovernor` with a `MemoryCaseStore` passed (`ok. 1 passed`); rows marked "read" were not probed.

| gap | 0.32.0 | how takt gets its behaviour | how it was read |
|---|---|---|---|
| A | closed: the gate is never armed when `approval_gate: false` (`runtime.rs:245-257`, `1815`) | `StopPolicy { approval_gate: false, .. }`; exit 4 then comes from `NeedsAuthority`, answered by the case's authority provider | probe: default policy `AwaitingApproval ["release.rollback"]` after 1 call; gate off with `idle_bound: Some(1)`, `IdleBound { idle_iterations: 1 }` after 1 call |
| B | closed, with a condition: the idle bound applies under a step budget (`runtime.rs:1134-1137`, `1795-1803`); progress is a new (action, arguments) pair in the run, and a performed effect does not make a repeated pair new (`1575-1578`, `1638`) | `idle_bound: Some(1)`, `IdleBound` mapped to `NoProgress`; validator proposals carry a per-revision key in their arguments (today `{}`, `crates/takt/src/validator.rs:313-325`); a harness proposal that changes no file answers `NoUsefulAction` | probe: budget 10 with `Some(1)`, 1 call then `IdleBound{1}`; with `None`, 10 calls then `Suspended(Budget max_steps 10)`; the same arguments twice, 2 calls, 2 effects, then `IdleBound{1}` |
| C | partly closed: `RunBudget { max_tokens, deadline, max_steps }` (`runtime.rs:291-306`, `1475-1479`, `1505-1517`); one `StopPolicy.on_breach` for every limit (`1490-1517`) | open: takt keeps a step that crosses the token limit (`crates/takt/tests/loop_gaps.rs:286-299`) and discards a harness step that returned at or after the hour deadline (`crates/takt/src/run.rs:10366-10420`); one `on_breach` cannot do both. Needs a breach choice per limit in Loom | read |
| D | closed: `EffectOutcomePending { ends_run: true }` ends the run `Waiting` (`runtime.rs:182-189`, `2663-2676`); a pending answer is not judged against the idle bound (`1431-1446`); the deadline is checked before each re-invocation | `ends_run: true` for a local wait and for a run with no step or hour budget; a forge wait pauses inside `invoke` and answers `ends_run: false` | read |
| E | changed: `CanonGovernor::with_completion_rule(CompletionRule::FirstDeclared)` (`crates/loom-governor/src/lib.rs:894`, `1334-1345`, `1951-1960`) | settled: takt takes `CompletionRule::FirstDeclared` (approval-record:completion-order-first-declared, 2026-10-22). Today takt completes on the first legitimate outcome by id (`crates/takt/src/decision.rs:78-86`); with `accepted` and `abandoned` both legitimate it records `abandoned` today and `accepted` once the story lands, exit 0 either way | read |

Outside A–E, needed for acceptance 2: `ActiveTime` (wall time less forge pauses) has no Loom limit (`max_duration_ms` over reported durations is near it if the executor reports wall time), and `DiskLow` has no Loom stop. Both read.

Clearing this for 0.32.0: no. It needs C's per-limit breach choice and a stop for `ActiveTime` and `DiskLow`. E is settled by approval-record:completion-order-first-declared.

## Against Loom 0.33.0 (2026-10-22)

Read at tag `0.33.0` (3d56bca9); `rt` is `crates/loom-commission/src/runtime.rs`. Read, not probed.

| gap | 0.33.0 | how takt gets its behaviour |
|---|---|---|
| C | closed: `StopPolicy.breach_by_limit` (`rt:1546-1563`, `3308-3322`); a kept breach stops the run at the top of the next iteration, after the completion check (`rt:1321-1345`, `1374-1379`) | `[{MaxTokens, KeepStep}, {Deadline, DiscardStep}]`; Loom breaches above the limit (`rt:3216`) and takt at it, so takt passes `max_tokens = t - 1`; reverting a discarded step's files stays takt's (`crates/takt/src/run.rs:10389`) |
| ActiveTime | closed: `LoopContext::host_stop` is asked before every executor call and before a pending request is invoked again (`rt:532`, `1402-1404`, `1527-1529`) | `--active-seconds` (`crates/takt-cli/src/main.rs:622-626`) as `HostStop "ActiveTime"`, the forge-wait total shared between the effect port and the context; `--hours` is `RunBudget.deadline` |
| DiskLow | the check closed, the order open: `host_stop` is asked before the executor call (`rt:1527`), while with `approval_gate: false` approval arrives as `NeedsAuthority` after a proposal and `NoAdmissibleAction` after the call | takt checks low disk after ApprovalRequired, NothingAdmissible and Unperformable (`run.rs:10006-10019`); on 0.33.0 a case that only a grant can move ends DiskLow (exit 8) where takt ends ApprovalRequired (exit 4), against `crates/takt/tests/free_disk.rs:513-529` |
| order of Hours and Unperformable | Loom checks NoPerformableAction before the deadline and the host stop (`rt:1386-1390`, `1527`) | takt checks Hours and ActiveTime first (`run.rs:9812`, `9995`): exit 3 against Loom's exit 7 when both hold; whether a test pins it is not read |
| `--tokens 0` | no token check before a step | takt stops before any step (`run.rs:8621-8624`); with Loom one step runs |

Clearing this for 0.33.0: no. It needs Loom to ask `host_stop` only when a step would follow
the admissibility and authority checks, or the owner's decision to accept exit 8 where takt
gives 4 (a change to the documented exit order).

## Decision on 0.33.0 (2026-10-22)

The orchestrator, under the owner's delegation: "A, taken now. Reason: B changes takt's exit
contract (4 -> 8, crates/takt/tests/free_disk.rs:509-528) and leaves confinement blocked. ...
Loom 0.34.0 carries the host_stop ordering; a substrate session now builds the writable
mount/workdir and several apertures".

Option A: this blocker stays open until a Loom release asks `host_stop` only when a step follows
the admissibility and authority checks. Option B, not taken: accept exit 8 where takt gives 4 and
land the loop story on 0.33.0.

## Against Loom 0.34.0 (2026-10-22)

Read at tag `0.34.0` (8cfac2cc); `rt` is `crates/loom-commission/src/runtime.rs`. Read, not probed.

| gap | 0.34.0 | how takt gets its behaviour |
|---|---|---|
| C, ActiveTime | closed, as on 0.33.0 (`rt:553-561`, `1528-1537`, `1738-1745`, `1609`) | unchanged |
| DiskLow | closed: `host_stop` is asked only when the handed frontier admits an action without a grant (`rt:1738`, `3645`); otherwise the host is asked just before a granted effect (`rt:1913`) | the composite executor proposes the gated action without a harness call, so `crates/takt/tests/free_disk.rs:529` holds |
| order of Hours and Unperformable | Loom checks `NoPerformableAction` (`rt:1594`) before the deadline (`rt:1740`) | option A below: exit 7 where takt gave 3 when both hold |
| `--tokens 0` | no token check before step 1 (`rt:1528-1531`) | takt stops before `run_until_blocked` when the token budget is spent, as `crates/takt/src/run.rs:8621-8624` does |

## Decision on 0.34.0 (2026-10-22)

The orchestrator, under the owner's delegation, chose option A for the order of Hours and
Unperformable, with two conditions; approval-record:wave-33-loom-0-34-0 quotes it. Cleared: the
`host_stop` order this blocker waited on ships in 0.34.0.
