---
format: aep.planning-md/3
id: story:image-harness-0-20-0
kind: story
status: implemented
title: The image runs b10x-harness 0.20.0, admitted by the metaharness takt reaches
summary: harness 0.20.0 sends x-opencode-session; needs a loom release on metaharness 0.19.0 (trial runs 3, 4)
tags:
- loom-agent
relations:
- serves: vision:safe-agentic-coding
- decomposes: epic:takt-as-a-loom-agent
scope:
- confidence: cited
  path: Cargo.lock
- confidence: cited
  path: Cargo.toml
- confidence: cited
  path: Dockerfile
- confidence: cited
  path: fuzz/Cargo.lock
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-10-26T16:16:30Z", actor: "human:timo", revision: 6}
- {from: "proposed", to: "active", at: "2026-10-26T16:16:30Z", actor: "human:timo", revision: 7}
- {from: "active", to: "implemented", at: "2026-10-26T16:16:36Z", actor: "human:timo", revision: 8, decided_on: {"recorded":{"test_result":1,"review_outcome":1}}}
---
# Story: image-harness-0-20-0

## Outcome

The image's `b10x-harness` is 0.20.0, whose llm 0.15.0 sends the `x-opencode-session` header the
OpenCode Go endpoints require, and the metaharness takt reaches through loom-sdk admits it.

## Context

- Trial run 3: "the session ended without the JSON line naming its action"; a probe showed the
  endpoint answering 400 `MissingSessionID` without `x-opencode-session`. Run 7 passed only through
  a loopback relay that added the header.
- Trial run 4 (harness 0.20.0 in the image): refused, "H9 Gap: the vendor is 0.20.0 and the
  adapter is pinned to ["0.18.0"]". metaharness 0.19.0 pins `b10x-harness` 0.20.0
  (`metaharness_b10x::PINNED_VERSIONS`); takt reaches metaharness only through `b10x-loom-sdk`,
  and loom 0.42.0 locks metaharness 0.15.0. Cargo refuses a `[patch]` that points at the same
  git source.

## Acceptance

1. takt pins a loom release whose lock holds metaharness 0.19.0 or later admitting harness 0.20.0.
2. `Dockerfile` installs `b10x-harness` 0.20.0, verified by SHA256.
3. A b10x-harness session through the pinned loom-sdk is not refused on the H9 pin.

## Out of Scope

- The loom release itself.

## Scope

- `Cargo.toml`, `Cargo.lock`, `fuzz/Cargo.lock`, `Dockerfile`.
