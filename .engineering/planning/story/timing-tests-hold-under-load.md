---
format: aep.planning-md/3
id: story:timing-tests-hold-under-load
kind: story
status: active
title: task check does not fail on wall-clock bounds under load
summary: three timing tests failed at load 63-69 and passed alone
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-10-19T15:58:59Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-19T15:58:59Z", actor: "human:timo", revision: 3}
---
# Story: timing-tests-hold-under-load

## Outcome

`task check` does not fail on a loaded host because a test measures wall-clock time against a
fixed bound.

## Context

On 2026-10-19 three tests failed only under load (load average 63.7 to 68.8) and passed alone:
`hours_bounds_a_harness_step_adv2.rs:204`, `hours_bounds_the_run_end` (`:214`) and `loop_scorecard`
`a_twenty_thousand_commit_history_is_walked_within_ten_seconds` ("two walks of 20,000 commits took
11.48 s"). Each failure stopped a wave gate and cost a rerun. Improvement trials run beside builds,
so the gate must hold under load.

## Acceptance

- Each of the three tests passes with the host loaded to twice its CPU count (a stress run in the
  test notes), and still fails on the defect it guards: a mutant per test.
- No wall-clock bound in `crates/**/tests` is shorter than ten times the unloaded median it
  measures, or the test measures work (steps, commits read) rather than time.
