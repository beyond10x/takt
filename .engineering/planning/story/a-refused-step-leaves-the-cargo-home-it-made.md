---
format: aep.planning-md/3
id: story:a-refused-step-leaves-the-cargo-home-it-made
kind: story
status: draft
title: A refused step leaves the cargo home's program and cache in place
tags:
- finding
relations:
- decomposes: epic:composable-agentic-work
- informed_by: review-result:adversary-u3-pass-1
revision: 1
---
## Outcome

When takt refuses a step that made `~/.cargo` and wrote a program into `~/.cargo/bin`, it leaves
what the specification says it leaves: no file removed, no directory it did not make removed.

## Context

- review-result:adversary-u3-pass-1, F3: `put_back` runs `remove_dir_all` on all of `~/.cargo`
  (held as a configuration path), deleting the planted program and cargo's cache, against
  `ess/domains/takt.yaml` ("no file removed and no directory it made removed"). Present before
  wave 38.
- Reproduction: `cargo test -p b10x-takt --test fresh_home_outside_adv1` passes while the
  defect is present; its `bin` case names this story.

## Acceptance

1. The `bin` case of `crates/takt/tests/fresh_home_outside_adv1.rs` asserts the program and the
   cache are left and the run stopped.
