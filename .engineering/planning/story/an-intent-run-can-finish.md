---
format: aep.planning-md/3
id: story:an-intent-run-can-finish
kind: story
status: draft
title: A run started with --intent can reach its accepted outcome
relations:
- informed_by: review-result:variety-runs-a0b8576
- decomposes: epic:composable-agentic-work
- depends_on: story:pr-opens-when-change-is-done
scope:
- confidence: cited
  path: crates/takt-cli/src/main.rs
- confidence: cited
  path: crates/takt-cli/tests/protocol_defaults_cli.rs
- confidence: cited
  path: crates/takt/src/protocol.rs
- confidence: inferred
  path: crates/takt/src/run.rs
- confidence: inferred
  path: crates/takt/tests
- confidence: inferred
  path: ess/domains/takt.citations.tsv
- confidence: inferred
  path: ess/domains/takt.yaml
- confidence: inferred
  path: protocols
revision: 4
---
## Outcome

A case opened by `takt run --intent` (protocol `software.change/1`, the default without `--protocol`) can reach `accepted` once its change is merged under a grant: every evidence kind its accepted outcome requires is written by some validator, or the default protocol for `--intent` requires only kinds takt's validators write.

## Context

From the planner's variety runs of 2026-10-18 (.engineering/planning/review-result/variety-runs-a0b8576.md), takt at the promoted a0b8576: with no `--protocol`, `--intent` opens on `software.change/1` (`crates/takt-cli/src/main.rs:1164`). Its only outcome reachable without an explicit decision is `accepted`, which needs `build_provenance`, `operational_observation` and `objective_observation`; takt's validators write `test_result`, `gate_result`, `pull_request`, `ci_result` and `merge_observation` only (`crates/takt/src/validator.rs`). After `takt approve` on a worktree task, two resumes each spent one `repository.inspect` step and stopped NoProgress (85,594 and 110,557 tokens) and the base never moved. The run cannot finish; every `--intent` run ends at the approval or in NoProgress.

Open: an offline `expected_before` needs a fake harness that edits, commits and then resumes after an approval; to be written with the fix.

## Scope

Derived 2026-10-18 by `story-scoper` for story:an-intent-run-can-finish and story:no-merge-approval-without-a-change together; one unit, built after wave 12 merges into main. Line numbers: main / wave/2026-10-18-w12 head.

- **Decided (planner, 2026-10-18):** `takt run --intent` opens a new takt-embedded protocol, `intent.delivery/1`, shaped on `story.delivery/1` without the planning-store story: its `accepted` needs only kinds a built-in validator writes, and `repository.merge` (and `pr.open`) require `change.made` in their precondition. `software.change/1` stays available through `--protocol` and for resumed cases. The load-time check refuses a protocol whose accepted outcome needs a kind no registered validator writes only for takt's embedded protocols and the `--intent` default, not for `--protocol software.change/1`.
- **Primary surface:** the protocol `--intent` opens by default: `crates/takt-cli/src/main.rs:380` / `:391` (`(None, Some(_)) => SOFTWARE_CHANGE`), constant at `:1164` / `:1181` — cited
- **Files:** `crates/takt/src/protocol.rs:33` (`EMBEDDED`), where the new protocol is registered — cited
- **Files:** `protocols/intent-delivery/1.yaml` (new), on the pattern of `protocols/story-delivery/1.yaml` at the wave 12 head (`change.read`, `change_record`) — inferred
- **Files:** `crates/takt-cli/src/main.rs:424` / `:435`, the input defaults that apply only to `SOFTWARE_CHANGE` — cited
- **Files:** the load-time check in `crates/takt/src/run.rs` `Shape::of` (wave 12 head `:398`) or `crates/takt/src/protocol.rs`, reading `validator::BUILT_IN` (wave 12 head `validator.rs:1533`) — inferred
- **Not the fix site:** `crates/takt/src/case.rs:1364` / `:1456`; `software.change/1` lives in the engineering-protocols dependency (0.9.0, `protocols/software-change/1.yaml`: accepted at `:277-285`, `repository.merge` at `:256-265`) and is not changed — cited
- **Also likely:** `crates/takt-cli/tests/protocol_defaults_cli.rs:302-312` and the test at `crates/takt-cli/src/main.rs:1810-1838`, which pin today's default; a library test with the test `Forge` and a fake harness that edits, commits and resumes — cited / inferred
- **Also likely:** `ess/domains/takt.citations.tsv:14` (hashes `EMBEDDED`), `ess/domains/takt.yaml`; `README.md:83` and `.agents/planner-agent.md:171` (they name the `--intent` default) — cited
- **Would collide with:** `crates/takt-cli/src/main.rs` (loop-scorecard's registration); `crates/takt/src/run.rs` (hours-bounds, step-guard) only if the check goes in `Shape::of`; `ess/domains/*`; `crates/takt-cli/tests/protocol_defaults_cli.rs`. Not `forge.rs`, `harness.rs`, `validator.rs`.
- **Open, for the implementor:** `GateRun` is registered only when a gates document exists (`main.rs:545-554`), so the new protocol's `pr.open` must not require `gate.green` unconditionally; the CLI has no fake forge (`forge_of`), so the acceptance runs as a library test.
- **Safety fact:** a resumed case keeps the protocol it opened with (`refuse_for_resume`, `main.rs:383-388` / `:394-399`) — step 2, unproven

## Acceptance

On scenario 1 with `--intent` and the fake forge, a run that edits, passes its tests, is granted the merge and resumes ends `Completed("accepted")`; a load-time check refuses any built-in protocol whose accepted outcome needs a kind no registered validator writes.
