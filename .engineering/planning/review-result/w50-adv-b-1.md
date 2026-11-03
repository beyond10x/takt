---
format: aep.planning-md/3
id: review-result:w50-adv-b-1
kind: review-result
status: active
title: Wave 50 adversary pass 1 on story:bench-subset-and-budget
relations:
- reviews: story:bench-subset-and-budget
revision: 1
---
unit: b (story:bench-subset-and-budget), working tree of rv/50-adv-b at 9385bf5b1 plus one new test file
verdict: red
cases: executed 4→10, red 2
origin: introduced 6, pre-existing 0, undecided 0
wrote-outside-worktree: none
needs-coordinator: yes. The story's acceptance command leaves out `--protocol`, and run as written it stops `RedBase`. Either the story text or the implementation has to change, and that is your call.

**Diff stat:** `?? crates/takt-cli/tests/w50_adv_usd_cli.rs`. That is the only change, it is a test file, and nothing is committed.

**Cases added** (`crates/takt-cli/tests/w50_adv_usd_cli.rs`). I ran the file on its own first: 4 passed, 2 failed, exit 101.
- `the_acceptance_command_as_written_stops_on_the_dollar_budget` (:152), **red**: `stopped: RedBase (the tests fail at the case's base 1ec22780, ...); steps 1, tokens 0` on `protocol software.change/1 (default)`.
  - The story's command has no `--protocol`, so it opens on software.change/1 and stops at the red base before any harness step. The unit's own test passes `--protocol protocols/bench-task/1.yaml` (`usd_budget_cli.rs:105`).
- `ten_steps_of_ten_cents_reach_a_budget_of_one_dollar` (:206), **red**: `stopped: Usd (1); steps 11, tokens 11, wall 1.1s, cost $1.10`.
  - Ten reports of 0.1 add up to 0.9999999999999999 in f64, which is under 1, so the run pays for an 11th step. Meanwhile the console rounds the cost to `$1.00`.
  - The spec declares `usd_budget: Optional<Decimal>`, and a downstream adopter's record wants `budget_micro_usd`.
  - Suggested fix (not applied): sum and compare in integer micro-dollars, or round the sum before `cost >= usd` at `run.rs:10588`.
- Green, as the story says:
  - `three_steps_of_a_cent_...`: 0.01 three times lands exactly on 0.03 (a round-half-even tie), so the run stops at step 3. This case also catches a `>=`→`>` mutant, which the unit's 0.02-a-step test cannot.
  - `a_harness_reporting_cost_on_some_steps_...`: costs 0.02, none, 0.02 stop at step 3, and the "reports no cost" line prints once.
  - `a_resumed_run_counts_its_dollars_from_zero`: `--case` resume reports `cost $0.04`, so the count restarts.
  - `a_run_reaching_both_budgets_at_once_prints_one_stop`: only one stop line ever prints. When both budgets run out on the same step, `Tokens` wins (`budget_spent` checks it first).

**Suite run, after the cases existed:** `build-slot.sh cargo test --locked -p b10x-takt-cli --test usd_budget_cli --test w50_adv_usd_cli`
- `usd_budget_cli`: `test result: ok. 4 passed; 0 failed`
- `w50_adv_usd_cli`: `test result: FAILED. 4 passed; 2 failed`, `EXIT=101`
- `cargo fmt --all --check`: EXIT=0. I ran `rustfmt --edition 2024` on my own file first.
- `cargo clippy --locked -p b10x-takt-cli --test w50_adv_usd_cli -- -D warnings`: `Finished`, EXIT=0
- `du -sh target`: 2.4G

**Judgement findings**, all on the tree above. What reaches each:
- Float sum: any `--usd` budget that the reported costs hit exactly. That is rare with real Claude costs, and certain with a fixed-cost double or a runner that hands each task "what is left" of the run's budget.
- Acceptance text: the story's command, run verbatim.
- `crates/takt/tests/scorecard.rs:987` is a constant pin: it asserts that `METRICS` contains `("outcome", 4)`. The CLI test of the saved card already covers this.
- The fixture `scorecard-outcome-v3/` now holds outcome 4. `promotion.rs:3369`, the test named for "saved under v3", rewrites the version in memory instead of reading a file saved under v3, so the names no longer describe the data.
- `run.rs:10639`: "the harness reports no cost" also prints when the harness reported costs on other steps. The wording is misleading, but the behaviour is right.
- `harness.rs:3709` reads `total_cost_usd` without checking it, so a negative cost lowers the count. Claude never reports a negative cost; I built this state, nobody reaches it (INFEASIBLE).

**Attacked and could not break:**
- A `Usd` stop happens before a step, never inside one: 0.02 a step against 0.03 stops after step 2.
- The stop names the budget as parsed (`0.030` prints as `0.03`), and the 0.03 case in the acceptance prints exactly `Usd (0.03)`.
- NaN cannot reach the sum, because JSON has no NaN.
- `outcome_of` and `rank` treat `Usd` as a budget stop end to end.
- Outcome v4 refuses a card saved under v3 the same way v3 refused v2, which is the intended version bump.
- `--usd` reaches `takt scenario run` through the shared `LoopArgs`.

**Deviations:**
- At my first build two other repositories' cargo processes were running, one more than the invariants allow. I queued through the build slot anyway.
- My first log is at `target/scratch-adv-b-run1.log`, not under `target/scratch/`. It is still inside the tree.

```findings
[
{"file": "crates/takt/src/run.rs", "line": 11680, "category": "arithmetic", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "the dollar count is an f64 sum compared with >=: ten reports of 0.1 sum to 0.9999999999999999 and --usd 1 lets an 11th step run (the spec says Decimal) (test: crates/takt-cli/tests/w50_adv_usd_cli.rs::ten_steps_of_ten_cents_reach_a_budget_of_one_dollar)"},
{"file": ".engineering/planning/story/bench-subset-and-budget.md", "line": 59, "category": "contract-drift", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "the acceptance command as written (no --protocol) opens software.change/1 and stops RedBase, exit 3, never Usd (0.03); the unit's test passes --protocol bench-task/1 instead (test: crates/takt-cli/tests/w50_adv_usd_cli.rs::the_acceptance_command_as_written_stops_on_the_dollar_budget)"},
{"file": "crates/takt/tests/scorecard.rs", "line": 987, "category": "test-quality", "severity": "note", "verdict": "CONFIRMED", "origin": "introduced", "message": "constant pin: asserts METRICS contains (outcome, 4) and observes no behaviour"},
{"file": "crates/takt/src/promotion.rs", "line": 3369, "category": "test-quality", "severity": "note", "verdict": "CONFIRMED", "origin": "introduced", "message": "the fixture dir scorecard-outcome-v3 now holds outcome 4, and the test named for a card saved under v3 rewrites the version in memory instead of reading one"},
{"file": "crates/takt/src/run.rs", "line": 10639, "category": "docs", "severity": "note", "verdict": "CONFIRMED", "origin": "introduced", "message": "'the harness reports no cost' is said when a harness reported cost on other steps and omitted it on one"},
{"file": "crates/takt/src/harness.rs", "line": 3709, "category": "boundary", "severity": "note", "verdict": "INFEASIBLE", "origin": "introduced", "message": "a negative total_cost_usd is summed unchecked and lowers the count; nobody shown to reach it"}
]
```
