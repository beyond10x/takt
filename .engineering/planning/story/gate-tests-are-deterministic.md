---
format: aep.planning-md/3
id: story:gate-tests-are-deterministic
kind: story
status: draft
title: The gate's tests pass or fail on the code alone
relations:
- decomposes: epic:composable-agentic-work
revision: 1
---
## Outcome

`task check` gives the same result on an unchanged tree whatever else the machine is doing: the tests below pass or fail on the code alone, not on load, timing or the caller's environment.

## Context

Seen on 2026-10-18 in gate runs of unchanged trees, each passing on a rerun with no change:
- `ci_pending_waits::an_hour_budget_cuts_the_wait_and_stops_waiting` (a 0.5 s budget) failed twice while other builds loaded the machine (reviewer verdicts on b0bfad2 and f5f0b2c).
- `records_reach_base_adv1::records_of_an_earlier_revision_reach_the_base_with_a_later_grant` failed once with `tests.run wrote what its validator does not: .git/info/refs` (the step guard refused the step); `.git/info/refs` is what `git update-server-info` writes; the writer was not found (the developer session's gate on e556ecf).
- `approval_record` tests failed 6 times under a reviewer's `XDG_CACHE_HOME` override (f5f0b2c review): the tests read the caller's cache location.
- `version_cli` compared the binary's revision with the live HEAD and failed when main moved during the suite (wave 11 close; filed in story:wave11-review-follow-ups).

## Scope

- `crates/takt/tests/ci_pending_waits*.rs`: budgets measured against a clock that load cannot stretch past the assertion (inferred)
- `crates/takt/tests/records_reach_base_adv1.rs` and the step guard's `.git/` entries in `crates/takt/src/git.rs`: who writes `.git/info/refs`, and whether a validator's own git call may (inferred)
- `crates/takt/tests/approval_record*.rs`: an explicit cache directory per test (inferred)

## Acceptance

Each named test passes 20 of 20 runs of `task check` with two other `cargo build --release` jobs running beside it, and under `XDG_CACHE_HOME` set to an empty directory.
