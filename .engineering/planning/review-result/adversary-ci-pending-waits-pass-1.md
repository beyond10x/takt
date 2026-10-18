---
format: aep.planning-md/3
id: review-result:adversary-ci-pending-waits-pass-1
kind: review-result
status: active
title: Adversary, ci-pending-waits, pass 1
relations:
- reviews: story:ci-pending-waits
revision: 1
---
unit: story:ci-pending-waits, worktree w8-ci-pending-waits at 200e15d (base 4cbc096) plus one untracked test file
verdict: NEEDS-CHANGE
cases: executed 332→336, red 4
origin: introduced 5 / pre-existing 0 / undecided 0
wrote-outside-worktree: 8 paths, all in the unit's scratch `adv1/`
needs-coordinator: none

**1. Diff.** `git --no-pager diff --stat` is empty; `git status --short` shows `?? crates/takt/tests/ci_pending_waits_adv1.rs`. No source file touched.

**2. Cases added** (alone: `cargo test -p b10x-takt --locked --test ci_pending_waits_adv1`, 0 passed, 4 failed)

| test | expected | actual |
|---|---|---|
| `adv1_an_infinite_hour_budget_waits_without_a_panic_and_lets_go_of_the_case` | `Ok(Waiting { on: ["ci.read"], budget: Some(Iterations(4)) })`, `run.lock` removed | `Err("the run panicked: cannot convert float seconds to Duration: value is either too big or NaN; run.lock left behind: true")` |
| `adv1_a_nan_hour_budget_never_polls_the_forge_without_a_pause_or_an_end` | stops by itself after at most 2 checks reads | `41 checks reads in 959.577284ms (the poll is 250 ms)`; ended only because the stub forge errors after 40 reads |
| `adv1_a_pending_check_refusal_through_the_bot_route_is_waited_on_whatever_the_paths_say` | a 405 for a pending required check, as the bot adapter reports it, is waited on | `a tree at failover-service: an error, not waited on`; likewise for a story `fix-a-failing-test` and a repository `o/fail2ban` |
| `adv1_a_pending_required_check_whose_name_holds_fail_is_still_waited_on` | waited on | `read as a failing check, not waited on: ["405 Required status check \"fail-fast\" is expected", "405 Required status check \"no-failures\" is in progress"]` |

**3. Gate.** fmt exit 0; clippy exit 0; `cargo test --workspace --locked --no-fail-fast` exit 101, only `ci_pending_waits_adv1` fails (0 passed; 4 failed); `ci_pending_waits` 9, `approval_record_adv1` 4, `approval_record_adv2` 5, `forge` 35 pass.

**4. Attacked, not broken.** Steps and hours bound the wait; a run with no budget stops Waiting at once; nothing sleeps after the budget is spent. The harness is reached only once nothing waited on is admissible. No evidence written on the waiting paths of `pr.open`, `ci.read`, `repository.merge`; guards unchanged. A head that never arrives ends `Ok(Waiting { on: ["ci.read"], budget: Some(Iterations(10)) })`. A checks read for the old head is not reachable: `ci.read` requires the pull request to show the pushed commit first, and `gh pr checks` reads the current head. Exit 6 mapping right; resume through `Run::resume`. The flipped tests each assert the exact Waiting stop; none weaker.

Smaller points: each poll writes about 2 decision files; `pr.open` pushes again on every poll, so a dropped ssh connection ends a waiting run with exit 5; help says `stopped: Waiting:`, the run prints `stopped: Waiting (…)`.

```findings
[
  {"file": "crates/takt/src/run.rs", "line": 664, "category": "boundary", "severity": "warning", "verdict": "CONFIRMED", "origin": "introduced", "message": "an infinite or huge --hours makes Run::pause panic in Duration::from_secs_f64 at the first wait, and the run leaves run.lock behind so the next run is refused"},
  {"file": "crates/takt/src/run.rs", "line": 643, "category": "boundary", "severity": "note", "verdict": "INFEASIBLE", "origin": "introduced", "message": "a NaN --hours is never spent and its pause is cut to zero, so a waiting run polls the forge with no pause and no end"},
  {"file": "crates/takt/src/forge.rs", "line": 91, "category": "acceptance", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "refused_for_a_pending_check reads the whole error chain, including the bot route's paths, case id and repository, so any fail there turns a pending-check refusal into an error"},
  {"file": "crates/takt/src/forge.rs", "line": 93, "category": "boundary", "severity": "note", "verdict": "CONFIRMED", "origin": "introduced", "message": "a pending required check whose name contains fail is treated as a failing check and ends the run in an error"},
  {"file": "ess/domains/takt.yaml", "line": 320, "category": "contract-drift", "severity": "note", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "takt.work.Stop has no waiting variant and its exit-code comment omits 6, and unresolved read-from citations rise from 5 to 132 with 6 needing hand re-pointing"}
]
```
