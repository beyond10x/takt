---
format: aep.planning-md/3
id: story:docs-edits-the-tests-read-are-attempts
kind: story
status: draft
title: An edit under docs/ that the story's tests read is never counted as an attempt
tags:
- finding
- priority:low
- rsi
refs:
- provider: finding
  reference: docs-edits-the-tests-read-are-attempts
relations:
- serves: vision:safe-agentic-coding
- informed_by: story:a-revision-without-code-is-no-new-attempt
revision: 2
---
# An edit under docs/ that the story's tests read is never counted as an attempt

## What is wrong

Since story:a-revision-without-code-is-no-new-attempt (wave 42 U9), a revision that changes
documentation only does not count toward `REPEATED_FAILURES`. `is_documentation_path`
(`crates/takt/src/run.rs`) calls everything under `docs/`, `README.md` and `CHANGELOG.md`
"documentation the tests cannot depend on". Tests do read it: in this repository
`crates/takt-cli/tests/cases_under_dot_takt.rs` reads a case under `docs/runs/`. When a story's
tests read a file under `docs/`, the edits that decide them are made there and none is counted, so
a run that keeps editing that file without turning its test green goes on to its iteration budget
instead of stopping `RepeatedTestFailure`.

Seen in wave 42's adversarial pass: the scripted case
`attempts_in_a_docs_file_the_tests_read_are_counted` (`crates/takt/tests/w42_adv_repeat_count.rs`,
ignored as this finding) makes 14 implementation attempts in `docs/fixture.txt`, each leaving the
test red, and the run stops `Iterations(30)`.

## Check

Not runnable by `scripts/check-finding`: it runs a library test with a scripted harness, not
`$TAKT`. Checked on the wave 42 tree at a2f650bf: exit 101.

```expected_before
command: cargo test -p b10x-takt --test w42_adv_repeat_count -- --exact attempts_in_a_docs_file_the_tests_read_are_counted --include-ignored
exit: 101
healthy_exit: 0
output: each leaving the test red
source: crates/takt/tests/w42_adv_repeat_count.rs
```

## Outcome

A revision counts as a new attempt when it changes a path the story's tests can read, documentation
or not. Either the documentation rule is narrowed to paths no test reads (for example the paths a
test file names are not documentation), or it is dropped for a story whose tests or scope name a
path under `docs/`. A revision that changes only documentation no test reads still does not count.

## Acceptance

1. The predicate above exits 0, and the case is no longer `#[ignore]`d.
2. `crates/takt/tests/w42_adv_repeat_count.rs`'s other case and the cases of
   story:a-revision-without-code-is-no-new-attempt still pass: a documentation-only revision that
   no test reads is not counted.

## Scope

- `crates/takt/src/run.rs`: `is_documentation_path`, `code_changed` (cited)
- `crates/takt/tests/w42_adv_repeat_count.rs`: the ignored case un-ignored (cited)
- `ess/domains/takt.yaml`: the repeated-failure rule's prose and its citations (inferred)

## Release 0.19.0

0.19.0 ships this story's cases `#[ignore]`d, accepted at its release check as a tracked finding
on the condition that each ignore's reason names this story.
