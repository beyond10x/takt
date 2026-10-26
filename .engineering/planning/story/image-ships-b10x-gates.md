---
format: aep.planning-md/3
id: story:image-ships-b10x-gates
kind: story
status: active
title: The image ships b10x-gates so a step's edits are scanned
summary: b10x-gates 0.1.23, SHA256-verified (trial run 6)
tags:
- loom-agent
relations:
- serves: vision:safe-agentic-coding
- decomposes: epic:takt-as-a-loom-agent
scope:
- confidence: cited
  path: Dockerfile
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-26T13:48:04Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-26T13:48:04Z", actor: "human:timo", revision: 4}
---
# Story: image-ships-b10x-gates

## Outcome

A step's edits are scanned inside the image: `b10x-gates` is installed at a pinned version,
verified by SHA256, so the scan takt runs on a harness's edit finds it.

## Context

- Trial run 6: exit 3, 147330 tokens, `StepRefused`: "no gates policy is set (B10X_GATES_POLICY):
  not scanned"; the image had no `b10x-gates` either. Run 7, on an image with `b10x-gates` 0.1.23
  and a policy file, scanned every edit and reached 8 steps.

## Acceptance

1. `Dockerfile` downloads `b10x-gates` 0.1.23 (`b10x-gates-linux-x86_64`) with `ADD --checksum`
   from its GitHub Release and installs it as `/usr/local/bin/b10x-gates`.
2. In the built image, as uid 10001, `b10x-gates --version` prints `b10x-gates 0.1.23`.

## Out of Scope

- The policy file: the deployment provides it (`B10X_GATES_POLICY`), never the image.

## Scope

- `Dockerfile` (tools stage and final stage).
