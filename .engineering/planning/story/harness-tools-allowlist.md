---
format: aep.planning-md/3
id: story:harness-tools-allowlist
kind: story
status: draft
title: The Claude adapter names its tools with --tools, not only --disallowedTools
relations:
- decomposes: epic:composable-agentic-work
revision: 1
---
## Outcome

The Claude adapter passes the allowed tool set with `--tools`, so each process start loads only those tools' definitions.

## Context

docs/runs/2026-10-18-efficiency-analysis.md (inferred there, not measured in isolation): takt's first-iteration tokens are 13,362 against the isolated arm's 7,448 per process start, and the adapter passes `--disallowedTools` without `--tools` (`crates/takt/src/harness.rs` at a0b8576, the Claude adapter's arguments). About 5.9k tokens per process start, three starts per run.

## Scope

- `crates/takt/src/harness.rs`: the Claude adapter's arguments (cited)

## Acceptance

With the change, the first iteration of a takt step on scenario 1 costs at most the isolated arm's 7,448 tokens plus 10 %, and Bash stays unavailable to the harness.
