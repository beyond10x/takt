---
format: aep.planning-md/3
id: review-result:adversary-loop-gaps-pass-1
kind: review-result
status: active
title: Adversary, loop-test-gaps, pass 1
relations:
- reviews: story:loop-test-gaps
revision: 1
---
unit: story:loop-test-gaps, impl/loop-test-gaps at 9507da2, plus my uncommitted file `crates/takt/tests/loop_gaps_adv1.rs`
verdict: CONFIRMED (warning): 4 natural mutants of the budget checks get past `loop_gaps.rs`
cases: executed 218→221, red 0 on the tree; each of the 4 mutants turns exactly 1 of my cases red
origin: introduced 3 / pre-existing 0 / undecided 0
wrote-outside-worktree: 1 directory, `<wave-scratch>/scratch/loop-test-gaps/adv1/`
needs-coordinator: keep `loop_gaps_adv1.rs` or fold its 3 cases into `loop_gaps.rs`; delete `adv1/target` (782M)

**1. Diff of the worktree.** `git --no-pager diff --stat` prints nothing, because the only change is the untracked `?? crates/takt/tests/loop_gaps_adv1.rs`. `git diff -- crates/takt/src` is 0 lines. I made every mutation in a copy under the scratch directory and never touched the worktree's source.

**2. Cases added** (`crates/takt/tests/loop_gaps_adv1.rs`). All 3 pass on the tree (`3 passed`, 1.35 s). Each one goes red against a mutated copy in which `loop_gaps.rs` stays fully green (`5 passed`):

| Case | Mutant (run.rs line) | Red line |
|---|---|---|
| `adv1_the_time_budget_does_not_stop_a_run_before_it_is_spent` (30 s budget, run reaches the gate) | H1, `:609` `>=` → `<` | `left: Hours(0.008333333333333333) right: ApprovalRequired(["repository.merge"])` |
| same case | H2, `:609` `h * 3600.0` → `h * 60.0` | same line as H1 |
| `adv1_the_time_budget_counts_every_step_not_only_the_last` (0.2 s steps against a 0.5 s budget) | H3, `:883` the clock restarts before every harness step | `left: Iterations(40) right: Hours(0.0001388888888888889)` |
| `adv1_the_token_budget_stops_a_run_whose_step_overshoots_it` (budget 50, a step reports 100) | T1, `:614` `>=` → `==` | `left: ApprovalRequired(["repository.merge"]) right: Tokens(50)` |

**3. Suite run** (my case already existed). `cargo fmt -p b10x-takt --check && cargo clippy … -D warnings && cargo test -p b10x-takt --locked --no-fail-fast` passed with `EXIT=0`: 20 test binaries, 221 cases, 0 failed. Full log: `adv1/suite.log`.

**4. Findings** (they cover 9507da2 plus my uncommitted file):

| # | file:line | What was measured | What reaches it | Verdict / origin / severity |
|---|---|---|---|---|
| 1 | `crates/takt/tests/loop_gaps.rs:321` | The test only checks the hours budget from above (`steps <= 2`). A run that stops before its first step passes, so H1 and H2 survive. | `--hours` on every run; H2 is a units mistake (an hour read as a minute) | CONFIRMED / introduced / warning |
| 2 | `crates/takt/tests/loop_gaps.rs:315` | The one step (600 ms) is longer than the whole budget (500 ms), so the test cannot tell total run time from time since the last step. H3 survives. | a run of many short steps under `--hours` | CONFIRMED / introduced / warning |
| 3 | `crates/takt/tests/loop_gaps.rs:287` | The budget equals the reported count exactly (100), so the `==` mutant T1 survives. The `Some(201)` half is only ever compared at 0 and 100 tokens, so it adds no kill the first half lacks. | any harness that overshoots `--tokens` in one step (the usual case) | CONFIRMED / introduced / warning |

**5. Attacked and not broken:**
- The tests kill mutants A–D for the reasons they claim. The implementor's logs show it, and I re-ran B in my copy: `the_gate_requested_with_a_changed_file…` goes red. T2 (`tokens >= t` → `>`) is killed by `the_token_budget_stops_the_run_once_reached…`.
- H4 (`elapsed >= h*3600` → `>`) survives every test, mine included. It is an equivalent mutant: the clock is continuous, so the two comparisons cannot be told apart.
- An inspect step that only deletes `check.sh` is refused with `repository.inspect changed 1 path(s)` and the file comes back. Deletions are counted (`git.rs:1195`). This was a probe in the copy only.
- A merge-gate request with a staged-only change is refused earlier, by the index rule (`run.rs:~978`): `repository.merge changed the index; …`. That refusal still holds when mutant B is applied, so mutant B cannot be reached through a staged-only change.
- The hours test is not flaky under load. I ran 24 copies at once next to 8 busy loops on 20 cores, twice: 0 failures. Elapsed time only grows, and no other stop can come before the budget check.
- Merging the frontier tests dropped no coverage. The removed `a_refused_take_is_printed_verbatim…` had 3 checks (verbatim refusal, title, `story:c` never asked); all 3 are now in `a_refused_take_removes_the_case…:545-583`. The removed `nothing_ready_opens_no_case_through_the_run` is now covered by `a_store_with_nothing_ready…`, which keeps its checks.
- Removing `Frontier::next` is safe. Nothing in `crates/takt`, `crates/takt-cli` or the tests calls it, and the 4 test implementations of `Frontier` only define the required methods. No crate under `~/beyond10x` depends on `b10x-takt`.

**6. Paths written outside the worktree**, all under `<wave-scratch>/scratch/loop-test-gaps/adv1/`:
- `copy/`: the mutated source copy, now restored, plus the probe file `copy/crates/takt/tests/adv1_probe.rs`
- `target/`: the copy's build output, 782M
- `run.rs.orig`, `mutate.py`, `mutate.out`, `mutant-*.log` (8 files), `probe_tail.rs`, `load.sh`, `load/` (24 logs), `suite.log`

```findings
[
  {"file": "crates/takt/tests/loop_gaps.rs", "line": 321, "category": "mutant", "severity": "warning", "verdict": "CONFIRMED", "origin": "introduced", "message": "the hours test bounds the stop only from above, so mutants that stop the run before its first step (run.rs:609 >= to <, h*3600 to h*60) stay green; loop_gaps_adv1.rs adv1_the_time_budget_does_not_stop_a_run_before_it_is_spent kills both"},
  {"file": "crates/takt/tests/loop_gaps.rs", "line": 315, "category": "mutant", "severity": "warning", "verdict": "CONFIRMED", "origin": "introduced", "message": "one 600 ms step against a 500 ms budget cannot tell elapsed-since-open from elapsed-since-last-step, so a clock restarted per step (before run.rs:883) stays green; adv1_the_time_budget_counts_every_step_not_only_the_last kills it"},
  {"file": "crates/takt/tests/loop_gaps.rs", "line": 287, "category": "mutant", "severity": "warning", "verdict": "CONFIRMED", "origin": "introduced", "message": "the token budget is tested only at exact equality, so run.rs:614 >= to == stays green and the Some(201) half is never compared above 100; adv1_the_token_budget_stops_a_run_whose_step_overshoots_it kills it"}
]
```
