---
format: aep.planning-md/3
id: story:cargo-test-runs-no-reviewer-probe
kind: story
status: draft
title: cargo test compiles no reviewer probe
tags:
- finding
refs:
- provider: finding
  reference: cargo-test-runs-no-reviewer-probe
relations:
- decomposes: epic:composable-agentic-work
revision: 1
---
## Outcome

`cargo test` compiles and runs no reviewer probe: each probe that checks a behaviour the code
promises becomes a named test beside the code's own, and the rest are removed.

## Context

`crates/takt/tests/reviewer_probe3.rs` ("Reviewer probes, round 3 (57c6462). Not part of the
change.") and `crates/takt/tests/reviewer_efficiency_probe.rs` were committed in b0c5457d
(2026-10-18) and compile into every `task check`. Found by a read-only review of 2026-10-21 at
0a61e14a.

## Scope

- `crates/takt/tests/reviewer_probe3.rs`, `crates/takt/tests/reviewer_efficiency_probe.rs`, the
  test files their kept cases move to.

## Acceptance

1. No file under `crates/*/tests/` says it is not part of the change; each kept case names the
   story or behaviour it checks; `task check` green.
