---
format: aep.planning-md/3
id: story:a-program-reinstalled-at-its-name-during-a-test-step-refuses-it
kind: story
status: draft
title: A program reinstalled at its name during a test step refuses the step again
summary: private program directories are links the guard now follows, so a reinstall at an existing name by another process reaches a test step
tags:
- from-finding
relations:
- serves: vision:safe-agentic-coding
revision: 3
---
# A program reinstalled at its name during a test step refuses the step again

## What is wrong

Found while implementing wave 53's unit c (story:program-rewritten-through-a-link-refuses-the-step).
The step guard now reads a link in a program directory with what it resolves to (`Entry::Through`,
`Outside::program_at` in `crates/takt/src/run.rs`). A test binary's private program directories
(`crates/takt/tests/support/own_home.rs`) are links to the real programs, so another process that
reinstalls a program at an existing name in the real home while a test step runs (a `cargo install`
of `ess`, say) changes what such a link resolves to, and the step is refused.

That partly reverses acceptance 1 of story:an-unrelated-program-install-refuses-no-step (the wave
39 flake): a write to the real home's program directories reaches a test again, though only a
reinstall at an existing name, not an install at a new one. Copies instead of links were weighed
and left out: several gigabytes per test binary, and a copied `rustc` finds no sysroot beside it.

## Check

No case reaches it yet. The first task is a case that, while a test step runs under the guard,
replaces the program an existing link in the private program directory resolves to from another
process, and shows the step refused.

## Outcome

A takt test's result does not depend on another process reinstalling a program at an existing name
in the real home, while the guard still refuses a step that rewrites the program behind a link.

## Scope

- `crates/takt/tests/support/own_home.rs`: the private program directories (inferred)
- `crates/takt/src/run.rs`: `Outside::program_at` (inferred)
