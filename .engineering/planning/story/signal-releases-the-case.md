---
format: aep.planning-md/3
id: story:signal-releases-the-case
kind: story
status: draft
title: SIGINT or SIGTERM releases the case and kills the harness child
summary: a signal kills the child group, undoes the step and releases run.lock; a lock with a dead pid is stale
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
scope:
- confidence: inferred
  path: crates/takt-cli/src/main.rs
- confidence: cited
  path: crates/takt/src/case.rs
- confidence: cited
  path: crates/takt/src/forge.rs
- confidence: cited
  path: crates/takt/src/run.rs
- confidence: inferred
  path: crates/takt/tests/signal_releases_the_case.rs
revision: 4
---
# Story: signal-releases-the-case

## Outcome

A run stopped by Ctrl-C or SIGTERM lets go of its case: the harness child and its process group are killed, the step in progress is undone, and `run.lock` is released, so the next `takt run` resumes the case. A lock left by a run that died anyway is recognised as stale.

## Context

Found by an independent code review of 2026-10-18, takt at 2a22093 (line numbers unchanged at 2ec88d4). Verified by the review with SIGTERM.

- takt installs no signal handler. Each child runs in its own process group (`crates/takt/src/forge.rs:451`), so a signal to takt does not reach it, and the lock is released only when `run_held` returns (`crates/takt/src/run.rs:637-644`).
- After Ctrl-C or SIGTERM, `run.lock` stays held and the harness child is orphaned, editing the tree for up to its 30-minute timeout (`crates/takt/src/harness.rs:73`).
- The next run exits 1 with "the case … is held … if neither is in progress, remove the file" (`crates/takt/src/case.rs:150-154`); once the file is removed, it stops with `StepRefused` for the uncommitted changes the orphan left (`run.rs:722-732`).
- Related: `design:continuous-orchestration` § 9 (the dispatcher's `locks` row) records the holder's pid in `run.lock` and reclaims a lock whose pid is dead.

## Expected before

On takt at 2ec88d4, holds; after the fix, fails:

- A test that starts `takt run` with a fake harness that sleeps, sends SIGTERM to takt during the step, and waits for takt to exit: `.engineering/cases/<case>/run.lock` still exists and the fake harness's process is still alive.

## Scope

- `crates/takt/src/run.rs`: `Run::run`, release and undo on a signal (cited)
- `crates/takt/src/forge.rs`: `run_child`, killing the running groups on a signal (cited)
- `crates/takt/src/case.rs`: `take_hold`, the holder's pid and stale-lock detection (cited)
- `crates/takt-cli/src/main.rs`: installing the handler (inferred)
- `crates/takt/tests/`: a SIGTERM test (inferred)

## Acceptance

After SIGTERM or SIGINT to a run during a harness step, takt exits with no process of the step's group alive, the tree as it was before the step and no `run.lock` in the case, and the next `takt run` resumes the case; a `run.lock` whose recorded pid is dead does not block the next run.
