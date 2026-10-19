---
format: aep.planning-md/3
id: story:wall-time-test-without-sleep
kind: story
status: draft
title: The crashed-run wall-time test sets its instants and does not sleep
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
scope:
- confidence: cited
  path: crates/takt/tests/scorecard_adv1.rs
revision: 3
---
## Outcome

`a_crashed_runs_wall_time_ends_at_its_own_last_line` (`crates/takt/tests/scorecard_adv1.rs:245`) checks the split of a killed run's wall time from the record's own `at` instants, set explicitly, and never from a sleep, so it passes under any machine load.

## Context

Reported 2026-10-19 by the developer of story:promotion: in a gate on bdba932 (promotion merged with main a388adf), with two other `cargo test` processes running, the test failed with "runs to the resume: 632 ms" against its bound of 500 ms after a 600 ms sleep. Alone it passes 8 of 8, and it passed in the df9681a gate. Inferred cause, not verified: under load the record helper's own writes spread past the bound, so the assertion measures scheduling. A flaky test in `task check` lets a red gate be rerun until green, which hides a real failure.

## Scope

- `crates/takt/tests/scorecard_adv1.rs`: the test's fixture (cited); story:run-trace-and-report also edits this file

## Acceptance

The test writes its run lines with fixed `at` instants and no sleep; it passes 20 of 20 while two other `cargo test --workspace` processes run, and it still fails when the wall-time split is wrong (the assertion is checked against a mutated split).
