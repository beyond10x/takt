---
format: aep.planning-md/3
id: story:no-repush-after-head-lag
kind: story
status: draft
title: pr.open does not push again when the forge's head lags
relations:
- decomposes: epic:composable-agentic-work
revision: 1
---
## Outcome

When the forge reports a pull request head that lags the pushed revision, takt waits for the forge to catch up instead of pushing the same revision again, and each read uses one forge call.

## Context

docs/runs/2026-10-18-efficiency-analysis.md: after a head lag `pr.open` pushes again (`crates/takt/src/validator.rs` at a0b8576 :782 calling `crates/takt/src/forge.rs` :1042), and each read does a view and a checks call; about 8 s per run on scenario 1. No story covers it.

## Scope

- `crates/takt/src/validator.rs`: `PrOpen` (cited)
- `crates/takt/src/forge.rs`: push and view (cited)

## Acceptance

On scenario 1 against the real forge, each run pushes the case branch once per revision, and the forge calls per run (pushes, views, checks) drop from the analysis's 13 to at most 6.
