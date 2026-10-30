---
format: aep.planning-md/3
id: story:forge-preflight
kind: story
status: implemented
title: The bot forge is checked before the case opens
summary: b10x-gates, gh and B10X_GATES_POLICY are checked in forge_of; a missing one is exit 1 before any step
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
- depends_on: story:bot-forge-reads-through-the-app
scope:
- confidence: cited
  path: crates/takt-cli/src/main.rs
- confidence: cited
  path: crates/takt/src/forge.rs
- confidence: inferred
  path: crates/takt/tests/forge_preflight.rs
revision: 9
transitions:
- {from: "draft", to: "proposed", at: "2026-10-29T21:22:42Z", actor: "human:timo", revision: 5}
- {from: "proposed", to: "active", at: "2026-10-29T21:22:43Z", actor: "human:timo", revision: 6}
- {from: "active", to: "implemented", at: "2026-10-30T00:45:24Z", actor: "human:timo", revision: 9, decided_on: {"recorded":{"test_result":1,"review_outcome":1}}}
---
# Story: forge-preflight

## Outcome

A run that will need the bot forge checks it can reach it before the case opens: `b10x-gates` on
the path and `B10X_GATES_POLICY` naming a file. A missing one is refused with exit 1 before any
step, naming what is missing, not found at `pr.open` after the budget is spent. `gh` is not
checked: after story:bot-forge-reads-through-the-app the bot forge runs no `gh`.

## Context

Found by an independent code review of 2026-10-18, takt at 2a22093.

- `forge_of` (`crates/takt-cli/src/main.rs:3195`) and `BotRoute::for_tree`
  (`crates/takt/src/forge/bot.rs:130`) check the origin remote only; the policy is read as
  optional, and neither looks for `b10x-gates`.
- The first call through them is `pr.open`, after tests, the gate and model steps; a missing tool
  or policy ends the run there with exit 5.
- The operator's trial run 3 hit this: exit 5 at step 8, 46,242 tokens spent.
- A cluster pod runs unattended (`examples/cluster-delivery/README.md`): a run that cannot reach its
  forge spends its token budget for nothing.

## Expected before

On takt at 421f4303, holds; after the fix, fails:

- `takt run --forge bot --harness none` on a repository whose origin is on github.com, with `PATH`
  holding no `b10x-gates` and `B10X_GATES_POLICY` unset, opens a case (its output contains
  `case ` and `forge: bot`) and does not exit 1.

## Scope

- `crates/takt-cli/src/main.rs`: `forge_of`, the preflight (cited, `:3195`)
- `crates/takt/src/forge/bot.rs`: `BotRoute::for_tree` and a check of its program and policy (cited, `:130`)
- `crates/takt-cli/tests/forge_preflight.rs`: new (inferred)

## Acceptance

With the bot forge selected:

1. `b10x-gates` absent from `PATH`: `takt run` exits 1, no directory is created under the cases
   directory, stderr names `b10x-gates`.
2. `B10X_GATES_POLICY` unset, or naming no file: the same, stderr naming `B10X_GATES_POLICY`.
3. both present and no `gh` on `PATH`: a case directory is created and the run's output holds
   `forge: bot` (the run gets past `forge_of`).
