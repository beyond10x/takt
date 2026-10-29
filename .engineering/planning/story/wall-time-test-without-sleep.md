---
format: aep.planning-md/3
id: story:wall-time-test-without-sleep
kind: story
status: draft
title: The after-cleanup deadline test injects its cleanup cost instead of a wall-clock margin
summary: a fixed injected cleanup cost lets hours_bounds_a_harness_step_adv1.rs catch a step deadline judged after the cleanup
tags:
- rsi
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
- depends_on: story:unqualified-stop-records-no-layer
scope:
- confidence: cited
  path: crates/takt/src/run.rs
- confidence: cited
  path: crates/takt/tests/hours_bounds_a_harness_step_adv1.rs
revision: 10
---
## Outcome

`crates/takt/tests/hours_bounds_a_harness_step_adv1.rs` catches a regression that judges a harness step's deadline after the step's cleanup instead of when the step returned. The cleanup takes a fixed, known cost injected on the cleanup path, so no wall-clock margin decides the result.

## Context

The fixture half of this story is delivered: `a_crashed_runs_wall_time_ends_at_its_own_last_line` (`crates/takt/tests/scorecard_adv1.rs:294`) sets its `at` instants with `pin()` (`:267`, used at `:301` and `:319`) and does not sleep, since da068178 (story:timing-tests-hold-under-load).

What remains is from the review of the wave-12 test headroom (dadc264): `hours_bounds_a_harness_step_adv1.rs` cannot catch the mutant that replaces `returned` by `Instant::now()` in the step's deadline check (`returned >= deadline`, `crates/takt/src/run.rs:4567@dadc264`). A margin large enough to hold under load is longer than the cleanup: 18-26 ms for `git status -uall` over 20000 files at load 20.

At 338082d, `returned` is taken when the harness step returns (`run.rs:12269`), and the deadline is judged after the step's change is read (`run.rs:12502`). The validator observation's pair (`:11203`, `:11339`) is outside this story.

A flaky test in `task check` lets a red gate be rerun until green, which hides a real failure (reported 2026-10-19 by the developer of story:promotion).

## Ordering

- `crates/takt/src/run.rs` is wave 39's file (story:unqualified-stop-records-no-layer); this story waits for wave 39 to merge (`depends_on`).
- `crates/takt/src/run.rs` is also scoped by story:abandon-a-case (`:8374-8443`, `:9720-9858`) and story:closed-pull-request-stops-the-run (where the run ends on a closed pull request). This story is tagged `rsi` and goes before both; its hunk is the step's cleanup path (`:12269-12502`), so no edge, and `aep plan artifact waves` keeps them in different waves.

## Expected before

Inferred from the review at dadc264, not re-run at 338082d; holds before the change, fails after:

- With `returned` replaced by `Instant::now()` in the check at `run.rs:12502`, `cargo test -p b10x-takt --test hours_bounds_a_harness_step_adv1` passes.

## Scope

- `crates/takt/src/run.rs`: a test-only injected cost on the step's cleanup path (cited, `:12269-12502`)
- `crates/takt/tests/hours_bounds_a_harness_step_adv1.rs`: the test (cited)

## Acceptance

A new test in `hours_bounds_a_harness_step_adv1.rs` makes the step's cleanup take a fixed injected cost and asserts without a wall-clock margin. It fails 3 of 3 against the mutant (`returned` replaced by `Instant::now()` at the step's deadline check) and passes 20 of 20 against the code while two other `cargo test --workspace` processes run.
