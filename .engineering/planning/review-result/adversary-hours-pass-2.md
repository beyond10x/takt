---
format: aep.planning-md/3
id: review-result:adversary-hours-pass-2
kind: review-result
status: active
title: Adversary, hours-bounds-a-harness-step, pass 2
relations:
- reviews: story:hours-bounds-a-harness-step
revision: 1
---
# Adversary pass 2: story:hours-bounds-a-harness-step

Unit at be1fd97 (impl/hours; unit 9cd870c, correction 1 be1fd97). Verdict: NEEDS-CHANGE, 3 red
cases and 2 green probes in `crates/takt/tests/hours_bounds_a_harness_step_adv2.rs`; 456 → 461 in
the b10x-takt package. Checked for flakiness: the file 3 times at load average 20-40 and once in the
suite, same result each time; margins are seconds.

Attacked without a break: resume (the deadline is recomputed from each run's start); the clock
(`Instant`, monotonic); a harness that returns exactly at the deadline; the refusal event (once,
naming the step, kept after the discard); a test command that forks a setsid daemon ignoring
SIGTERM (killed, run bounded); forge validators (exempt, as the prose says).

```findings
[
  {
    "file": "crates/takt/src/validator.rs",
    "category": "correctness",
    "severity": "warning",
    "message": "gate.run commands are not held to the hour budget, and the prose does not say so: a gate command sleeping 25 s under --hours 0.001 makes the run take 28.2 s and stop Completed(accepted) past its end",
    "line": 502
  },
  {
    "file": "crates/takt/src/run.rs",
    "category": "correctness",
    "severity": "note",
    "message": "a step that returns success past the run's end keeps and commits its change: a harness that writes tests/late.txt and returns Ok 2 s past the deadline gets it committed and the run stops Hours",
    "line": 3971
  },
  {
    "file": "ess/domains/takt.yaml",
    "category": "correctness",
    "severity": "note",
    "message": "prose says a budget-ended observation stops on hours; a test command that also wrote into the tree stops step_refused: check.sh appends to README.md and sleeps 20 s under --hours 0.001; the stop is StepRefused and the refusal says timed out",
    "line": 1783
  },
  {
    "file": "crates/takt/tests/ci_pending_waits.rs",
    "category": "correctness",
    "severity": "note",
    "message": "the reason given for the 3 s budget in ci_pending_waits is wrong: the memory-forge validators ignore time_left; the change only widens the margin of an existing flake",
    "line": 414
  }
]
```
