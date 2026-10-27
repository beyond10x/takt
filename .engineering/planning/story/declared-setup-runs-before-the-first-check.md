---
format: aep.planning-md/3
id: story:declared-setup-runs-before-the-first-check
kind: story
status: draft
title: A run executes its declared setup commands before the red-base check
summary: setup key, SetupFailed stop before any model call
tags:
- priority:medium
relations:
- serves: vision:safe-agentic-coding
- decomposes: epic:downstream-story-delivery
scope:
- confidence: inferred
  path: crates/takt/src/main.rs
- confidence: inferred
  path: crates/takt/src/run.rs
- confidence: inferred
  path: crates/takt/src/run_config.rs
- confidence: inferred
  path: crates/takt/tests
- confidence: inferred
  path: ess/domains/takt.yaml
revision: 2
---
# Story: declared-setup-runs-before-the-first-check

## Outcome

A run-file key `setup` lists `{name, argv}` commands.
- At open, takt runs each once, before the red-base check and before any model call, under the test command's
  environment and confinement. Each argv runs as a process, never a shell string.
- Each command's duration is recorded, and its output goes to the state dir.
- A failure is a new stop, `SetupFailed { name, exit }`, at exit 1.
- On resume, setup runs again only when the base commit changed.

## Context

- A downstream trial (2026-10-27): the operator installed dependencies by hand before every trial. takt has no setup
  surface (`RunConfig` keys, `crates/takt/src/run_config.rs:71-91`, 110251a).
- The continuous-orchestration Workspace seam runs the same setup in each child's tree
  (`story:the-workspace-seam-enters-and-leaves-managed-worktrees`).

## Acceptance

Each is a named conformance scenario run by `task check`.

1. `setup::a_failed_setup_stops_before_a_model_call`: a setup command that exits 3 gives exit 1, a stop naming the
   command and `3`, and 0 requests to the fake model endpoint.
2. `setup::setup_runs_before_the_red_base_check`: the event order is setup, then the red-base check.
3. `setup::a_resume_on_the_same_base_skips_setup`, and one on a moved base runs it again.
4. `setup::an_unknown_setup_key_is_refused_at_its_line`.

## Scope

- `crates/takt/src/run_config.rs` (`setup` key)
- `crates/takt/src/run.rs` (open and resume; the new stop)
- `crates/takt/src/main.rs` (exit code)
- `ess/domains/takt.yaml` (the `SetupFailed` stop, before code)
- `crates/takt/tests/` (conformance scenarios)
