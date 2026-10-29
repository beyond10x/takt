---
format: aep.planning-md/3
id: story:test-rustup-home-is-not-isolated
kind: story
status: draft
title: The tests' step guard still reads the operator's rustup toolchains
tags:
- finding
- priority:low
- rsi
refs:
- provider: finding
  reference: test-rustup-home-is-not-isolated
relations:
- serves: vision:safe-agentic-coding
- informed_by: story:an-unrelated-program-install-refuses-no-step
revision: 1
---
# The tests' step guard still reads the operator's rustup toolchains

## What is wrong

The step guard refuses a step when a program directory it lists changed since the run began
(`program_dirs`, `crates/takt/src/run.rs`): the directories on `PATH`, `$CARGO_HOME/bin`, and the
`bin` of every toolchain under `$RUSTUP_HOME/toolchains`. Wave 42 U3 gave each test binary private
copies of the first two (`crates/takt/tests/support/own_home.rs`); the rustup toolchains stay the
operator's. A `rustup update` or a toolchain install by another process while the suite runs can
still refuse a test's step, the defect U3 fixed for `PATH` and `~/.cargo/bin`.

U8 tried a linked `RUSTUP_HOME` and dropped it: it broke 3 confinement tests, because the confined
test command runs the toolchain bound read-only by its canonical path, and a toolchain made of
links to the real one is not whole inside that bind (`own_state.rs`, module comment). U3's commit
names the same gap.

## Check

A source check: `own_home.rs` does not touch `RUSTUP_HOME`.

```expected_before
command: grep -q 'RUSTUP_HOME' crates/takt/tests/support/own_home.rs
exit: 1
healthy_exit: 0
source: crates/takt/tests/support/own_state.rs
```

## Outcome

The guard's toolchain directories in a test binary are its own, or the guard is told, for a test
binary only, to watch toolchain directories that no other process changes, while the confined test
command still finds a whole toolchain by its canonical path.

## Acceptance

1. The predicate above exits 0, or the healthy source shape is restated in the story when the
   fix takes another route.
2. A test writes a file into a stand-in toolchain `bin` during a run and the run's steps are not
   refused, with the confinement tests still passing.

## Scope

- `crates/takt/tests/support/own_home.rs`: `isolate` (cited)
- `crates/takt/tests/support/own_state.rs`: the module comment on rustup (cited)
- `crates/takt/src/run.rs`: `program_dirs`, if the guard is told its directories (inferred)
- `crates/takt/tests/tests_have_a_home_of_their_own.rs` (inferred)
