---
format: aep.planning-md/3
id: story:unit-tests-run-steps-over-the-operators-path
kind: story
status: draft
title: The takt binary's unit tests run guarded steps over the operator's program directories
tags:
- finding
- priority:medium
- rsi
refs:
- provider: finding
  reference: unit-tests-run-steps-over-the-operators-path
relations:
- serves: vision:safe-agentic-coding
- informed_by: story:an-unrelated-program-install-refuses-no-step
revision: 1
---
# The takt binary's unit tests run guarded steps over the operator's program directories

## What is wrong

Since wave 42 U3, every integration test binary that runs steps reads program directories of its
own (`crates/takt/tests/support/own_home.rs`), so a program another process writes into
`~/.cargo/bin` while the suite runs no longer refuses a test's step.
`crates/takt/tests/tests_have_a_home_of_their_own.rs` checks this for `crates/*/tests` only. A
`#[cfg(test)]` module under `crates/*/src` is a test binary too, and the `own_home` hook is not in
it: the unit tests of `crates/takt-cli/src/main.rs` run `takt run --harness none --test-cmd …` in
process (`fn takt`, `crates/takt-cli/src/main.rs:4344`), a guarded `tests.run` step over the
operator's `PATH` and `~/.cargo/bin`. They can still be refused by an unrelated install, the
defect U3 fixed for the integration tests.

Seen in wave 42's adversarial pass: `no_unit_test_module_runs_steps_over_the_operators_program_directories`
(`crates/takt/tests/w42_adv_prompt_lines.rs`, ignored as this finding) names
`takt-cli/src/main.rs`.

## Check

Not runnable by `scripts/check-finding`: it runs a library test, not `$TAKT`. Checked on the
wave 42 tree at a2f650bf: exit 101.

```expected_before
command: cargo test -p b10x-takt --test w42_adv_prompt_lines -- --exact no_unit_test_module_runs_steps_over_the_operators_program_directories --include-ignored
exit: 101
healthy_exit: 0
output: unit-test binaries that run steps over the operator's program directories
source: crates/takt/tests/w42_adv_prompt_lines.rs
```

## Outcome

No unit-test module under `crates/*/src` runs a guarded step over the operator's program
directories: the step-running unit tests of `crates/takt-cli/src/main.rs` move to an integration
test binary that includes `support/own_home.rs`, or the unit-test binary gets program directories
of its own the same way.

## Acceptance

1. The predicate above exits 0, and the case is no longer `#[ignore]`d.
2. `tests_have_a_home_of_their_own` covers unit-test modules under `crates/*/src` as well, or the
   case above stays as that check.

## Scope

- `crates/takt-cli/src/main.rs`: the `#[cfg(test)] mod tests` that runs `takt run` (cited)
- `crates/takt-cli/tests/`: a new integration test file for those cases, if they move (inferred)
- `crates/takt/tests/w42_adv_prompt_lines.rs`: the ignored case un-ignored (cited)
- `crates/takt/tests/tests_have_a_home_of_their_own.rs` (inferred)
