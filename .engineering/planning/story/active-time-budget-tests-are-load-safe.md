---
format: aep.planning-md/3
id: story:active-time-budget-tests-are-load-safe
kind: story
status: draft
title: The active-time budget tests fail on a loaded machine
tags:
- finding
- priority:medium
refs:
- provider: finding
  reference: active-time-budget-tests-are-load-safe
relations:
- serves: vision:safe-agentic-coding
revision: 1
---
# The active-time budget tests fail on a loaded machine

## What is wrong

`a_forge_wait_longer_than_the_active_budget_does_not_stop_the_run`
(`crates/takt/tests/active_time_budget.rs:347` at 7c9e7129) failed in both workspace test runs of
wave 39's gate, and `steps_that_pass_the_active_budget_stop_the_run_on_it` (`:400`) in the first;
both passed alone at the same revision (`docs/waves/wave-39.md`). The failing run stopped on
`ActiveTime { budget: 2, active: 2.103274918, waited: 3.000860609 }` before it reached the merge's
approval: the steps before the forge waits, which the test counts on finishing well inside a 2 s
active budget, took 2.1 s while the machine's load average was 24 to 40.

The budget is wall time less waiting, so the test's margin is the time three cheap steps take on
the machine that runs it. Inferred: no budget defect, only a margin too small for a shared
machine. Not yet shown which steps used the time.

## Check

Not runnable by `scripts/check-finding`: the command runs cargo's tests, not `$TAKT`. It needs
load beside it; `stress-ng` or a parallel build stands in for the shared machine.

```expected_before
command: (stress-ng --cpu 0 --timeout 120 >/dev/null 2>&1 &); for i in $(seq 5); do cargo test -p b10x-takt --test active_time_budget 2>&1 | grep -q 'FAILED' && exit 1; done; exit 0
exit: 1
healthy_exit: 0
output: test result: FAILED
source: docs/waves/wave-39.md
```

## Outcome

The two tests give the same result under load as on an idle machine, and still fail when the
budget does not exclude a forge wait or does not stop a run that passes it.

## Acceptance

1. The steps' active time in a failing run is shown, step by step, before the fix.
2. The predicate above exits 0.
3. A mutant that counts waiting as active time turns the first test red; one that never stops on
   the active budget turns the second red.

## Scope

- `crates/takt/tests/active_time_budget.rs` (cited)
