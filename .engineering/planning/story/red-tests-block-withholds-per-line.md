---
format: aep.planning-md/3
id: story:red-tests-block-withholds-per-line
kind: story
status: implemented
title: The red tests' block is all or nothing under the secret scan
summary: A red test run's block makes no path relative and withholds no single line; one refused line hides every failing test.
tags:
- finding
- priority:medium
- rsi
refs:
- provider: finding
  reference: red-tests-block-withholds-per-line
relations:
- serves: vision:safe-agentic-coding
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-29T04:44:14Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-29T04:44:14Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-10-29T05:12:57Z", actor: "human:timo", revision: 4, decided_on: {"recorded":{"test_result":1}}}
---
# The red tests' block is all or nothing under the secret scan

## What is wrong

A failed gate's block writes each path under the tree relative to it and withholds only the lines
the secret scan refuses (`relative_to_tree`, `refused_lines`, `failed_block_scanned` in
`crates/takt/src/harness.rs`). The block a failed `tests.run` opens the prompt with
(`red_tests_block_scanned`) does neither: a test runner that prints the tree's absolute path, as
`cargo test` does in a panic's location for a path outside the crate, fails the scan's
personal-paths rule, and the whole block becomes one line saying the test run failed. The step
then gets no failing test and no assertion location. Found in the review of the self-run branch's
merge into `integration/takt` (finding 2).

## Check

```expected_before
command: awk '/^fn red_tests_block_scanned\(/{f=1} f&&/^}/{exit} f&&/relative_to_tree|refused_lines|block_withheld/{n++} END{if(n){print "the red tests block makes paths relative or withholds lines"; exit 0} print "the red tests block is all or nothing under the scan"; exit 9}' crates/takt/src/harness.rs
exit: 9
healthy_exit: 0
output: the red tests block is all or nothing under the scan
```

## Outcome

The red tests' block writes paths under the tree relative to it and withholds only the lines the
scan refuses, with the count in the check's heading, as the gates' block does; the block naming
the assertions is built from the kept lines.

## Acceptance

1. The predicate above exits 0.
2. A test: a red test run whose output names the tree's absolute path and one line the scan
   refuses gives a red tests' block with the paths relative, that line withheld and counted, and
   the other tests and their assertion locations shown.
