---
format: aep.planning-md/3
id: story:ci-wait-watches-the-check
kind: story
status: implemented
title: takt waits for CI by watching the check, not by fixed 30 s sleeps
relations:
- decomposes: epic:composable-agentic-work
- informed_by: story:ci-pending-waits
- serves: vision:safe-agentic-coding
- depends_on: story:story-delivery-tests-first
- informed_by: story:no-repush-after-head-lag
scope:
- confidence: cited
  path: crates/takt/src/run.rs
- confidence: cited
  path: crates/takt/tests/ci_wait_backoff.rs
- confidence: cited
  path: ess/domains/takt.yaml
revision: 14
transitions:
- {from: "draft", to: "proposed", at: "2026-10-18T23:34:16Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-18T23:34:16Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-11-03T15:07:43Z", actor: "human:timo", revision: 14, decided_on: {"recorded":{"test_result":1,"review_outcome":1}}}
---
## Outcome

`ci.read` returns within a few seconds of CI finishing: it watches the check (or polls at a short, growing interval capped by `--hours`), instead of sleeping a fixed 30 s between reads.

## Context

docs/runs/2026-10-18-efficiency-analysis.md (planner-side analysis of runs T-14..18 and T-24..28 at a0b8576 against the isolated Claude arm I-4..6): two fixed 30 s sleeps (`POLL`, `crates/takt/src/run.rs` at a0b8576 :485 and :1437) cost 60 s of a 127 s run (T-16); CI itself took 7 s, so 44 s were idle after CI had finished. The isolated arm's single `gh pr checks --watch` waited 11.4 s. story:ci-pending-waits added the wait; no story covers its interval.

## Scope

- `crates/takt/src/run.rs`: `POLL` and the CI wait loop (cited)
- `crates/takt/src/forge.rs`: the checks read (inferred)

## Acceptance

On scenario 1 against the real forge, the time from CI completing to the `ci_result` record is under 5 s in each of 5 runs; a CI that never reports still ends at the `--hours` budget with a stop.

## Design

Owner decision of 2026-10-19 ("takt needs some protocol node which waits on PR/MR becoming green, and if the remote repo is github, we require gh cli ... we do not want to expose the cli as tool to takt"):

- `ci.wait` is a protocol action of `story.delivery` taken by takt (deterministic), never offered to the harness; the harness keeps no shell and no `gh`.
- For forge `github`, `takt run` refuses before the first step, naming `gh`, unless `gh` is on PATH and `gh auth status` exits 0.
- Before waiting it checks that the pull request's head (`gh pr view <n> --json headRefOid`) equals the pushed revision; a lagging head is waited for, never re-pushed (story:no-repush-after-head-lag).
- It polls `gh pr checks <n> --json name,state,bucket,startedAt,completedAt,link` every 2 s, growing to at most 15 s, until every check is complete or one fails. `gh pr checks --watch` prints text only (gh 2.100.0 `--help`), so the JSON form is polled; it is also what gives story:ci-progress-events its per-check states. `gh` runs as takt's child with a minimal environment and is killed at the `--hours` deadline, which ends the run with the budget stop.
- `ci_result` records every check (name, bucket, link) bound to the head revision.
- `Run::POLL` (`crates/takt/src/run.rs`) goes.

Acceptance, added to the one above: a run where `gh` is absent exits before the first step naming it; the harness invocation's tool list contains no shell.

Ordering: after story:story-delivery-tests-first, which writes `story.delivery/2`; `ci.wait` is an action of that protocol.

## Decided in wave 49

Wave 49, the coordinator's decision inside the approved wave (2026-11-03):

- The `gh` parts of the Design (a `gh` preflight, `gh pr view`, `gh pr checks` as takt's child)
  are superseded: GitHub is reached through the Connectors forge (`crates/takt/src/forge/connectors.rs`,
  wave 48) and the command forge, and credentials never enter takt's children. takt takes no `gh`
  dependency; the forge seam `Forge::checks` stays.
- Delivered: the forge wait is a backoff, 2 s growing 1.5x per consecutive wait on the same
  awaited set to a 15 s cap, reset when the awaited set or the reported head changes, cut to the
  `--hours` budget; `Run::POLL` (30 s) is gone (`crates/takt/tests/ci_wait_backoff.rs`).
- Not in this change: per-check name, state and link on `ci_result` (needs the evidence format and
  every forge adapter; story:ci-progress-events covers per-check states). The 5-run measurement
  against the real forge is a trial, left to the trial agent.
- `protocols/story-delivery/2.yaml` needed no change; `ci.read` is already taken by takt.
