---
format: aep.planning-md/3
id: story:a-pinned-test-failing-on-is-a-repeated-failure
kind: story
status: draft
title: A pinned test that keeps failing stops the run
summary: RepeatedTestFailure counts a test green at the base and red across edits
tags:
- trial-0.24.0
revision: 1
---
## Outcome

A run stops on RepeatedTestFailure when the same test, green at the base, keeps failing across
code edits, even if the set of failing tests changes around it.

## Context

In a cluster trial run, step 8 failed one test and step 10 failed the same test and a new one;
the run went on to the token budget. `REPEATED_FAILURES = 2`
(`crates/takt/src/run.rs:215`) counts only "the same set of tests", so a pinned test that stays
red while others come and go is never counted.

## Acceptance

1. Two `tests.run` results in a row, at new revisions, failing {a} and then {a, b}, with `a`
   green at the base, stop the run with RepeatedTestFailure naming `a`.
2. Failing {a} and then {b} still does not stop the run.
3. A test that was already red at the base is not counted.
