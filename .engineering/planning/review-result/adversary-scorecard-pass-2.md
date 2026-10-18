---
format: aep.planning-md/3
id: review-result:adversary-scorecard-pass-2
kind: review-result
status: active
title: Adversary, scorecard-reader, pass 2
relations:
- reviews: story:scorecard-reader
revision: 1
---
unit: story:scorecard-reader, worktree w10-scorecard at e7e8698 (on 7545103, base 9bd9d18) plus one untracked test file
verdict: NEEDS-CHANGE
cases: executed 493→501, red 4
origin: introduced 2 / pre-existing 0 / undecided 0
wrote-outside-worktree: 3 logs in the unit's scratch `adv2/`
needs-coordinator: a new comparison rule (decided: dominance per outcome rank)

**Cases** (`crates/takt/tests/scorecard_adv2.rs`)

| case | expected | actual |
|---|---|---|
| `a_candidate_that_regresses_two_of_five_trials_is_worse` | at least 1 worse | 0 (`median rank 4 against 4`) |
| `one_saved_case_against_five_candidate_cases_two_regressed_is_worse` | at least 1 worse | 0 |
| `self_is_never_worse_and_one_lowered_case_always_is` (200 seeded) | self never worse; one lowered always worse | self held in 200; 68 strictly worse candidates not worse |
| `an_accepted_case_whose_later_run_errored_stays_accepted` | accepted | error |
| `an_even_count_regression_off_the_middles_is_worse` | worse | green |
| `a_scenario_only_the_candidate_has_is_not_worse` | `no baseline`, not worse | green |
| `a_line_with_three_instants_reads_its_first` | first `at` | green |
| `a_save_does_not_follow_a_link_or_a_path_revision` | refused, nothing written | green |

**Gate.** `task check` exit 201 at tests; without fail-fast 497 passed, 4 failed; all regression lanes pass.

**Attacked, not broken.** Self-comparison never worse (200 seeded); a case belongs to one scenario; a scenario only the candidate has reads `no baseline`; `last_usage` reset per step, per instance, recorded for refused steps; `Events::read` first-`at` rule; `--save` hex check, refusals, `create_new` against links and existing files.

```findings
[
  {"file": "crates/takt/src/scorecard.rs", "line": 663, "category": "property", "severity": "blocker", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "the median outcome rank misses any regression in fewer than half a scenario's cases: 5 accepted against 3 accepted plus 2 errors is not worse, and 68 of 200 seeded candidates with one case lowered and none raised pass"},
  {"file": "crates/takt/src/scorecard.rs", "line": 134, "category": "boundary", "severity": "note", "verdict": "INFEASIBLE", "origin": "introduced", "message": "an accepted case whose later resumed run ended on an error reads as error"}
]
```
