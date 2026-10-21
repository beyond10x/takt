---
format: aep.planning-md/3
id: story:a-sweep-reaps-no-other-calls-child
kind: story
status: draft
title: A sweep reaps no child another call is waiting on
tags:
- finding
refs:
- provider: finding
  reference: a-sweep-reaps-no-other-calls-child
relations:
- decomposes: epic:composable-agentic-work
revision: 1
---
## Outcome

A validator's command is reaped only by the call that started it: no other sweep in the same
process takes its exit status, and `cargo test` of `b10x-takt` does not fail on it.

## Context

check.yml run 39569813626 on main (02130a98, 2026-10-21) failed in test-library:
`a_command_bound_gate_with_its_project_file_is_evaluated` (`crates/takt/tests/gate_validator.rs:777`)
stopped `NoProgress` because the gate's command ended in "No child processes (os error 10)". The
release run's gate passed the same test at the same commit (run 39569846703). Inferred, not
traced: `sweep_left` (`crates/takt/src/forge.rs:1000-1011`) kills and reaps what is in a process
group no running `run_child` call leads, and a command started with `process_group(0)` outside
`run_child` while another test's sweep runs would be reaped there, leaving ECHILD to its own
`wait`.

## Scope

- `crates/takt/src/forge.rs`, the gate command's spawner, tests.

## Acceptance

1. One instance traced: which wait took the child's status.
2. A test that runs a sweep beside a gate command fails before the fix and passes after, 50 times.
