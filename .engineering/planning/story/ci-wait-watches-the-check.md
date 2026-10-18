---
format: aep.planning-md/3
id: story:ci-wait-watches-the-check
kind: story
status: draft
title: takt waits for CI by watching the check, not by fixed 30 s sleeps
relations:
- decomposes: epic:composable-agentic-work
- informed_by: story:ci-pending-waits
revision: 1
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
