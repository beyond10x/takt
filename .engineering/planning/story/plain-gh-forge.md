---
format: aep.planning-md/3
id: story:plain-gh-forge
kind: story
status: draft
title: A gh forge for repositories outside the organization
summary: --forge gh pushes, opens, reads checks and merges under a grant through the user's gh login; organization repositories keep the bot route
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
scope:
- confidence: inferred
  path: README.md
- confidence: inferred
  path: crates/takt-cli/src/main.rs
- confidence: inferred
  path: crates/takt/src/forge.rs
- confidence: inferred
  path: crates/takt/tests/gh_forge.rs
revision: 3
---
# Story: plain-gh-forge

## Outcome

A repository outside the organization runs story delivery with a forge over the user's own `gh` login: push, open the pull request, read its checks and merge under a recorded grant, with the pull-request text scanned before it leaves the machine. The organization's repositories keep the bot route.

## Context

Owner decision of 2026-10-18 (product-requirements:takt horizon, within 3 months). Today the forge is the bot route or none: `forge_of` in `crates/takt-cli/src/main.rs` builds `BotRoute::for_tree` for a github.com `origin` and nothing else (README § Requirements: "for pull requests, an `origin` on github.com and `b10x-gates` and `gh` on the `PATH`"). An outside user has no organization bot App, so R14 (outside users) cannot be met for the merge path.

The trade-off the owner accepted: outside repositories skip the bot route's checks (signed commits through the App, the organization's private-rule scan). The `Forge` trait (`crates/takt/src/forge.rs`) is the seam; this story adds one adapter and a `--forge gh` choice.

## Expected before

On takt at 7b5aca3, holds; after the fix, fails:

- `takt run --forge gh …` is refused as an unknown forge (`takt run --help` lists only `bot` and `none`).

## Scope

- `crates/takt/src/forge.rs`: a `GhForge` adapter (inferred)
- `crates/takt-cli/src/main.rs`: `forge_of` and the `--forge` values (inferred)
- `README.md`: the requirements line (inferred)
- `crates/takt/tests/`: a forge test with a stub `gh` (inferred)

## Acceptance

With `--forge gh` and a stub `gh` on the `PATH`, a story run pushes the case branch, opens the pull request, reads its checks and, under a recorded grant of `repository.merge` at the case's revision, merges it, each through `gh`; the run refuses `--forge gh` for a repository the organization's policy lists, so the bot route stays the only route there.
