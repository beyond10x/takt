---
format: aep.planning-md/3
id: review-result:adversary-run-record-pass-2
kind: review-result
status: active
title: Adversary, run-record-and-story-evidence, pass 2
relations:
- reviews: story:run-record-and-story-evidence
revision: 1
---
unit: story:run-record-and-story-evidence, worktree w9-run-record at b0bfaaa (base 8ce2019) plus one untracked test file
verdict: NEEDS-CHANGE
cases: executed 380→390, red 7
origin: introduced 4 / pre-existing 3 / undecided 0
wrote-outside-worktree: 7 paths in the unit's scratch `adv2/`
needs-coordinator: whether the guard covers all of `.engineering/` except `cases/` (decided: yes)

**Cases** (`crates/takt/tests/run_record_adv2.rs`, each run alone first)

| case | expected | actual |
|---|---|---|
| `a_store_directory_planted_as_a_link_is_refused` | refused, no `test_result` | committed; aep reads the forged record through the link |
| `a_step_that_moves_the_store_elsewhere_is_refused` | refused | `.engineering/plan.sqlite3` and `project.yaml` committed; story `implemented` there with a `test_result` |
| `a_mode_change_in_the_store_is_refused` | mode stays 100644 | 100755 committed |
| `a_fifo_in_the_store_does_not_hang_the_run` | never blocks | blocked; released after 10.1 s by the test |
| `an_unreadable_file_in_the_store_leaves_nothing_of_the_step` | tree clean after the error | `M check.txt`, `?? .engineering/evidence/` |
| `a_store_write_whose_line_cannot_be_written_is_taken_back` | taken back | green |
| `a_step_that_cannot_be_printed_is_discarded_with_its_commit` | HEAD restored, last line `error` | green |
| `a_harness_that_panics_still_ends_the_record` | last line `stop`/`error`, no `run.lock` | ends on `decision`; `run.lock` left |
| `a_torn_repair_does_not_make_the_record_unreadable` | `read()` Ok | refused |
| `a_store_of_a_thousand_files_adds_little_to_a_step` | under 2 s more | green (424 ms vs 1.08 s for the whole run) |

The first three reproduce at 8ce2019 with `record_on` removed (3 of 3), so they are pre-existing; the base has no store snapshot.

**Gate.** fmt 0, clippy 0, tests 101 (383 passed, 7 failed, all in `run_record_adv2`), ess 0. No existing test file edited since 8ce2019.

**Attacked, not broken.** A link planted outside the store pointing in, a rename into the store, a new store directory, a step's own `git commit`: caught and put back. Both-or-neither holds: the reset runs only on the checked branch after a clean check; untracked `cases/` survives. A step whose lines fail to print is discarded whether it committed or not. Printed lines keep text and order. A step cannot forge a `takt: …` subject. A middle unparsable line is an error. Only `run()` writes `events.jsonl`, under a `create_new` lock. The validator path refuses store writes. RunError and RunSkipped and their 5 citations hold.

```findings
[
  {"file": "crates/takt/src/run.rs", "line": 165, "category": "acceptance", "severity": "blocker", "verdict": "NEEDS-CHANGE", "origin": "pre-existing", "message": "A step that plants a missing .engineering/evidence as a link to forged records passes the store guard, is committed, and aep reads the test_result through the link."},
  {"file": "crates/takt/src/frontier.rs", "line": 335, "category": "acceptance", "severity": "blocker", "verdict": "NEEDS-CHANGE", "origin": "pre-existing", "message": "STORE_DIRS leaves out .engineering/project.yaml, so a step moves the store to a SQLite file it fills, with the story implemented and a test_result, and is committed."},
  {"file": "crates/takt/src/run.rs", "line": 136, "category": "boundary", "severity": "note", "verdict": "CONFIRMED", "origin": "pre-existing", "message": "The snapshot keeps bytes but not modes, so a step's mode change on a store file is committed."},
  {"file": "crates/takt/src/run.rs", "line": 158, "category": "boundary", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "The store snapshot reads a FIFO a step planted and blocks the run with no end."},
  {"file": "crates/takt/src/run.rs", "line": 1378, "category": "boundary", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "A store file the run cannot read fails the snapshot after the step, and the run returns without undo_step, leaving the step's change in the tree."},
  {"file": "crates/takt/src/run.rs", "line": 722, "category": "contract-drift", "severity": "note", "verdict": "INFEASIBLE", "origin": "introduced", "message": "A harness that panics leaves a record ending on decision and leaves run.lock, against the promise that every record ends on stop or error."},
  {"file": "crates/takt/src/events.rs", "line": 232, "category": "concurrency", "severity": "note", "verdict": "INFEASIBLE", "origin": "introduced", "message": "A repair write cut short too leaves two unreadable lines in a row, and read() then refuses the record for good."}
]
```
