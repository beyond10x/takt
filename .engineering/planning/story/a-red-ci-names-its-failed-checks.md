---
format: aep.planning-md/3
id: story:a-red-ci-names-its-failed-checks
kind: story
status: draft
title: A red CI read names its failed checks in the next prompt
summary: Checks::Fail carries names and output; a fail no edit can fix stops
tags:
- finding
- priority:medium
- trial-0.24.0
relations:
- informed_by: story:a-failed-check-read-is-offered-again
revision: 1
---
## Outcome

A red CI read names its failed checks (name, conclusion, and a summary or log tail where the
forge gives one) in the `ci.read` effect and in the next prompt's failed block. When every failed
check is one no offered edit can fix, the run stops with a named stop instead of offering an edit.

## Context

In a cluster trial run on 0.24.0, `ci.read` said only "checks on pull request #… at … through
connectors: fail". The failing check was the target repository's planning validation, failing on
a field takt's own store write had added; no edit the model may make (`.engineering/**` is on
the judge surface) could fix it. The next step guessed with `tests.write` at 1.2M tokens, and the
run stopped on its token budget at 4,012,613 of 3,000,000.

Where, at 0.24.0: `forge::Checks` is only `Pass | Fail | Pending`
(`crates/takt/src/forge.rs:105-110`); `CiRead` (`crates/takt/src/validator.rs:4359-…`, writes at
`:4459-4495`) does not implement `take_failed`, whose default returns nothing
(`validator.rs:128-130`); the github check reading keeps status and conclusion and drops the name
(`crates/takt/src/forge/connectors.rs`). `FailedCheck`, which the prompt's failed block reads
(`crates/takt/src/run.rs:9206`, `crates/takt/src/harness.rs:48-52`), comes only from `tests.run`
and `gate.run`. Related: story:a-failed-check-read-is-offered-again (re-reading, not naming),
story:ci-wait-watches-the-check (names in `ci_result`, not in the prompt).

```expected_before
command: a forge double reports one failed check run named "planning validate"; run ci.read and the next judgement step
output: neither the ci.read effect nor the next prompt contains "planning validate"
```

## Acceptance

1. `Checks::Fail` carries each failed check's name, conclusion and details link, and the summary
   or log tail where the forge gives one, bounded in size.
2. They are recorded as `FailedCheck { id: "ci.read:<name>" }`, so the next prompt's failed block
   shows them, and the `ci.read` effect line names them.
3. When every failed check reads only paths the step may not change, the run stops with a named
   stop that lists them.
4. Tested against the github and gitlab paths of the connectors forge and the local forge.
