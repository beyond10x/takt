---
format: aep.planning-md/3
id: review-result:adversary-u4-pass-1
kind: review-result
status: active
title: 'Adversary pass 1 on U4: a step without an action is retried'
relations:
- reviews: story:a-harness-step-without-an-action-is-retried
revision: 1
---
# Adversary pass 1 on U4 (a step without an action is retried)

Tree: `wave/38` at 417f4b33 plus the uncommitted unit edits. Test file added:
`crates/takt/tests/no_action_retried_adv1.rs`: 1 passed, 2 failed. no_action_retried 5 passed;
no_action_tries_help 1 passed.

| file:line | verdict | origin | finding |
|---|---|---|---|
| crates/takt/src/run.rs:9994 | warning | introduced | a no-action try without usage writes a step line without tokens; the scorecard reads the run's tokens as unknown |
| crates/takt/src/run.rs:596 | note | introduced | "the step is tried again" is printed before the host stop is asked |
| crates/takt/src/run.rs:11047 | note | introduced | each try counts against `--iterations`; a budget ending at a try is exit 3, not 5 (intended, documented) |

Not broken: the try counter is per step; the exact match of loom's text; refused steps are not
retried; each retry starts from the discarded state; budget stops between tries; usage between
tries; replay; a resume after exit 5.

```findings
[
  {"file": "crates/takt/src/run.rs", "line": 9994, "category": "contract-drift", "severity": "warning", "verdict": "INFEASIBLE", "origin": "introduced", "message": "a no-action try whose session reported no usage writes a harness step line without tokens, which the scorecard reads as unknown run tokens, so a recovered run's case is refused by scorecard --against and promote; no real harness was shown to end without a usage event"},
  {"file": "crates/takt/src/run.rs", "line": 596, "category": "acceptance", "severity": "note", "verdict": "CONFIRMED", "origin": "introduced", "message": "the line saying the step is tried again is printed before host_stop is asked, so a run stopped by a budget or low disk after a failed try announces a retry it never makes"},
  {"file": "crates/takt/src/run.rs", "line": 11047, "category": "judgement", "severity": "note", "verdict": "CONFIRMED", "origin": "introduced", "message": "each try counts against --iterations, so a run whose step budget ends at a no-action try ends exit 3 on Iterations instead of exit 5 and the scorecard ranks it as a budget stop, not an error"}
]
```
