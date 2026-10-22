---
format: aep.planning-md/3
id: review-result:wave-33-loop-a23e242
kind: review-result
status: active
title: Grant review, wave 33 loop on run_until_blocked, at a23e242
relations:
- reviews: story:the-loop-is-run-until-blocked
revision: 1
---
approve
Grant review of wave/33 at a23e242 (story:the-loop-is-run-until-blocked against 7b54bad).

| check | result |
|---|---|
| changed and acceptance tests at a23e242 in a scratch tree | all pass (forge 39, merge_under_authority 32, cases_root_link 31, free_disk 19, bin 19, frontier_cli 6), exit 0 |
| `takt run --help` at a23e242 against 43d6531 | byte-identical (`cmp` exit 0) |
| frontier bounds the harness | `offered.contains` guard kept, `crates/takt/src/run.rs:11279` |
| a refused step is reverted | `run.rs:10444-10458` to `undo_step` |
| merge gate, authority | `CaseAuthority` handed to Loom's check; the governor supplies none |
| ESS citations | `check-citations --require-digests` 1058 resolved, 0 unresolved; 13 sampled, 12 exact |
| the 9 changed test expectations and `cases_root_link` | each follows from the approved order |

The gate's one `active_time_budget` failure (load average 37) is load: wall time over 8 interleaved
runs per revision, minimum 5.045 s at 7b54bad and 5.024 s at a23e242, median 5.110 s and 5.284 s.

```findings
- file: crates/takt/src/run.rs
  line: 10129
  category: scope
  severity: warning
  verdict: approve
  origin: introduced
  message: "the idle count across calls (last_idle, run.rs:10129, 10186, 10428-10436) and the forge pause (run.rs:10224-10259) are takt loop control the approval record did not name; necessary at a step budget of 1. Resolved by approval-record:loop-pre-checks-ahead-of-loom revision 3"
- file: crates/takt/src/run.rs
  line: 10444
  category: correctness
  severity: note
  verdict: approve
  origin: introduced
  message: "undo_step after a Stale or NotAdmitted harness proposal rolls the cases directory back past the Loom executor and request observations of that call; inferred from code"
- file: crates/takt/src/run.rs
  line: 10299
  category: correctness
  severity: note
  verdict: approve
  origin: introduced
  message: "a case with two legitimate outcomes: Loom completes only on exactly one, so the turn goes to Taker::Nobody and the run ends NothingAdmissible (exit 3) where 7b54bad completed (exit 0); inferred from code. Filed as story:loop-completes-with-two-legitimate-outcomes"
- file: ess/domains/takt.yaml
  line: 3570
  category: citation
  severity: note
  verdict: approve
  origin: introduced
  message: "cites run.rs:10468 (the refusal block) for 'Exactly one action: the validator's own'; run.rs:477 and 10300 state it"
```
