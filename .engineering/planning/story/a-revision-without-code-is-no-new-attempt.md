---
format: aep.planning-md/3
id: story:a-revision-without-code-is-no-new-attempt
kind: story
status: implemented
title: A revision that changes no code counts toward a repeated test failure
summary: a doc-only edit after a red run is the second failure, and a resume stops at once
tags:
- finding
- priority:high
- rsi
refs:
- provider: finding
  reference: a-revision-without-code-is-no-new-attempt
relations:
- serves: vision:safe-agentic-coding
- informed_by: story:red-run-is-no-repeated-failure
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-28T23:15:14Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-28T23:15:15Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-10-29T21:30:15Z", actor: "human:timo", revision: 4, decided_on: {"recorded":{"test_result":1}}}
---
# A revision that changes no code counts toward a repeated test failure

## What is wrong

`REPEATED_FAILURES` (`crates/takt/src/run.rs`) stops a run when two `tests.run` results in a row,
each at a new revision, fail the same tests. A revision that changes documentation only, or no
file the tests build from, counts as a new revision. In the proof run of takt 0.18.1 on a story of
beyond10x/llm (2026-10-28), a doc-only edit after a red run made the second failure, and the run
stopped. A resume reruns the tests at once and stops again, so the case cannot recover.

Related, not the same: story:red-run-is-no-repeated-failure (the intended red run before the
implementation counts as the first repeat).

## Check

```expected_before
command: cargo test -p b10x-takt --test repeated_failure_counts_code -- --exact a_doc_only_revision_is_no_new_attempt
exit: 101
healthy_exit: 0
output: error: no test target named `repeated_failure_counts_code`
source: crates/takt/src/run.rs
```

## Outcome

A failing `tests.run` counts toward `REPEATED_FAILURES` only when the revision it ran at changed,
since the previous failing run, a path outside `docs/**` and the repository's other documentation
paths. A resumed case gets at least one implementation edit before the count can stop it again.

## Acceptance

1. A scripted harness that makes a doc-only edit after a red run does not stop the run
   `RepeatedTestFailure`; the step is offered again.
2. Two code edits in a row that leave the same tests failing stop the run as today.
3. A case stopped `RepeatedTestFailure` and resumed runs at least one implementation edit before
   it can stop on the same reason.

## Scope

- `crates/takt/src/run.rs` (inferred)
