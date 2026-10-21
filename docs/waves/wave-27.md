# Wave 27: a sweep reaps no child another call is waiting on

Stage: **closing**. Shape: one dispatcher in one checkout, one wave branch `wave/27` cut from `main`
at 8340882b, one sub-agent at a time, approved by the orchestrator under the owner's delegation.

## Units

| unit | story | where |
|---|---|---|
| A | story:a-sweep-reaps-no-other-calls-child | `crates/takt/src/forge.rs`, `crates/takt/src/validator.rs`, `crates/takt/src/harness.rs`, `crates/takt/tests/` |

## Log

- 2026-10-21: wave opened from `main` at 8340882b. Read in the source, not yet traced: the gate's
  command is started by `b10x-assertion-providers` (engineering-protocols 0.9.0,
  `crates/assertion-providers/src/process.rs:175`) in a process group of its own, outside
  `run_child`, so it is not in `RUNNING`; a `sweep_left` another run calls meanwhile in the same
  process kills and reaps it, and the command's own `try_wait` gets ECHILD. Unit A dispatched.
- 2026-10-21: unit A committed in c6c03b6: `forge::hold_sweeps`, a read guard held around the gate's
  collect and the harness session's run; every sweep takes the write guard first. strace of the red
  run: the sweep thread sends SIGKILL to the gate's command, then both threads `waitpid` it and the
  later one gets ECHILD. The new test was red 36 of 36 before (1 with ECHILD), green 50 of 50 after.
  `task check` stopped at 73 citations the change moved; 4774e77 re-points them (0 unresolved).
- 2026-10-21: CI budget (story:the-cli-suite-fits-its-ci-budget, acceptance 3): main check run
  39580233035 at 8340882b passed, jobs 6.0, 12.6 and 1.9 min (test-cli, test-library,
  check-static); 1 of 3 consecutive runs under 20 min, since run 39569813626 at 02130a98 was red.
- 2026-10-21: review of 4774e77, needs-revision. High: the test's side thread took the write lock
  again at once and starved every reader, one file run took 1.6 s to 910 s; 18289f0 pauses 2 ms
  between sweeps (10 of 10 runs in 1.0-1.4 s; with both holds removed the reviewer saw 20 of 20
  fail). Low: `hold_sweeps` now says a thread must not hold it twice. 9f3cc25 re-points the 25
  citations the doc line moved.
- 2026-10-21: close. `task check` at 9f3cc25 with ess 0.66.0: 1619 passed, 0 failed, 6 ignored,
  exit 0. story:a-sweep-reaps-no-other-calls-child implemented on test_result.
