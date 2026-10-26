---
format: aep.planning-md/3
id: review-result:adversary-u1-pass-1
kind: review-result
status: active
title: 'Adversary pass 1 on U1: red base and repeated test failures stop the run'
relations:
- reviews: story:repeated-identical-test-failures-stop-the-run
- reviews: story:a-red-base-stops-at-open
revision: 1
---
# Adversary pass 1 on U1 (red base stops at open; repeated identical test failures stop the run)

Tree: `wave/38` at 417f4b33 plus the uncommitted unit edits. Test files added:
`crates/takt/tests/test_failures_adv1.rs` (1 red), `crates/takt-cli/tests/test_failure_stops_cli_adv1.rs`
(1 passed, 4 failed). The unit's own 11 cases pass.

| # | file:line | verdict | origin | finding |
|---|---|---|---|---|
| 1 | crates/takt/src/validator.rs:1098 | blocker | introduced | the 300-character summary cuts the trial's three failing names; no set is named and the repeated stop never fires (24 steps, 8 harness calls) |
| 2 | crates/takt/src/run.rs:10665 | blocker | introduced | the red-base stop is tied to the run's number, so a case first tested in a later run is not stopped |
| 3 | crates/takt/src/run.rs:10665 | warning | introduced | a restart after the red-base stop calls the harness 3 times at the unchanged base |
| 4 | crates/takt/src/run.rs:10663 | warning | introduced | the repeated-failure count starts at zero each run; every restart spends 3 more harness calls |
| 5 | crates/takt-cli/src/main.rs:645 | note | introduced | the opt-out is a `takt run` flag only |
| 6 | crates/takt/src/scorecard.rs:86 | note | introduced | an environment-caused red base is ranked in comparisons, unlike a low disk |

```findings
[
  {"file": "crates/takt/src/validator.rs", "line": 1098, "category": "acceptance", "severity": "blocker", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "the 300-char summary bound cuts the trial's own three failing test names, so failures_in names no set and RepeatedTestFailure never fires on the trial it was written for (24 steps, 8 harness calls, no stop)"},
  {"file": "crates/takt/src/run.rs", "line": 10665, "category": "acceptance", "severity": "blocker", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "RedBase is tied to run_number == 1, so a case whose first tests.run comes in a later run (the trial's runs 1-5 stopped before any test result) is never stopped and spends model calls at a red base"},
  {"file": "crates/takt/src/run.rs", "line": 10665, "category": "acceptance", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "a pod restart after RedBase (exit 3) resumes at the unchanged red base and calls the harness 3 times, so the stop holds for one run only"},
  {"file": "crates/takt/src/run.rs", "line": 10663, "category": "acceptance", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "the repeated-failure count starts at zero each run, so every restart after RepeatedTestFailure spends 3 more harness calls with no upper bound"},
  {"file": "crates/takt-cli/src/main.rs", "line": 645, "category": "judgement", "severity": "note", "verdict": "INFEASIBLE", "origin": "introduced", "message": "the red-base opt-out is a takt run flag only; takt scenario run and the run configuration file cannot reach it"},
  {"file": "crates/takt/src/scorecard.rs", "line": 86, "category": "judgement", "severity": "note", "verdict": "INFEASIBLE", "origin": "introduced", "message": "a RedBase caused by the environment measures the machine as DiskLow does, yet it is still ranked in scorecard comparisons"}
]
```
