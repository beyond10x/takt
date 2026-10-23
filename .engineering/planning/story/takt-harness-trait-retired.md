---
format: aep.planning-md/3
id: story:takt-harness-trait-retired
kind: story
status: draft
title: 'takt has no Harness trait: each harness is a Loom AgentExecutor'
relations:
- decomposes: epic:takt-on-loom-sdk
- serves: vision:safe-agentic-coding
- depends_on: story:takt-evaluation-code-retired
revision: 2
---
# Story: takt-harness-trait-retired

## Outcome

takt has no `Harness` trait of its own: each harness is a Loom `AgentExecutor` that proposes, and the test fakes are executors too. `crates/takt/src` has no `trait Harness`, and `AGENTS.md` § Layout names no `harness` module.

## Context

- Split from story:takt-evaluation-code-retired on 2026-10-23. At a4a2e3e: `trait Harness` at `crates/takt/src/harness.rs:80`; 128 `impl … Harness for` in 74 files under `crates/takt/tests`; `takt::harness` imported in 81 files.
- ESS citations `ess/domains/takt.citations.tsv` rows 58-61 and 111-115 cite `trait Harness` and `StepContext` in `harness.rs`.
- The fuzz target `harness_reply` (`fuzz/Cargo.toml`) fuzzes `ClaudeCode::parse`; it moves to the parser the executor reads a reply with, or goes with its reason.

## Open

What replaces the trait (for example `SessionHarness` implementing `AgentExecutor`, and a scripted executor for the tests) is decided in this story's design note before code.

## Scope

- `crates/takt/src/harness.rs`, `crates/takt/src/run.rs`, `crates/takt/src/lib.rs`, `crates/takt-cli/src/main.rs`
- `crates/takt/tests/**`, `crates/takt-cli/tests/**`
- `fuzz/Cargo.toml`, `fuzz/fuzz_targets/harness_reply.rs`
- `ess/domains/takt.yaml`, `ess/domains/takt.citations.tsv`
- `AGENTS.md` (§ Layout)
- `crates/takt/tests/no_own_evaluation.rs` (gains the `trait Harness` check)

## Acceptance

1. `cargo test -p b10x-takt --test no_own_evaluation` fails on a `trait Harness` in `crates/takt/src`, and passes.
2. `cargo check` in `fuzz/` passes, and no fuzz target names a removed item.
3. `ess specify validate --path ess` and `python3 -I scripts/check-citations --require-digests` pass, with no citation into a removed line.
4. `AGENTS.md` § Layout names no `harness` module.
5. `task check` passes.

## Finding (2026-10-23, from the implementation of story:takt-evaluation-code-retired)

`finding:loop-reads-its-own-decision`. The loop reads the decision of `CaseDir::evaluate` for more than the two pre-checks approval-record:loop-pre-checks-ahead-of-loom keeps: the frontier line, `legitimate()`, `admissible()`, `approval_required()` and the claims (`crates/takt/src/run.rs:1381`, `:1424`, `:10263-10277`, `:10354`, `:10582` at the wave-34 tree). Moving those reads onto Loom's frontier is part of making each harness an executor here.

- `expected_before`: `grep -cE 'decision\.(legitimate|admissible|approval_required|frontier_line)\(' crates/takt/src/run.rs` prints a number above 0.
