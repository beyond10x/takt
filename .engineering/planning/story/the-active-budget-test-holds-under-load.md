---
format: aep.planning-md/3
id: story:the-active-budget-test-holds-under-load
kind: story
status: draft
title: The active-budget forge-wait test holds on a loaded host
relations:
- serves: vision:safe-agentic-coding
revision: 1
---
## Outcome

`a_forge_wait_longer_than_the_active_budget_does_not_stop_the_run`
(`crates/takt/tests/active_time_budget.rs`) holds on a loaded or slow host: its assertion does not
depend on the run's non-waiting work finishing within the 2 s active budget (`BUDGET`, line 30).

## Context

| where | active time measured | budget | result |
|---|---|---|---|
| darwin run 39912520340 at 5c85e16, attempt 1 (macos-15) | 2.381594916 s, waited 3.215474709 s | 2 s | `Waiting { on: ["ci.read"] }` instead of `ApprovalRequired(["repository.merge"])` |
| the same run, attempt 2 | | 2 s | passed |
| Linux, `task check` at load average 87 (wave 35, U2b) | 2.18 s | 2 s | failed; 3 of 3 passed alone |

The test passed in the four earlier darwin runs of wave 35. Its defect is in the test's fixed
budget, not in takt's behaviour, so it has no `expected_before` predicate: no `takt` command shows
it.

## Acceptance

1. The test's budget leaves the run's own work room on a host at load average 87, while every
   forge wait still exceeds the budget, so the property it names stays tested.
2. `task check` and `darwin.yml` stay green.

## Out of scope

The active-time accounting in `crates/takt/src/run.rs`.
