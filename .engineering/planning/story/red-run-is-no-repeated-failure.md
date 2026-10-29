---
format: aep.planning-md/3
id: story:red-run-is-no-repeated-failure
kind: story
status: active
title: The red run a story's new test is meant to give counts toward a repeated failure
tags:
- finding
- priority:high
- rsi
refs:
- provider: finding
  reference: red-run-is-no-repeated-failure
relations:
- serves: vision:safe-agentic-coding
scope:
- confidence: cited
  path: crates/takt/src/run.rs
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-28T21:21:41Z", actor: "human:timo", revision: 3, executor: "agent:planner"}
- {from: "proposed", to: "active", at: "2026-10-28T21:21:42Z", actor: "human:timo", revision: 4, executor: "agent:developer"}
---
# The red run a story's new test is meant to give counts toward a repeated failure

## What is wrong

Under `tests_first = required` (story.delivery/2), a run writes a test, runs it red on purpose,
then edits the implementation. `REPEATED_FAILURES` is 2 (`crates/takt/src/run.rs:202`): two
`tests.run` results in a row, each at a new revision, failing the same tests stop the run
`RepeatedTestFailure`. The intended red run is the first of the two, so a run whose first
implementation edit leaves the new test failing stops at once, with no second edit.

Seen in a self-run of takt at cb9012a2 on its own
story:credential-shape-sees-a-token-after-a-word-character (codex harness, 2026-10-28): step 2
`tests.write` committed the test (c67fe4e2), step 3 `tests.run` failed it as intended, step 4
`implementation.edit` fixed `credential_shape` but not the caller the test reads (f489bbdd), step
5 `tests.run` failed the same test, and the run stopped `RepeatedTestFailure (2 tests.run results
in a row, each at a new revision, failed the same tests: credentials_joined_to_words_are_detected_and_withheld)`
after one implementation attempt, exit 3.

## Check

Not runnable by `scripts/check-finding`: it needs a harness that makes an edit which leaves the
test red. A library test with a scripted harness stands in; it is the unit's first test.

```expected_before
command: cargo test -p b10x-takt --test repeated_failure_counts_after_red -- --exact a_first_edit_that_leaves_the_red_test_failing_is_not_a_repeat
exit: 101
healthy_exit: 0
output: RepeatedTestFailure
source: crates/takt/src/run.rs
```

## Outcome

The red run a protocol asks for before the implementation (the `tests.run` that makes
`tests.shown_red` true) does not count toward `REPEATED_FAILURES`: the count starts at the first
`tests.run` after an implementation edit. Two failing results after two implementation edits, each
at a new revision and failing the same tests, still stop the run as now.

## Acceptance

1. A scripted harness writes a test, the run shows it red, the harness makes an edit that leaves it
   red, the run does not stop `RepeatedTestFailure` and offers `implementation.edit` again.
2. Two implementation edits in a row that each leave the same tests failing stop the run
   `RepeatedTestFailure`, as today.
3. A run without `tests_first` (story.delivery/1) keeps today's count.
4. The predicate above exits 0.

## Scope

- `crates/takt/src/run.rs`: `REPEATED_FAILURES` and the count at its use (cited)
- `crates/takt/tests/repeated_failure_counts_after_red.rs`: new (inferred)
