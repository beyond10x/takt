---
format: aep.planning-md/3
id: story:forge-preflight
kind: story
status: active
title: The bot forge is checked before the case opens
summary: b10x-gates, gh and B10X_GATES_POLICY are checked in forge_of; a missing one is exit 1 before any step
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
scope:
- confidence: cited
  path: crates/takt-cli/src/main.rs
- confidence: cited
  path: crates/takt/src/forge.rs
- confidence: inferred
  path: crates/takt/tests/forge_preflight.rs
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-10-29T21:22:42Z", actor: "human:timo", revision: 5}
- {from: "proposed", to: "active", at: "2026-10-29T21:22:43Z", actor: "human:timo", revision: 6}
---
# Story: forge-preflight

## Outcome

A run that will need the bot forge checks it can reach it before the case opens: `b10x-gates` and `gh` on the path and `B10X_GATES_POLICY` set. A missing one is refused with exit 1 before any step, not found at `pr.open` after the budget is spent.

## Context

Found by an independent code review of 2026-10-18, takt at 2a22093 (line numbers unchanged at 2ec88d4).

- `forge_of` (`crates/takt-cli/src/main.rs:606-649`) and `BotRoute::for_tree` (`crates/takt/src/forge.rs:642-664`) check the origin remote only; the policy is read as optional (`forge.rs:658`), and neither looks for `b10x-gates` or `gh`.
- The first call through them is `pr.open`, after tests, the gate and model steps; a missing tool or policy ends the run there with exit 5.
- The operator's trial run 3 hit exactly this: exit 5 at step 8, 46,242 tokens spent.

## Expected before

On takt at 2ec88d4, holds; after the fix, fails:

- `takt run --forge bot --harness none` on a repository whose origin is on github.com, with `PATH` holding no `b10x-gates` and `B10X_GATES_POLICY` unset, opens a case (its output contains `case ` and `forge: bot`) and does not exit 1.

## Scope

- `crates/takt-cli/src/main.rs`: `forge_of`, the preflight (cited)
- `crates/takt/src/forge.rs`: `BotRoute::for_tree` and a check of its tools and policy (cited)
- `crates/takt/tests/`: a preflight test (inferred)

## Acceptance

With the bot forge selected and any of `b10x-gates`, `gh` or `B10X_GATES_POLICY` missing, `takt run` exits 1 before any case is opened, naming what is missing.
