---
format: aep.planning-md/3
id: story:gate-exec-finds-the-toolchain-in-the-image
kind: story
status: active
title: gate-exec finds the Rust toolchain on its system path inside the image
summary: cargo, rustc, rustfmt, clippy in /usr/local/bin (trial run 2)
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
# Story: gate-exec-finds-the-toolchain-in-the-image

## Outcome

A gate run inside takt's image finds `cargo`, `rustc`, `rustfmt` and `cargo-clippy` on the system
path `scripts/gate-exec` searches (`/usr/local/bin`, `/usr/bin`, `/bin`, then `$CARGO_HOME/bin`),
whatever `CARGO_HOME` the run is given.

## Context

- Trial run 2: exit 5, step 2 `gate.run` fail, cargo not found on gate-exec's path: the rust base
  image keeps the toolchain under `/usr/local/cargo/bin` and `/usr/local/rustup`, and the run's
  `CARGO_HOME` was a writable directory with no `bin`.
- The trial's second derived image linked the toolchain's programs into `/usr/local/bin`; gate.run
  then ran (run 7, steps 3 and 6 reached the `citations` gate).

## Acceptance

1. In the built image, as uid 10001 with `PATH=/usr/local/bin:/usr/bin:/bin` and `CARGO_HOME`
   unset or set to an empty writable directory, `cargo --version`, `rustc --version`,
   `cargo fmt --version` and `cargo clippy --version` exit 0.

## Scope

- `Dockerfile` (final stage).
