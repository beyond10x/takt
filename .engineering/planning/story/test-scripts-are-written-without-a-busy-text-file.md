---
format: aep.planning-md/3
id: story:test-scripts-are-written-without-a-busy-text-file
kind: story
status: draft
title: Test files that write a script and then run it can hit Text file busy
summary: 119 test files write a script with fs::write and run it; a fork on another thread can make its exec fail ETXTBSY
tags:
- finding
- priority:low
- rsi
refs:
- provider: finding
  reference: test-scripts-are-written-without-a-busy-text-file
relations:
- serves: vision:safe-agentic-coding
revision: 1
---
# Test files that write a script and then run it can hit "Text file busy"

## What is wrong

Wave 42 U1 found the cause of a flaky library test: the fake `gates` script it wrote with
`fs::write` and then ran failed at exec with "Text file busy (os error 26)" in 3 of 200 runs. A
fork on another test thread inherits the write descriptor of a script still being written
(inferred from the error). U1 fixed the three library tests in `crates/takt/src/harness.rs` with a
helper that writes the script from a child `/bin/sh`.

119 integration-test files in `crates/takt/tests` and `crates/takt-cli/tests` write a script the
same way (`fs::write`, then `set_permissions` to `0o755` or `0o775`) and then run it
(`git grep -lE "from_mode\(0o7[57]5\)" -- crates/takt/tests crates/takt-cli/tests | wc -l`).

## Check

```expected_before
command: git grep -lE "from_mode\(0o7[57]5\)" -- crates/takt/tests crates/takt-cli/tests
exit: 0
healthy_exit: 1
output: crates/takt
source: crates/takt/tests
```

## Outcome

Every test that writes a script it then runs writes it through one shared test-support helper
that holds no write descriptor in the test process, so no test fails at exec with ETXTBSY.

## Acceptance

1. One helper in a shared test-support module writes an executable script without this process
   opening it for writing; the predicate above exits 1.
2. The library's and the CLI's tests pass.

## Scope

- `crates/takt/tests/support/` (inferred)
- `crates/takt-cli/tests` (inferred)
