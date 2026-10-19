---
format: aep.planning-md/3
id: review-result:adversary-hours-pass-1
kind: review-result
status: active
title: Adversary, hours-bounds-a-harness-step, pass 1
relations:
- reviews: story:hours-bounds-a-harness-step
revision: 1
---
# Adversary pass 1: story:hours-bounds-a-harness-step

Unit at 9cd870c (impl/hours, from 7e05b0a). Verdict: NEEDS-CHANGE, 5 red cases and one green
probe in `crates/takt/tests/hours_bounds_a_harness_step_adv1.rs`; 450 → 456 in the b10x-takt
package. A sixth failure in the suite, `ci_pending_waits::an_hour_budget_cuts_the_wait_and_stops_waiting`,
is flaky under load (0.5 s budget, load average 26; it passed alone 3 of 3 times).

Attacked without a break: a setsid'd grandchild that ignores SIGTERM and holds the pipe is killed
before the discard; NaN, negative, 0, huge and infinite budgets; `time_left` reads zero after the
kill; on resume the budget counts from this run's start; the CLI refuses a budget that is not finite
or not above 0.

```findings
[
  {
    "file": "crates/takt/src/run.rs",
    "category": "correctness",
    "severity": "note",
    "message": "time_left is read before step preparation; the child overshoots the run end: the harness is held 24-99 ms past started+budget because time_left is computed before snapshots and checkpoint git calls and the child timeout counts from spawn",
    "line": 3774
  },
  {
    "file": "crates/takt/src/run.rs",
    "category": "correctness",
    "severity": "warning",
    "message": "a harness failing by itself before the budget ends is reported as Hours: a harness that fails with 'not logged in' 100 ms before the budget ends stops the run Ok(Hours) instead of the harness-failed error",
    "line": 3939
  },
  {
    "file": "crates/takt/src/run.rs",
    "category": "correctness",
    "severity": "note",
    "message": "a step killed by the budget leaves no event in events.jsonl: after a budget kill, events.jsonl holds only opened, decision, and stop with steps:1; no line names the killed step",
    "line": 3950
  },
  {
    "file": "crates/takt/src/forge.rs",
    "category": "correctness",
    "severity": "note",
    "message": "a grandchild joining takt's own group without the mark writes after the discard: a claude child's grandchild does setpgid into takt's group and drops TAKT_CHILD, survives run_child and sweep_left, and writes after Hours (constructed)",
    "line": 519
  },
  {
    "file": "crates/takt/src/validator.rs",
    "category": "correctness",
    "severity": "warning",
    "message": "a validator step is not bounded by --hours: a ./check.sh sleeping 15 s under --hours 0.001 makes the run take 15.2 s; the stop is correctly Hours",
    "line": 278
  }
]
```
