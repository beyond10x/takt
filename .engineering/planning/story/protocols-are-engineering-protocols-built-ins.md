---
format: aep.planning-md/3
id: story:protocols-are-engineering-protocols-built-ins
kind: story
status: implemented
title: takt reads its three protocols from the engineering protocols' registry
summary: engineering-protocols 0.10.0 ships story.delivery/1, self.improvement/1, loop.improvement/1; takt drops its copies
tags:
- loom-agent
relations:
- serves: vision:safe-agentic-coding
scope:
- confidence: cited
  path: Cargo.lock
- confidence: cited
  path: Cargo.toml
- confidence: cited
  path: crates/takt/src/protocol.rs
- confidence: cited
  path: ess/domains/takt.yaml
- confidence: cited
  path: fuzz/Cargo.lock
- confidence: cited
  path: protocols
revision: 10
transitions:
- {from: "draft", to: "proposed", at: "2026-10-26T13:59:35Z", actor: "human:timo", revision: 8}
- {from: "proposed", to: "active", at: "2026-10-26T13:59:35Z", actor: "human:timo", revision: 9}
- {from: "active", to: "implemented", at: "2026-10-26T16:16:37Z", actor: "human:timo", revision: 10, decided_on: {"recorded":{"test_result":1,"review_outcome":2}}}
---
# Story: protocols-are-engineering-protocols-built-ins

## Outcome

takt reads `story.delivery/1`, `self.improvement/1` and `loop.improvement/1` from the engineering
protocols' registry, as it reads `software.change/1`, and carries no copy of its own.

## Context

- engineering-protocols 0.10.0 ships the three protocols as built-ins (registry ids
  `story-delivery@1`, `self-improvement@1`, `loop-improvement@1`), byte-identical to takt 0.12.0's
  `protocols/<name>/1.yaml` (`cmp` against the tag).
- takt 0.12.0 embeds two of them (`EMBEDDED` in `crates/takt/src/protocol.rs`) and its tests read
  all three from `protocols/` with `include_str!`.

## Acceptance

1. `b10x-canon-engineering` and `b10x-canon-engineering-assertions` are pinned at
   engineering-protocols tag 0.10.0.
2. `EMBEDDED` is gone; `LoadedProtocol::builtin("story.delivery/1")` and
   `builtin("story-delivery/1")` return the registry's document, and likewise for the other two.
3. `protocols/story-delivery/1.yaml`, `protocols/self-improvement/1.yaml` and
   `protocols/loop-improvement/1.yaml` are deleted; `protocols/loop-improvement/metrics.yaml` stays.
4. No test or build step reads the deleted files; ESS citations of them are external citations of
   engineering-protocols 0.10.0.

## Scope

- `Cargo.toml`, `Cargo.lock`, `fuzz/Cargo.lock`, `crates/takt/src/protocol.rs`, the tests that
  read the protocol files, `protocols/`, `ess/domains/takt.yaml`, `README.md`, `AGENTS.md`.
