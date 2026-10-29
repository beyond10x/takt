---
format: aep.planning-md/3
id: story:test-home-is-the-operators-home
kind: story
status: draft
title: A test binary with program directories of its own still runs in the operator's home
tags:
- finding
- priority:low
- rsi
refs:
- provider: finding
  reference: test-home-is-the-operators-home
relations:
- serves: vision:safe-agentic-coding
- informed_by: story:an-unrelated-program-install-refuses-no-step
revision: 1
---
# A test binary with program directories of its own still runs in the operator's home

## What is wrong

`crates/takt/tests/support/own_home.rs` gives every test binary that runs steps private program
directories: each writable directory on `PATH` becomes a directory of links to its programs
(`copy_programs`, `own_home.rs:91`), and `CARGO_HOME` a private directory linking the real one's
entries. `HOME` stays the operator's. Every step a test runs, and every git, aep and test command
under it, reads and writes the operator's home: its git configuration, `~/.cache`, `~/.local/state`
(the transcripts directory the takt-cli unit tests create, `crates/takt-cli/src/main.rs`, `fn
takt`). A test can then pass or fail on what the operator's home holds, and leave files in it.

A judgement note from wave 42's adversarial pass, not a seen failure; no test shows a result
that depends on the home yet.

## Check

A source check, not a behaviour check: `own_home.rs` sets no `HOME`. A behaviour test (a test
binary that runs a step and finds no file it wrote under the operator's home) should replace it
when the story is scoped.

```expected_before
command: grep -q 'set_var("HOME"' crates/takt/tests/support/own_home.rs
exit: 1
healthy_exit: 0
source: crates/takt/tests/support/own_home.rs
```

## Outcome

A test binary that includes `own_home.rs` runs with a `HOME` of its own under its private root,
holding what the tests need (a git identity, the aep cache of story:test-aep-cache-is-the-operators-cache),
and nothing of the operator's home is read or written by its steps. A child test binary that
inherits the marker keeps the home its test built.

## Acceptance

1. The predicate above exits 0.
2. A test runs a step under the private home and finds that the step wrote nothing under the
   home the test binary was started with.
3. `tests_have_a_home_of_their_own` asserts the private `HOME` alongside the private `PATH`.

## Scope

- `crates/takt/tests/support/own_home.rs`: `isolate` (cited)
- `crates/takt/tests/support/own_state.rs`: the aep cache derived from `HOME` (cited)
- `crates/takt/tests/tests_have_a_home_of_their_own.rs` (inferred)
