---
format: aep.planning-md/3
id: story:confinement-seam
kind: story
status: draft
title: Every child process of a run starts through a Confinement adapter
summary: The fifth seam with a none adapter; a refusing test adapter proves nothing escapes it
tags:
- loom
relations:
- depends_on: story:forge-pull-request-evidence
- serves: vision:safe-agentic-coding
- decomposes: epic:takt-on-loom-sdk
- depends_on: story:validators-perform-as-effects
scope:
- confidence: cited
  path: crates/takt-cli/src/main.rs
- confidence: inferred
  path: crates/takt/src/confinement.rs
- confidence: inferred
  path: crates/takt/src/forge.rs
- confidence: inferred
  path: crates/takt/src/frontier.rs
- confidence: cited
  path: crates/takt/src/git.rs
- confidence: cited
  path: crates/takt/src/harness.rs
- confidence: inferred
  path: crates/takt/src/lib.rs
- confidence: cited
  path: crates/takt/src/run.rs
- confidence: cited
  path: crates/takt/src/validator.rs
- confidence: inferred
  path: crates/takt/tests/confinement.rs
revision: 19
---
# Story: confinement-seam

## Outcome

Every child process a run starts goes through one seam, `crates/takt/src/confinement.rs`, and the test command runs under the `SubstrateRunner` of loom-sdk: a fixed `PATH` of read-only toolchain roots, only the build directory writable, no network. A test command the Substrate profile cannot serve is refused by name, never run unconfined. The routes that close by confining the test command are closed, and their tests pass.

## Context

- Rescoped into `epic:takt-on-loom-sdk` (2026-10-20): the confining backend is the one loom-sdk exports, not one of takt's own. Confining the harness session, git, the forge and `aep` is `story:harness-child-confined`, blocked on loom-sdk.
- Today children are started with `std::process::Command` in `crates/takt/src/forge.rs`, `frontier.rs`, `git.rs`, `harness.rs`, `improvement.rs`, `promotion.rs`, `run.rs` and `validator.rs` (`grep -n Command::new crates/takt/src/*.rs`). Every one already gets an environment without credentials it does not need, a timeout and a bounded output tail (`crates/takt/tests/validator_confinement.rs:1-8`), and the test command starts in a user and mount namespace of its own (`crates/takt/tests/test_command_namespace_refused.rs:1-4`).
- After `story:harness-proposes-takt-performs` the harness session is started by metaharness, which takes the process runner from its embedder (metaharness 0.9.3 `crates/metaharness/src/process.rs:74-94`, `ProcessRunner`); the session executor is handed one that goes through the seam, and its crate starts no process of its own.
- loom-sdk exports `SubstrateRunner` and `TestRunner` for a test command (Loom 0.26.0 `crates/loom-sdk/src/lib.rs:77-80`). Its profile serves only a Rust toolchain installed under `RUSTUP_HOME` (`crates/loom-intake-slice/src/confinement.rs:559-562`) with only `target` writable (`:26`); a Python test command is refused by name until loom-sdk serves other toolchains. takt builds no substitute.
- Path resolution: `story:step-cannot-forge-its-validators` refuses a guarded path whose resolution crosses a link, checked per component with `symlink_metadata`, which is not race-free. This story replaces it with `openat2` and `RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS | RESOLVE_NO_XDEV` (Linux), with an fd-based snapshot and a named refusal where the syscall is not available.

## Routes this story closes

From the reviews of `story:step-cannot-forge-its-validators`, `story:loop-scorecard` and `story:promotion` (2026-10-18 and 2026-10-19), each through the test command:

- M8: a step puts a `cargo` wrapper that exits 0 on `test` in a user-writable directory early on `PATH`; the case was accepted with a forged pass. Closed by the fixed `PATH` of read-only toolchain roots. Ignored test: `crates/takt/tests/step_cannot_forge_its_validators.rs:1444`.
- M19: the test command runs files the harness wrote, with `HOME` and `XDG_STATE_HOME` passed through (`crates/takt/src/validator.rs:275-279` at the time); a probe wrote a record into `$XDG_STATE_HOME/takt/promotions/`. Closed by a scratch `HOME` and XDG directories and no write outside the tree.
- M20: the test command writes a loose object over an existing blob id in `.git/objects/`, and a later loop case snapshots the planted definitions. Closed by no write access to the object store.
- C6 and C8 (the promotion review at 2d10b7f): the newest promotion record rewritten, and a trailing `.refused.json` deleted, both through write access to the operator's store that a candidate has only through M19.

The routes that close only by confining the harness child (M9, M16, M17, M18) are `story:harness-child-confined`'s.

## Domain relations

- Confinement adapter: `takt.work.ConfinementName` (`ess/domains/takt.yaml`).
- Step → EvidenceProducer: a deterministic step is performed by exactly one validator — inferable (`ess/domains/takt.yaml`, `takt.work.Step` relation `producer`); the test command is spawned by that validator.

## Scope

- new: `crates/takt/src/confinement.rs`
- `crates/takt/src/lib.rs` (the module registered)
- `crates/takt/src/forge.rs`, `frontier.rs`, `git.rs`, `harness.rs`, `improvement.rs`, `promotion.rs`, `run.rs`, `validator.rs` (spawn through the seam)
- `crates/<session-executor>/` (the metaharness process runner taken from its embedder)
- `crates/takt-cli/src/main.rs` (`--confinement none | substrate`)
- `crates/takt/tests/step_cannot_forge_its_validators.rs` (M8 no longer ignored)
- new: `crates/takt/tests/confinement.rs`

## Acceptance

1. No module of `crates/takt/src` but `confinement.rs`, and no source of the session-executor crate, names `std::process::Command`; a test in `crates/takt/tests/confinement.rs` reads the sources and fails on any other occurrence.
2. With a test adapter that refuses every program, `takt run --harness none` starts no process at any spawn site, writes no evidence record and stops with the refusal named.
3. Under `--confinement substrate` on the scratch crate, the test command runs through the `SubstrateRunner` of loom-sdk and the run writes the `test_result` record it writes under `none`.
4. `a_cargo_wrapper_on_the_path_outside_the_tree_earns_no_pass` (M8) is no longer ignored and passes under `--confinement substrate`; tests for M19 and M20 show the write refused.
5. Under `--confinement substrate`, a Python test command ends the run with a refusal that names the toolchain, and starts no child unconfined.
6. The first line the run prints and its events name the confinement adapter in use.

## Out of Scope

Confining the harness session, git, the forge and `aep` (`story:harness-child-confined`); toolchains other than Rust under Substrate; resource limits beyond those the profile of loom-sdk sets; which adapter is the default.
