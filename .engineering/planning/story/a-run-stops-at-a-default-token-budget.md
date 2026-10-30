---
format: aep.planning-md/3
id: story:a-run-stops-at-a-default-token-budget
kind: story
status: active
title: A run stops at 3,000,000 tokens unless it names another budget
summary: an unattended run in a pod has a token bound even when nobody set one
tags:
- loom-agent
relations:
- decomposes: epic:takt-as-a-loom-agent
- serves: vision:safe-agentic-coding
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-10-29T21:22:42Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-29T21:22:42Z", actor: "human:timo", revision: 3}
---
# Story: a-run-stops-at-a-default-token-budget

## Outcome

A `takt run` given no `--tokens` and no `tokens` in its run file stops once the harness has
reported 3,000,000 tokens, with the stop `Tokens (3000000)`, exit 3, and the `stop` line of the
case's `events.jsonl` naming it. A run that names a budget keeps its own; `takt run --help`
states the default.

## Context

An unattended run in a cluster pod has nobody to stop it. The one-hour pod trial of 2026-10-26
spent 17,200,642 tokens over 12 runs and accepted nothing; its runs 7 and 8 alone spent
5,452,774 and 8,339,887 tokens (trial record, 2026-10-26). `--tokens` exists
(`crates/takt-cli/src/main.rs`, `LoopArgs::tokens`) and is checked before every step
(`crates/takt/src/run.rs`, `budget_spent`), but it is off unless given. A default makes the bound
hold for every run, including one whose operator forgot it.

The budget counts what one invocation reported, as `--tokens` does (R10); a step under way runs
to its end, so a run can pass the budget by one step's tokens. The budget is the run's, not the
case's: a resumed case starts its count again.

## Scope

- `crates/takt-cli/src/main.rs`: `LoopArgs::tokens` (`:1075-1077`), its default of 3,000,000 when neither the flag nor the run file gives one, and `--help` stating it (cited)
- `crates/takt/src/run_config.rs`: a `tokens` key, a whole number above zero, and `SHAPE` (`:788`) naming it; the run file has no such key today (cited)
- `crates/takt/src/run.rs`: `budget_spent` (`:10488-10504`) unchanged; the default reaches it through `options.tokens` (cited)
- `crates/takt-cli/tests/`: a test of the default and of the run file's key (inferred)
- `website/docs/reference/configuration.md`: the `tokens` row of the run file table (inferred)

## Acceptance

With a harness double that reports 1,000,000 tokens on every step and never makes the fixture's
test pass, `takt run --repo <fixture> --harness <double> --forge none`:

1. with no `--tokens` and no run file: exits 3 after the step that brings the count to 3,000,000 or
   more; stdout holds `stopped: Tokens (3000000)`; the case's `events.jsonl` has a `stop` line whose
   `stop` is `Tokens (3000000)`.
2. with `--tokens 5000000`: stops `Tokens (5000000)`.
3. with `--config <file>` whose run file holds `tokens: 2000000`: stops `Tokens (2000000)`.
4. with that file and `--tokens 5000000`: stops `Tokens (5000000)`.
5. with a run file holding `tokens: 0` or `tokens: x`: exits 2 before any case opens, naming the
   file and the line.
6. `takt run --help` prints `3000000` in the `--tokens` entry.

This story shares `crates/takt-cli/src/main.rs` with story:forge-preflight (`LoopArgs::tokens`
against `forge_of`) and `website/docs/reference/configuration.md` with
story:bot-forge-reads-through-the-app (the `tokens` row against the bot row): distinct hunks,
accepted.
