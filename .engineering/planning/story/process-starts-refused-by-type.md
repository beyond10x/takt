---
format: aep.planning-md/3
id: story:process-starts-refused-by-type
kind: story
status: draft
title: A process start outside the confinement seam is refused by clippy, by type
tags:
- loom
relations:
- decomposes: epic:takt-on-loom-sdk
- depends_on: story:confinement-seam
revision: 1
---
## Outcome

A process start outside the confinement seam is refused by the compiler's lint, by type, rather
than found by a text scan.

## Context

The wave 29 review (finding 2) evaded the confinement guard in `crates/takt/tests/confinement.rs`.
Unit 9 of wave 29 rewrote it to read code with comments and literals blanked, follow `use` trees
and scan every `crates/*/src`. A text scan still misses a path a macro assembles, a `Command`
handed back by another API, `libc` re-exported under another name and a `pub use` inside the
seam. clippy's `disallowed_types` and `disallowed_methods` (`clippy.toml`) refuse
`std::process::Command` and the libc spawn functions by resolved path, with `#[allow]` only in
`crates/takt/src/confinement.rs`.

## Scope

- `clippy.toml`, `crates/takt/src/confinement.rs`, the session-executor's test module
  (`harness_parity.rs` keeps three `std::process::Command` starts; it depends on no takt crate).

## Acceptance

1. `clippy.toml` disallows `std::process::Command`, `tokio::process::Command` and the libc
   fork, exec, posix_spawn, system and popen functions; `cargo clippy --workspace --all-targets
   -- -D warnings` passes with the only `#[allow]` in the seam and the session-executor's tests.
2. A file outside the seam that starts a process through a macro-assembled path fails clippy: a
   fixture test.
