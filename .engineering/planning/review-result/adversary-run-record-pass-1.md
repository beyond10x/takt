---
format: aep.planning-md/3
id: review-result:adversary-run-record-pass-1
kind: review-result
status: active
title: Adversary, run-record-and-story-evidence, pass 1
relations:
- reviews: story:run-record-and-story-evidence
revision: 1
---
unit: story:run-record-and-story-evidence, worktree w9-run-record at 44b20a5 (base 8ce2019) plus one untracked test file
verdict: NEEDS-CHANGE
cases: executed 372→377, red 4
origin: introduced 4 / pre-existing 1 / undecided 0
wrote-outside-worktree: 4 logs in the unit's scratch `adv1/`
needs-coordinator: the resumed-run case also passes through the known --case gap at case.rs:1093-1099

**Cases** (`crates/takt/tests/run_record_adv1.rs`)

| case | expected | actual |
|---|---|---|
| `a_judgement_step_cannot_record_a_test_result_on_the_story` (tests fail; an `implementation.edit` step runs `aep plan artifact evidence … --kind test_result --source takt`) | refused, no `test_result` on the story | `committed as 55be959 implementation.edit` with the evidence file; history holds `test_result`, `source: takt` |
| `a_store_write_the_run_made_is_named_in_the_record` (stdout closed after the write) | store `test_result` count equals `store_evidence` lines; last line `stop` or `error` | `the store holds 1 test_result the record does not name; run: Err(the reader went away)` |
| `an_accepted_case_whose_store_line_was_lost_is_not_recorded_twice` | 1 `test_result` | 2 |
| `a_torn_line_does_not_turn_an_accepted_stop_into_an_unrecorded_error` | `Completed(accepted)`, last line `stop` or `error` | `Err(read line 1 of …/events.jsonl … expected , or })`; last line `decision` |
| `a_hanging_or_missing_aep_leaves_the_stop_and_ends_the_record` | under 20 s, `accepted`, 2 takt refusals, last line `stop`, tree clean | green |

**Gate.** fmt 0, clippy 0, `cargo test --workspace --locked --no-fail-fast` 101 (373 passed, 4 failed, all in `run_record_adv1`), ess 0.

**Attacked, not broken.** Writing into the case directory, appending or editing `events.jsonl`, planting a symlink: the content snapshot refuses the step and puts it back; lines are written after `undo_step`. No validator's `writes()` includes `evidence/` or `events.jsonl`. A record at another revision is refused (`subject_revision`). Path from the kernel's evidence line, instant from `observed_at`. aep children built like the git, validator and harness children (`without_credentials`, `run_child` limits); hang and missing binary bounded. `write_on_case_branch` checks the branch first. serde escapes JSON. The `error` text equals what `finish` prints; no child stderr found carrying a credential. Scenario run records on the scenario repository's tree. All 32 new `read from:` citations hold.

```findings
[
  {"file": "crates/takt/src/run.rs", "line": 1480, "category": "acceptance", "severity": "warning", "verdict": "CONFIRMED", "origin": "pre-existing", "message": "A judgement step can write a test_result (source takt) on the story through aep or by hand, takt commits it, and the implemented move then counts it; no step guard covers the planning store."},
  {"file": "crates/takt/src/run.rs", "line": 790, "category": "concurrency", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "The store write is committed before its line is printed and recorded, so a failed print or emit leaves a store record that events.jsonl does not name, and the run ends on neither stop nor error."},
  {"file": "crates/takt/src/run.rs", "line": 771, "category": "concurrency", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "The duplicate check reads only events.jsonl, so a lost store_evidence line makes the next run record test_result on the story a second time."},
  {"file": "crates/takt/src/run.rs", "line": 713, "category": "boundary", "severity": "warning", "verdict": "NEEDS-CHANGE", "origin": "introduced", "message": "An error from close (an unreadable events.jsonl line) is not recorded as an error line and turns every later accepted run of the case into an error."},
  {"file": "crates/takt/src/frontier.rs", "line": 583, "category": "judgement", "severity": "note", "verdict": "CONFIRMED", "origin": "introduced", "message": "takt's test_result is stored with actor human:$USER and no executor, so the store attributes a machine-made record to the operator."},
  {"file": "ess/domains/takt.yaml", "line": 396, "category": "contract-drift", "severity": "note", "verdict": "CONFIRMED", "origin": "introduced", "message": "RunEvent's error variant is declared as String while events.rs writes an object with an error field."}
]
```
