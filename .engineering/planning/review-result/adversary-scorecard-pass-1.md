---
format: aep.planning-md/3
id: review-result:adversary-scorecard-pass-1
kind: review-result
status: active
title: Adversary, scorecard-reader, pass 1
relations:
- reviews: story:scorecard-reader
revision: 1
---
unit: story:scorecard-reader, worktree w10-scorecard at 7545103 (base 9bd9d18) plus 2 untracked test files
verdict: NEEDS-CHANGE
cases: executed 479→489, red 9
origin: introduced 8 / pre-existing 0 / undecided 0
wrote-outside-worktree: the unit's scratch `adv1/`
needs-coordinator: what --against compares (decided: one outcome per case, scenarios by median outcome rank); a scenario with no candidate run (decided: worse)

**Cases** (`crates/takt/tests/scorecard_adv1.rs`, `crates/takt-cli/tests/scorecard_adv1.rs`)

| case | expected | actual |
|---|---|---|
| `a_resumed_scenario_one_case_is_not_worse_than_itself` | 0 worse | 1 (`ApprovalRequired against accepted worse`) |
| `a_scorecard_with_varying_trials_is_not_worse_than_itself` | 0 | 1 (`NoProgress against accepted worse`) |
| `a_baseline_scenario_the_candidate_never_ran_is_not_passed_silently` | at least 1 worse | 0 |
| `a_crashed_runs_wall_time_ends_at_its_own_last_line` | under 500 ms | 600 ms |
| `a_wave_09_store_evidence_line_does_not_stretch_the_runs_wall_time` | 2000 | 3902000 |
| `a_changed_outcome_definition_does_not_switch_the_outcome_gate_off` | at least 1 worse | 0 (`not compared`) |
| `huge_reported_tokens_do_not_panic_or_wrap` | error or saturated | panic: attempt to add with overflow |
| `the_loop_records_the_four_components_the_harness_parsed` | usage {11,22,33,44} | no `usage` key |
| `a_saved_scorecard_against_itself_exits_0` | exit 0 | 3 |
| `save_on_an_unborn_head_is_refused_and_writes_nothing` | exit 1 | green |

**Gate.** `task check` exit 201 at tests (ess, 380 of 380 citations, fmt, clippy pass); without fail-fast 480 passed, 9 failed.

**Attacked, not broken.** Negative or float tokens read as 0; two `opened` without a `stop` read as unfinished; lines after a `stop` ignored; a planted `task_spend` never read; case ids parsed with `rsplit`; old whole-second instants parse; even-n median exact; exit 3 only from a worse count; ESS validates and its citations resolve. A symlinked case directory is skipped silently (nothing reaches it).

```findings
[
  {"file": "crates/takt/src/scorecard.rs", "line": 439, "category": "acceptance", "severity": "blocker", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "--against ranks every run against the best saved run, so a scenario-1 case (ApprovalRequired run, grant, resumed accepted run) compared with itself is worse and exits 3"},
  {"file": "crates/takt/src/scorecard.rs", "line": 442, "category": "boundary", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "a saved scenario with no candidate run, or only a no-record case, is passed silently with 0 worse and exit 0"},
  {"file": "crates/takt/src/scorecard.rs", "line": 460, "category": "acceptance", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "a changed outcome definition version turns the outcome comparison off for the cycle"},
  {"file": "crates/takt/src/run.rs", "line": 1595, "category": "acceptance", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "the harness already parses the four token components but the loop records usage as None"},
  {"file": "crates/takt/src/scorecard.rs", "line": 213, "category": "contract-drift", "severity": "note", "verdict": "CONFIRMED", "origin": "introduced", "message": "a wave-09 store_evidence line's second at key wins on read, stretching the run's wall time"},
  {"file": "crates/takt/src/scorecard.rs", "line": 201, "category": "boundary", "severity": "note", "verdict": "CONFIRMED", "origin": "introduced", "message": "the skipped line written at resume falls inside the crashed run's span"},
  {"file": "crates/takt/src/scorecard.rs", "line": 219, "category": "boundary", "severity": "note", "verdict": "INFEASIBLE", "origin": "introduced", "message": "summing step tokens overflows u64"},
  {"file": "crates/takt/src/scorecard.rs", "line": 42, "category": "judgement", "severity": "note", "verdict": "CONFIRMED", "origin": "introduced", "message": "no build sets TAKT_REVISION, so saves are named by HEAD of the scenario repository, and an existing file is overwritten silently"}
]
```
