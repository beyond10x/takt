---
format: aep.planning-md/3
id: story:promotion-builds-on-a-mac-with-a-user-cargo-config
kind: story
status: draft
title: The macOS promotion build runs on a Mac that has a ~/.cargo/config.toml
summary: 'follow-up of the Substrate confinement hop: the build refuses fail-closed when a .cargo/config.toml sits above it'
tags:
- adr-0109
relations:
- decomposes: epic:takt-on-loom-sdk
revision: 2
---
## Outcome

takt's macOS promotion build runs on a Mac whose operator has `~/.cargo/config.toml` (or any
`.cargo/config.toml` above the build directory), with that file kept out of the confined build,
instead of stopping fail-closed.

## Why

story:confinement-through-substrate-on-linux-and-macos (held on `wave/substrate-confinement`) moves the macOS promotion build onto
Substrate's `macos` adapter. Cargo reads every `.cargo/config.toml` from the build directory up to
`/`, and the adapter cannot hide a file above the workspace, so takt refuses the build when one
exists (a macOS test on that branch pins the refusal). Most developer Macs have `~/.cargo/config.toml`, so the
promotion build is unusable there.

## Acceptance

1. With a `.cargo/config.toml` above the build directory, the macOS promotion build runs and the
   file's settings do not reach it (for example through `CARGO_HOME` pointed at a private
   directory and a build directory outside the home, or a Substrate aperture that hides it).
2. The fail-closed test becomes the test of that behaviour; its rewrite names this story.
3. `task check` passes on Linux and the darwin workflow passes.
