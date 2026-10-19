---
format: aep.planning-md/3
id: story:run-child-sweep-survives-setsid
kind: story
status: draft
title: No process a test command started writes into the tree after run_child returns, under load
tags:
- priority:high
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
revision: 2
---
## Outcome

`run_child`'s guarantee holds under load: when it returns, no process the child started, including one that called `setsid`, writes into the tree afterwards. `validator_confinement_adv1.rs:428` (`run_child_leaves_no_process_that_writes_on`) passes on an idle machine and under a load of 30 or more.

## Context

The developer reported on 2026-10-19 that this test failed once under load in a gate on 26bb59d (load 26-38). A `setsid`'d process wrote `late` after `run_child` returned. It passed when run alone. The developer did not loosen it: the test asserts a guarantee, not a timing premise. Inferred, not traced: the sweep that kills the child's processes races a process that leaves the session or process group before the sweep reads it. A race in this guarantee lets a test command keep writing to the tree after takt has judged it.

## Scope

- `crates/takt/src/validator.rs`: `run_child` and its sweep (inferred)
- `crates/takt/src/forge.rs`: `sweep_left`, if the sweep lives there (inferred)

## Acceptance

`validator_confinement_adv1.rs:428` passes 20 of 20 while two other `cargo test --workspace` processes run, and a variant whose child double-forks and calls `setsid` before the sweep also writes nothing after `run_child` returns.
