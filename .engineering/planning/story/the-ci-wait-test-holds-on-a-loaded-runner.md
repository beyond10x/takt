---
format: aep.planning-md/3
id: story:the-ci-wait-test-holds-on-a-loaded-runner
kind: story
status: draft
title: The CI-wait timing test holds on a loaded macOS runner
tags:
- from-finding
relations:
- informed_by: story:ci-wait-watches-the-check
- decomposes: epic:composable-agentic-work
revision: 1
---
# Story: the CI-wait timing test holds on a loaded macOS runner

## Outcome

`ci_result_follows_any_completion_within_one_cap` in `crates/takt/tests/ci_wait_backoff.rs` passes
on a loaded macOS runner while still failing when the wait does not notice a completion within one
cap.

## Context

The release run of 0.28.0 (https://github.com/beyond10x/takt/actions/runs/41667833256, attempt 1)
failed in one job, "Test the tagged commit on macOS / takt / darwin (test-library)", on this test
alone: "ci_result later than one cap 225ms after the checks passed", one case 706.72943ms late.
The same test passed on the pull request's macOS job and on Linux.

The test scales the backoff down (first wait 100 ms, cap 225 ms) and allows the cap, the run's own
time between reads and 20 ms. On a loaded runner the scheduler delay of one sleep is larger than
that allowance, so the assertion measures the host rather than the wait.

## Acceptance

- The test's allowance follows the measured lateness of the host's own sleeps, or the test checks
  the waits the run recorded instead of wall-clock time, so a delayed scheduler does not fail it:
  tests.
- A wait that ignores a completion for longer than one cap still fails the test: a mutant of the
  reset or the cap is caught.
