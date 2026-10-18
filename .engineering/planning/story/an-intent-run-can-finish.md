---
format: aep.planning-md/3
id: story:an-intent-run-can-finish
kind: story
status: draft
title: A run started with --intent can reach its accepted outcome
relations:
- informed_by: review-result:variety-runs-a0b8576
- decomposes: epic:composable-agentic-work
revision: 1
---
## Outcome

A case opened by `takt run --intent` (protocol `software.change/1`, the default without `--protocol`) can reach `accepted` once its change is merged under a grant: every evidence kind its accepted outcome requires is written by some validator, or the default protocol for `--intent` requires only kinds takt's validators write.

## Context

From the planner's variety runs of 2026-10-18 (.engineering/planning/review-result/variety-runs-a0b8576.md), takt at the promoted a0b8576: with no `--protocol`, `--intent` opens on `software.change/1` (`crates/takt-cli/src/main.rs:1164`). Its only outcome reachable without an explicit decision is `accepted`, which needs `build_provenance`, `operational_observation` and `objective_observation`; takt's validators write `test_result`, `gate_result`, `pull_request`, `ci_result` and `merge_observation` only (`crates/takt/src/validator.rs`). After `takt approve` on a worktree task, two resumes each spent one `repository.inspect` step and stopped NoProgress (85,594 and 110,557 tokens) and the base never moved. The run cannot finish; every `--intent` run ends at the approval or in NoProgress.

Open: an offline `expected_before` needs a fake harness that edits, commits and then resumes after an approval; to be written with the fix.

## Scope

- `crates/takt-cli/src/main.rs`: the protocol `--intent` opens (cited)
- the built-in `software.change/1` protocol and `crates/takt/src/validator.rs`: the kinds its accepted outcome needs against the kinds validators write (cited)

## Acceptance

On scenario 1 with `--intent` and the fake forge, a run that edits, passes its tests, is granted the merge and resumes ends `Completed("accepted")`; a load-time check refuses any built-in protocol whose accepted outcome needs a kind no registered validator writes.
