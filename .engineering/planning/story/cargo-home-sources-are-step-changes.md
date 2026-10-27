---
format: aep.planning-md/3
id: story:cargo-home-sources-are-step-changes
kind: story
status: draft
title: Writes into the sources cargo compiles from CARGO_HOME are step changes
tags:
- finding
relations:
- decomposes: epic:composable-agentic-work
- informed_by: review-result:adversary-u3-pass-1
revision: 1
---
## Outcome

A step that writes into the sources cargo compiles from `$CARGO_HOME` (`registry/src`,
`git/checkouts`) is a change outside the tree and stops the run, in a fresh `HOME` and in an
existing one.

## Context

- review-result:adversary-u3-pass-1, F1 and F2: a `tests.run` that plants
  `~/.cargo/registry/src/.../serde-1.0.228/{.cargo-ok,build.rs}` is accepted with a pass. In an
  existing `~/.cargo` this was so before wave 38 (`Outside::paths` never covered those sources);
  wave 38's fresh-HOME change (story:fresh-home-first-test-run-is-not-refused) made a fresh
  `HOME` behave the same. The tests run with the caller's `$CARGO_HOME`, so the planted
  `build.rs` would run in later builds inside and outside takt (cargo skips unpacking a crate
  whose `.cargo-ok` reads `{"v":1}`; from cargo's behaviour, not run here).
- Reproduction: `cargo test -p b10x-takt --test fresh_home_outside_adv1` passes while the
  defect is present; its two registry cases name this story.

## Acceptance

1. The two registry cases of `crates/takt/tests/fresh_home_outside_adv1.rs` assert a stopped run.
2. A fresh `HOME`'s first `tests.run` that only fills cargo's cache (index, `.crate` archives,
   unpacked sources cargo itself wrote) is still not refused.
