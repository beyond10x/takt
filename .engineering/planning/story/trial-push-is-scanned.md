---
format: aep.planning-md/3
id: story:trial-push-is-scanned
kind: story
status: draft
title: The trial-branch push is scanned before it leaves, as pr.open's is
relations:
- decomposes: epic:composable-agentic-work
- informed_by: story:scan-before-push
revision: 1
---
## Outcome

The trial-branch push in `takt-cli` (`TrialRoute`, `push_argv`) scans what it publishes before anything leaves, under the same policy and with the same route as `pr.open` since story:scan-before-push: the push is refused when a forbidden literal is in any commit, message, name or blob it would send.

## Context

From the developer's adversary passes on story:scan-before-push (2026-10-19). `pr.open` now scans whole files and pushes to the forge's URL instead of a remote by name. The trial-branch push in `crates/takt-cli/src/main.rs` (`TrialRoute` at `crates/takt-cli/src/main.rs:581`, `push_argv` at `:626`, called at `:671`) is not scanned and still pushes to the remote by name, so a trial run can publish what `pr.open` would refuse.

## Scope

- `crates/takt-cli/src/main.rs`: `TrialRoute`, `push_argv` (cited)
- `crates/takt/src/forge.rs`: the scan `pr.open` uses (inferred)

## Acceptance

A trial run whose branch holds a planted forbidden literal in a commit message is refused before the push, naming the literal's rule, and the bare remote used in the test receives no ref; a clean trial branch still pushes.
