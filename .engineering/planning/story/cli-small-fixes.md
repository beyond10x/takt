---
format: aep.planning-md/3
id: story:cli-small-fixes
kind: story
status: archived
title: 'Three small CLI fixes: hours bound a step, scenario exit codes, branch-safe case ids'
summary: --hours bounds the harness step, scenario run --help drops nothing ready, check_id uses check-ref-format
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
scope:
- confidence: cited
  path: crates/takt-cli/src/main.rs
- confidence: cited
  path: crates/takt/src/case.rs
- confidence: cited
  path: crates/takt/src/harness.rs
- confidence: cited
  path: crates/takt/src/run.rs
- confidence: inferred
  path: crates/takt/tests/cli_small_fixes.rs
revision: 5
transitions:
- {from: "draft", to: "archived", at: "2026-10-18T18:34:14Z", actor: "human:timo", revision: 5}
---
# Story: cli-small-fixes

## Outcome

Three small CLI defects go: `--hours` bounds a harness step too, `takt scenario run --help` lists only the exit codes that verb has, and a `--case-id` git cannot use as a branch is refused before the run.

## Context

Found by an independent code review of 2026-10-18, takt at 2a22093 (line numbers unchanged at 2ec88d4). Each item is low.

1. `--hours` is checked between steps only (`crates/takt/src/run.rs:621-625`); a harness step keeps its own 30-minute timeout (`crates/takt/src/harness.rs:73`), so one step can overrun the budget by up to 30 minutes.
2. `takt scenario run` shows the shared `EXIT_CODES` (`crates/takt-cli/src/main.rs:56`), whose exit 0 line includes `frontier: nothing ready` (`main.rs:205-211`); that verb never prints it.
3. `check_id` (`crates/takt/src/case.rs:95-121`) admits ids such as `foo.` or `a~b`, which then fail in `git checkout -b takt/<id>` (`crates/takt/src/git.rs:151`) after the run started.

## Expected before

On takt at 2ec88d4, each holds; after the fix, each fails:

- `takt scenario run --help` output contains `frontier: nothing ready`.
- `check_id("foo.")` and `check_id("a~b")` return `Ok`, while `git check-ref-format --branch takt/foo.` exits non-zero.
- With a fake harness that sleeps 10 s and `--hours 0.001`, the run's step takes the full 10 s (the harness child is not given the remaining budget as its timeout).

## Scope

- `crates/takt/src/run.rs`: the remaining budget passed to the harness step (cited)
- `crates/takt/src/harness.rs`: the step timeout (cited)
- `crates/takt-cli/src/main.rs`: the scenario verb's exit codes (cited)
- `crates/takt/src/case.rs`: `check_id` with `git check-ref-format --branch` (cited)
- `crates/takt/tests/`: tests for the three (inferred)

## Acceptance

A harness step is killed when the `--hours` budget ends, `takt scenario run --help` no longer lists `frontier: nothing ready`, and a case id that `git check-ref-format --branch takt/<id>` rejects is refused by `check_id` with exit 1 before any case is opened.
