---
format: aep.planning-md/3
id: story:confinement-through-substrate-on-linux-and-macos
kind: story
status: draft
title: takt confines through Substrate on Linux and macOS; its own Seatbelt, unshare and bwrap code is retired
summary: 'ADR 0109 hop T1: Substrate''s confinement contract is takt''s only confinement'
tags:
- adr-0109
relations:
- decomposes: epic:takt-on-loom-sdk
- serves: vision:safe-agentic-coding
revision: 1
---
# Story: confinement-through-substrate-on-linux-and-macos

## Outcome

takt confines every child it confines today (the test command, the promotion's build and its
binary's `--version`, the test command's login hiding) through Substrate's confinement contract
only, on Linux and on macOS, and its own mechanisms are gone: no `sandbox-exec`, `bwrap` or
`unshare` is left in takt's sources.

## Context

- ADR 0109 (accepted 2026-10-27), decision 6: "loom-sdk and takt consume Substrate only and delete
  their own confinement code: takt's `seatbelt.rs`, its `unshare` path and its promotion bwrap".
  Its end-to-end check: "a takt run with an accepted result on Linux and on macOS (takt's
  `darwin.yml`) through Substrate, with no `sandbox-exec`, `bwrap` or `unshare` left in takt's
  sources". This story is the ADR's takt hop T1.
- What takt carries today: the seam with `none` and `substrate` (`crates/takt/src/confinement.rs:296`,
  `:379`), the Substrate adapter on loom-sdk's `SubstrateRunner` that needs a delegated cgroup and
  starts itself again under `systemd-run` (`confinement.rs:384-398`, `:511`); its own Seatbelt
  profiles for the promotion build and the test command (`crates/takt/src/seatbelt.rs:1-18`,
  `SANDBOX_EXEC` at `:25`); the `unshare` login-hiding path of the test command
  (`crates/takt/src/forge.rs:829`); the promotion's bubblewrap build (`crates/takt/src/promotion.rs:101`,
  `:1503-1554`, `:1642`).
- Substrate's consumer surface for loom-sdk and takt is substrate's `story:confinement-consumer-surface`
  (epic `confinement-contract-family`), which covers takt's promotion build ("an install root outside
  the clone writable through a declared writable root, a `--version` run of the result") and picks
  an adapter by its served families and facts, never by name or OS. ADR 0109: "a profile that needs
  a fact the adapter lacks is refused with the fact's name; nothing degrades silently".
- Behaviour change the ADR names: takt's macOS profile is `(allow default)` with the network open;
  Substrate's is `(deny default)`. A repository whose tests need the network declares routes, or
  runs `local`, which is requested by name with a reason and reported as declared insecure.
- `story:harness-child-confined` (the harness session, git, the forge and `aep` under a profile) is
  a separate story; this one moves what takt confines today and adds no confined child.

## Decision

- Adapters are chosen from Substrate's served families and facts; takt names no OS and no
  mechanism. `--confinement none` stays and is Substrate's `local`, named with its reason in the
  `opened` event and the first line.
- The pin moves to the substrate release that ships `story:confinement-consumer-surface`
  (dependency-blocker:substrate-confinement-consumer-surface); takt builds no substitute before it.

## Acceptance

Each item is a conformance scenario named in `ess/ess-inputs.yaml`, run by `task check` (Linux)
and by `darwin.yml` (macOS).

1. `confinement/test-command-confined-linux`: on Linux, a run with `--confinement substrate` runs
   the test command through Substrate's `linux` adapter; a test command that writes outside the
   tree and its `target/`, or connects to a host, is refused, and the run records the adapter's
   name and facts on its `opened` event.
2. `confinement/test-command-confined-macos`: on macOS, the same run goes through Substrate's
   `macos` adapter with the same refusals; a profile needing a family `macos` does not serve
   (limits) is refused naming that family, never run unconfined.
3. `confinement/promotion-build-confined`: `takt promote` builds the candidate and runs its
   `--version` through Substrate on both platforms, the install root outside the clone writable
   through a declared writable root and nothing else written.
4. `confinement/unserved-fact-refused`: a profile that needs a fact the chosen adapter lacks ends
   the run before any child starts, exit 1, naming the fact; no child runs unconfined.
5. `confinement/local-is-declared-insecure`: `--confinement none` runs on Substrate's `local`, and
   the `opened` event and the first line say so with the reason.
6. `confinement/no-own-mechanism-left`: `grep -rnE 'sandbox-exec|"bwrap"|libc::unshare' crates/`
   prints nothing; `crates/takt/src/seatbelt.rs` does not exist.

## Scope

- `crates/takt/src/confinement.rs` (the seam on Substrate's contract), `crates/takt/src/seatbelt.rs`
  (removed), `crates/takt/src/forge.rs` (the `unshare` path), `crates/takt/src/promotion.rs` (the
  bubblewrap and Seatbelt build), `Cargo.toml` (the substrate pin), `.github/workflows/darwin.yml`,
  `ess/domains/takt.yaml`, `ess/ess-inputs.yaml`, `crates/takt/tests/confinement.rs`. Inferred from
  the paths above; not yet confirmed by an implementor.

## Blocked by

- dependency-blocker:substrate-confinement-consumer-surface.

## Out of Scope

Confining the harness session, git, the forge and `aep` (`story:harness-child-confined`); ingress
routes and rate bounds (ADR 0109 F1 B).
