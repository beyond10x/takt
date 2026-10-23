---
format: aep.planning-md/3
id: story:takt-evaluation-code-retired
kind: story
status: active
title: 'Canon''s evaluation path loom-sdk replaced is gone: no b10x_canon::eval, no decision.rs'
summary: Removal held by a test; fuzz target, ESS citations and AGENTS.md Layout follow
tags:
- loom
relations:
- decomposes: epic:takt-on-loom-sdk
- serves: vision:safe-agentic-coding
- depends_on: story:the-loop-is-run-until-blocked
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-10-22T16:10:52Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-22T16:10:52Z", actor: "human:timo", revision: 4}
---
# Story: takt-evaluation-code-retired

## Outcome

`crates/takt/src` holds none of Canon's evaluation path that loom-sdk replaced: no `b10x_canon::eval` call and no `decision.rs`. `CaseDir::evaluate` stays only for the two pre-checks approval-record:loop-pre-checks-ahead-of-loom keeps. A test keeps it so, and the specification and `AGENTS.md` describe what replaced them. The `Harness` trait and the `harness` module move to story:takt-harness-trait-retired (narrowed 2026-10-23, below).

## Context

- After `story:the-loop-is-run-until-blocked` the run reaches none of it. At a4a2e3e: `use b10x_canon::eval` (`crates/takt/src/case.rs:18`, `:3018`, `:3099-3107`), the `decision` module (`crates/takt/src/decision.rs`, `crates/takt/src/lib.rs:11`), still imported by `crates/takt/src/replay.rs:25` and `crates/takt/src/loop_scorecard.rs:25`, which read decision documents.
- `read_evidence` becomes a `serde_yaml_ng` read of `b10x_canon::model::EvidenceRecord` (canon 0.6.0 `eval/evidence.rs:18-35` does only that); `refusal()` re-runs Canon's evaluation for an error message and goes, with the expectation of `crates/takt/tests/authority_adv1.rs:196` following.
- The fuzz target `harness_reply` fuzzes `ClaudeCode::parse`, which stays; it is unchanged here.

## Narrowed (2026-10-23, planner)

The implementor's scoping at a4a2e3e counted 128 `impl … Harness for` in 74 files under `crates/takt/tests`, 81 files importing `takt::harness`, and 82 `CaseDir::evaluate` calls in 16 test files. Retiring the trait is a design choice (what replaces it, such as `SessionHarness` implementing Loom's `AgentExecutor`) and a change to 74 test files, outside this story's Scope. It moves to story:takt-harness-trait-retired; this story keeps the evaluation path and widens its Scope by the two import lines and the one test expectation it changes.

## Scope

- `crates/takt/src/decision.rs` (removed, `Decision` moves to `crates/takt/src/case.rs`), `crates/takt/src/case.rs`, `crates/takt/src/lib.rs`, `crates/takt/src/replay.rs`, `crates/takt/src/loop_scorecard.rs`, `crates/takt/src/run.rs`
- `crates/takt-cli/src/main.rs`
- `crates/takt/tests/authority_adv1.rs` (the refusal text), and any test file whose `use takt::decision` import moves
- `ess/domains/takt.yaml`, `ess/domains/takt.citations.tsv` (citations of removed lines re-pointed)
- `AGENTS.md` (§ Layout)
- new: `crates/takt/tests/no_own_evaluation.rs`

## Acceptance

1. `cargo test -p b10x-takt --test no_own_evaluation` passes: it reads `crates/takt/src` and fails on a `b10x_canon::eval` path or a `decision.rs`, and on an `evaluate` method of `CaseDir` unless `run.rs` runs `Shape::only_behind` and `Shape::unperformable` over the loop's one evaluate call. The loop's other reads of that decision are finding:loop-reads-its-own-decision, held by story:takt-harness-trait-retired (amended 2026-10-23 on the wave-34 review).
2. `cargo check` in `fuzz/` passes.
3. `ess specify validate --path ess` and `python3 -I scripts/check-citations --require-digests` pass, with no citation into a removed line.
4. `AGENTS.md` § Layout names no `decision` module and names the session-executor crate.
5. `task check` passes.

## Kept until a Loom contract change

approval-record:loop-pre-checks-ahead-of-loom keeps two of takt's checks ahead of every Loom step:
`Shape::only_behind` (ApprovalRequired, exit 4) and `Shape::unperformable` (Unperformable, exit 7),
with the evaluation they read. That part stays, and acceptance 1 allows exactly what those two
checks need, until Loom's documented contract changes (loom `AGENTS.md:329-338` and `:378`); that
change is the owner's.

## Out of Scope

The `Harness` trait and the `harness` module (story:takt-harness-trait-retired). The design document, README and changelog (`story:design-names-loom-as-the-runtime`).
