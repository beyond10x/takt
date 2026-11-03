---
format: aep.planning-md/3
id: review-result:w50-adv-a-1
kind: review-result
status: active
title: Wave 50 adversary pass 1 on story:fenced-forge-dispatch
relations:
- reviews: story:fenced-forge-dispatch
revision: 1
---
```
unit: story:fenced-forge-dispatch, tree takt-w50-a at 28f467ab9 (unit commit 06f3e88ff) plus my uncommitted test file
verdict: red
cases: executed 7→12, red 2
origin: introduced 2, pre-existing 2, undecided 0
wrote-outside-worktree: none
needs-coordinator: yes
```
The unit's fence holds a lost merge answer back for exactly one observation. Two of the five new cases are red: a merge that is still landing is sent again, and so is one cut off by a crash.

**Cases added** (`crates/takt/tests/w50_adv_forge_write.rs`). They use a wrapper over `Memory` that reports an applied merge as still open for N reads, as the glab adapter reports GitLab's `locked` state (`gitlab.rs:591`). Each observation reads the pull request twice: the view, then a second read right before the merge (`validator.rs:5894`).

| case | asserts | now |
|---|---|---|
| `…landing_one_observation_late_is_sent_once` (control, lag 2) | 1 merge call, outcomes unknown, not_attempted, not_attempted | green |
| `…landing_two_observations_late_is_sent_once` (lag 4) | 1 merge call | red |
| `…merge_cut_off_before_its_line_is_sent_once` (panic after send, lag 2) | 1 merge call | red |
| `…pr_open_answer_lost_records_unknown_with_the_number_found` | pr.open line is unknown, #1, head | green |
| `…unknown_merge_then_closed_sends_nothing_and_records_closed` | 1 call, `closed`, outcomes unknown, not_attempted | green |

**Suite:**
- `fenced_forge_dispatch`: 7 passed.
- `w50_adv_forge_write`: 3 passed, 2 failed.

**Attacked and could not break:**
- A lost `pr.open` answer: the pull request found is taken, its line says unknown, and no second open is sent.
- An unknown merge, then the pull request is closed: no merge is sent and `closed` is recorded.
- An unknown merge, then the head moves: no merge is sent.
- `forge_write` on the wrong step: it reaches `Step` lines only through `Events::writing`, and the validator takes it each step.
- Old and new readers: every reader parses lines as untyped JSON.
- Each adapter's classes: none claims `refused` or `not_attempted` after the request had gone out.

**Not attacked:** the GitLab and connectors classes, through tests.

```findings
[
{"file": "crates/takt/src/validator.rs", "line": 4707, "category": "acceptance", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "unknown_merge_of reads only the newest forge_write line, so the not_attempted wait step clears the unknown and the next observation re-sends a merge still landing (glab reports locked as open) (test: crates/takt/tests/w50_adv_forge_write.rs::w50_adv_a_lost_merge_answer_landing_two_observations_late_is_sent_once)"},
{"file": "crates/takt/src/validator.rs", "line": 5939, "category": "concurrency", "severity": "warning", "verdict": "CONFIRMED", "origin": "introduced", "message": "nothing is recorded before the merge is sent, so a crash between send and the step line leaves no unknown and the next run sends the merge again at once (test: crates/takt/tests/w50_adv_forge_write.rs::w50_adv_a_merge_cut_off_before_its_line_is_sent_once)"},
{"file": "crates/takt/src/validator.rs", "line": 5894, "category": "judgement", "severity": "note", "verdict": "CONFIRMED", "origin": "pre-existing", "message": "the read right before the merge is checked only for its base, so a pull request it reports merged or closed is sent a merge anyway; seen at runtime in the first crash-case run"},
{"file": "crates/takt/src/validator.rs", "line": 6010, "category": "judgement", "severity": "note", "verdict": "CONFIRMED", "origin": "pre-existing", "message": "a merge answered applied whose pull request still reads open is sent again by the next observation; the new applied record is not consulted"}
]
```
