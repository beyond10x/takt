---
format: aep.planning-md/3
id: story:adopted-orphans-are-reaped-while-a-child-runs
kind: story
status: active
title: An orphan takt adopted is reaped once it ends, while the test command still runs
summary: no zombie of a test's killed process group outlives its test
tags:
- priority:high
relations:
- decomposes: epic:downstream-story-delivery
- serves: vision:safe-agentic-coding
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-10-28T13:11:31Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-28T13:11:31Z", actor: "human:timo", revision: 3}
---
## Outcome

A process that a test command started and orphaned, which takt adopted as the subreaper of what it starts, is reaped as soon as it has ended, while the test command still runs: it does not stay a zombie until the run ends, so a test that waits for a killed process group to empty sees it empty, under takt as on a plain shell.

## Context

A run of takt 0.17.0 on beyond10x/tools (2026-10-28; records outside the repository) stopped `RedBase` (exit 3) twice on tests of `b10x-tool-process` that pass outside takt: `tests::the_kill_at_the_bound_names_a_group_whose_ended_shell_is_not_yet_reaped` and `a_descendant_in_the_group_that_outlives_its_shell_does_not_hold_the_call_past_the_bound`. takt sets `PR_SET_CHILD_SUBREAPER` (`crates/takt/src/forge.rs`, `leftovers::adopt`) and reaps what it adopted only in a sweep after a child ended, so an orphan the tests killed stays takt's zombie and its process group still answers signals.

## Scope

- `crates/takt/src/forge.rs`: reaping ended adopted processes while a child runs
- `crates/takt/tests/`: a reproduction

## Acceptance

- `adopted_orphans_are_reaped`: a command run as takt runs a test command, whose shell starts a process in its process group, is killed and leaves it orphaned, waits until signal 0 to the group fails; it exits 0 within its bound under takt. On takt 0.17.0 it times out.
- No child another thread waits on is reaped by takt: `sweep_spares_other_children` passes.
