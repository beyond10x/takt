---
format: aep.planning-md/3
id: story:takt-evaluation-code-retired
kind: story
status: draft
title: 'The evaluation code loom-sdk replaced is gone: no decision.rs, no Harness trait, no CaseDir::evaluate'
summary: Removal held by a test; fuzz target, ESS citations and AGENTS.md Layout follow
tags:
- loom
relations:
- decomposes: epic:takt-on-loom-sdk
- serves: vision:safe-agentic-coding
- depends_on: story:the-loop-is-run-until-blocked
revision: 2
---
# Story: takt-evaluation-code-retired

## Outcome

`crates/takt/src` holds none of the evaluation code loom-sdk replaced: no `b10x_canon::eval` call, no `decision.rs`, no `Harness` trait and no `CaseDir::evaluate`. A test keeps it so, and the specification and `AGENTS.md` describe what replaced them.

## Context

- After `story:the-loop-is-run-until-blocked` the run reaches none of it. Today it is: `use b10x_canon::eval` (`crates/takt/src/case.rs:15`), `CaseDir::evaluate` (`case.rs:2323`), the `decision` module (`crates/takt/src/decision.rs`, `crates/takt/src/lib.rs:10`) and the `Harness` trait (`crates/takt/src/harness.rs:69`).
- The fuzz target `harness_reply` (`fuzz/Cargo.toml:27-28`) fuzzes the reply parser of the `Harness` adapter; it moves to the parser the session executor reads a reply with, or is removed with its reason.
- `AGENTS.md` § Layout names the `decision` and `harness` modules (`AGENTS.md:23`).

## Scope

- `crates/takt/src/decision.rs` (removed), `crates/takt/src/case.rs`, `crates/takt/src/harness.rs`, `crates/takt/src/lib.rs`, `crates/takt/src/run.rs`
- `crates/takt-cli/src/main.rs`
- `fuzz/Cargo.toml`, `fuzz/fuzz_targets/harness_reply.rs`
- `ess/domains/takt.yaml`, `ess/domains/takt.citations.tsv` (citations of removed lines re-pointed)
- `AGENTS.md` (§ Layout)
- new: `crates/takt/tests/no_own_evaluation.rs`

## Acceptance

1. `cargo test -p b10x-takt --test no_own_evaluation` passes: it reads `crates/takt/src` and fails on a `b10x_canon::eval` path, a `decision.rs`, a `trait Harness` or an `evaluate` method of `CaseDir`.
2. `cargo check` in `fuzz/` passes, and no fuzz target names a removed item.
3. `ess specify validate --path ess` and `python3 -I scripts/check-citations --require-digests` pass, with no citation into a removed line.
4. `AGENTS.md` § Layout names no `decision` or `harness` module and names the session-executor crate.
5. `task check` passes.

## Kept until a Loom contract change

approval-record:loop-pre-checks-ahead-of-loom keeps two of takt's checks ahead of every Loom step:
`Shape::only_behind` (ApprovalRequired, exit 4) and `Shape::unperformable` (Unperformable, exit 7),
with the evaluation they read. That part stays, and acceptance 1 allows exactly what those two
checks need, until Loom's documented contract changes (loom `AGENTS.md:329-338` and `:378`); that
change is the owner's.

## Out of Scope

The design document, README and changelog (`story:design-names-loom-as-the-runtime`).
