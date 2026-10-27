---
format: aep.planning-md/3
id: story:a-toolchain-linked-into-the-tree-is-a-step-change
kind: story
status: draft
title: A rustup toolchain linked into the tree is a step change
tags:
- finding
relations:
- decomposes: epic:composable-agentic-work
- informed_by: review-result:adversary-u3-pass-1
revision: 1
---
## Outcome

A step that links a rustup toolchain into the tree, or changes rustup's `settings.toml`, is a
change outside the tree and stops the run.

## Context

- review-result:adversary-u3-pass-1, F4: `$RUSTUP_HOME/toolchains/evil` linked to `target/evil`
  with a `cargo` in it, plus `settings.toml` with `default_toolchain = "evil"`, is accepted,
  because a toolchain `bin` whose real path is inside the tree is skipped and `settings.toml` is
  not watched. Present before wave 38. `RUSTUP_HOME` reaches the test command; that a rustup proxy
  would then run the planted program is inferred, not run.
- Reproduction: `cargo test -p b10x-takt --test fresh_home_outside_adv1` passes while the
  defect is present; its rustup case names this story.

## Acceptance

1. The rustup case of `crates/takt/tests/fresh_home_outside_adv1.rs` asserts a stopped run.
