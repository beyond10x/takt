---
format: aep.planning-md/3
id: story:untracked-cargo-lock-remedy
kind: story
status: draft
title: A tests.run refusal for an untracked Cargo.lock names the remedy
summary: the refusal for a test command that writes an untracked Cargo.lock says to commit or ignore it
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
scope:
- confidence: cited
  path: crates/takt/src/run.rs
- confidence: inferred
  path: crates/takt/tests/untracked_cargo_lock_remedy.rs
revision: 4
---
# Story: untracked-cargo-lock-remedy

## Outcome

A run on a repository whose test command writes a file the repository does not track (`Cargo.lock` with no committed lock file) is refused with the remedy named, so the operator knows what to commit or ignore.

## Context

Found by an independent code review of 2026-10-18, takt at 2a22093 (line numbers unchanged at 2ec88d4). Verified in a trial.

- With no committed `Cargo.lock` and the default `--test-cmd` (`cargo test --quiet`, `crates/takt-cli/src/main.rs:138`), `cargo` writes `Cargo.lock`, the observation's change check refuses it, and the run stops: `StepRefused (tests.run wrote what its validator does not: Cargo.lock)`, exit 3, on every run (`crates/takt/src/run.rs:767-799`).
- The refusal names the path but no remedy.

## Expected before

On takt at 2ec88d4, holds; after the fix, fails:

- `takt run --harness none` on a cargo repository with no committed `Cargo.lock` exits 3, its output contains `tests.run wrote what its validator does not: Cargo.lock`, and the output contains neither `commit` nor `.gitignore` after that phrase.

## Scope

- `crates/takt/src/run.rs`: the refusal after an observation that wrote outside its own files (cited)
- `crates/takt/tests/`: a refusal-text test (inferred)

## Acceptance

On a cargo repository with no committed `Cargo.lock`, the `tests.run` refusal names the remedy: commit `Cargo.lock`, or ignore it, or give a `--test-cmd` that does not write it.
