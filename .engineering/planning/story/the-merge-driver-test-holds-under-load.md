---
format: aep.planning-md/3
id: story:the-merge-driver-test-holds-under-load
kind: story
status: draft
title: The merge-driver judge-guard test holds on a loaded host
summary: git's own .git/info/refs write during pr.open is not a step change
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
revision: 1
---
# Story: the-merge-driver-test-holds-under-load

## Outcome

`judge_guard::a_configured_merge_driver_runs_nowhere_and_hides_nothing` passes on a loaded host,
as it does on an idle one.

## Context

- Failed once in `task check` on a host at load average 56 (2026-10-26, a docs session's run);
  the same test passed in its other runs. The failure: `pr.open` was refused with
  `StepRefused("pr.open wrote what its validator does not: .git/info/refs")` where the test
  expects `ApprovalRequired(["repository.merge"])` (`crates/takt/tests/judge_guard.rs:328`).
- `.git/info/refs` is a file git's own housekeeping writes (`git update-server-info`, which `git gc`
  runs); inferred, not traced: an automatic housekeeping run during `pr.open` under load.
- Related: story:the-active-budget-test-holds-under-load (the other test that failed at that load).

## Acceptance

Each item is a conformance scenario named in `ess/ess-inputs.yaml`, run by `task check`.

1. `step-guard/git-housekeeping-is-not-a-step-change`: a step during which git writes
   `.git/info/refs` (the test runs `git update-server-info` inside the step) is not refused for
   it; a write by the step to another path under `.git/info/` is still refused.
2. The test passes 20 runs in a row with the host loaded to twice its CPU count, and still fails
   on the defect it guards (a mutant).

## Scope

Inferred, not confirmed: `crates/takt/src/run.rs` (the step guard's view of `.git`),
`crates/takt/tests/judge_guard.rs`.
