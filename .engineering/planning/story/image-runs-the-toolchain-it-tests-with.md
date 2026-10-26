---
format: aep.planning-md/3
id: story:image-runs-the-toolchain-it-tests-with
kind: story
status: implemented
title: The image tests a repository whose dependencies build against OpenSSL
summary: pkg-config and libssl-dev in the final stage (trial run 1)
tags:
- loom-agent
relations:
- serves: vision:safe-agentic-coding
- decomposes: epic:takt-as-a-loom-agent
scope:
- confidence: cited
  path: Dockerfile
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-10-26T13:48:03Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-26T13:48:03Z", actor: "human:timo", revision: 4}
- {from: "active", to: "implemented", at: "2026-10-26T16:16:36Z", actor: "human:timo", revision: 5, decided_on: {"recorded":{"test_result":1,"review_outcome":2}}}
---
# Story: image-runs-the-toolchain-it-tests-with

## Outcome

A run inside takt's image tests a Rust repository whose dependencies build native code against
OpenSSL: `cargo test` in the container finds `pkg-config` and the OpenSSL headers.

## Context

- Trial run 1 (takt 0.12.0 image, `--iterations 1`): exit 3, step 1 `tests.run` fail, the
  `test_result` evidence fail from `takt.tests-run`: `openssl-sys` needs `pkg-config` and
  `libssl-dev`, which only the build stage installed.
- An image with both added (the trial's first derived image) passed `tests.run` in run 7
  (117, 117, 118 passed, 0 failed).

## Acceptance

1. The final stage of `Dockerfile` installs `pkg-config` and `libssl-dev`.
2. In the built image, as uid 10001, `pkg-config --modversion openssl` exits 0.

## Out of Scope

- Other system libraries a target repository may need.

## Scope

- `Dockerfile` (final stage).
