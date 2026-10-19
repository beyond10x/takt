# Wave 12: improvement turns 2 and 3, the loop scorecard, one pull request per handed-over change, scan before push

Skill: `aep:implementing` 0.23.1, wave mode. Stage: **closed**. Approval: the planner role named these units (the wave 12 queue, the RSI order, and the carry-over of the efficiency unit from wave 11); each judge-surface merge needs the planner's grant of its exact commit, on a reviewer verdict. Mode: interactive, three implementors in parallel.

## Units

| unit | serves | acceptance (condensed) | judge surface |
|---|---|---|---|
| `story:case-id-is-a-valid-branch-name` (improvement turn 2) | `vision:safe-agentic-coding` | `takt run --case-id 'a~b'` and the other ids git refuses exit 1 with takt's message before anything is written; `scripts/check-finding` prints `not_reproduced` at the fix | yes |
| `story:hours-bounds-a-harness-step` (improvement turn 3) | `vision:safe-agentic-coding` | under `--hours 0.001` a 20 s harness step is killed within the budget plus 10 s and its changes discarded | yes |
| `story:loop-scorecard` (moved to wave 13) | `vision:safe-agentic-coding` | over three recorded turns `loop.measure` writes a `loop_measurement` whose figures each name their case ids | yes |
| `story:pr-opens-when-change-is-done`, `story:no-model-step-for-approval`, `story:no-inspect-for-stateless-harness` (carried over from wave 11) | `vision:safe-agentic-coding` | `pr.open` once per handed-over change and CI once; no harness call when only an ungranted capability remains; no read step for a stateless harness | yes |
| `story:scan-before-push` (moved to wave 13) | `vision:safe-agentic-coding` | `pr.open` scans commit messages and the diff before anything is pushed; a refused scan pushes nothing | yes |
| `story:bench-nouns-in-ess` | `vision:safe-agentic-coding` | `ess specify validate` passes with BenchTask, BenchRun and BenchArm declared, each relation to a bench entity named or `UNMAPPED:` | yes (ESS only; merged at 21030e8 before its grant in error, reviewed after) |
| `story:wave11-review-follow-ups` | `vision:safe-agentic-coding` | the low findings of the wave 11 reviews and the stale step-guard texts | yes |

## Selection path

The planner's wave 12 queue (case-id-is-a-valid-branch-name and hours-bounds-a-harness-step as improvement turns 2 and 3, each its own unit; loop-scorecard on the RSI path; wave11-review-follow-ups), the carry-over of the efficiency unit from wave 11 on the planner's decision, and two disjoint units started when the planner asked for three implementors in parallel (scan-before-push, bench-nouns-in-ess). case-id, loop-scorecard, scan-before-push and bench-nouns started early on their own branches; this branch opens from local main at be1731d, after wave 11 landed, and each unit merges it before its grant.

loop-scorecard's Ordering gates it past draft on three recorded improvement turns; the planner placed it in wave 12 to be built now, with turns 2 and 3 recorded in this wave.

case-id-is-a-valid-branch-name, hours-bounds-a-harness-step and wave11-review-follow-ups were filed without a `serves` relation; this opening relates them to `vision:safe-agentic-coding`, as every other story of the epic.

## Left out

- `story:takt-tui-crate`: its dependencies story:tui-ui-document and story:takt-serve-read-only are draft; the chain starts at story:tui-ui-document in the next free slot.
- `story:run-records-harness-model`: `events.rs` and `harness.rs`, shared with hours-bounds-a-harness-step and the efficiency unit.
- `story:signal-releases-the-case`, `story:forge-preflight`, `story:untracked-cargo-lock-remedy`, `story:abandon-a-case`: wave 13 by collision on `run.rs`, `case.rs` and `forge.rs`.

## Pre-flight

| check | value |
|---|---|
| tree | `main` at be1731d; `task check` exit 0 at 3fe3f21 (627 passed plus `version_cli` passing on a rerun at be1731d; 462 of 462 citations) |
| model budget | not stated; N = 3 |

## The commits this wave makes

One opening commit (this page, the stories to `active`), each unit's commits on its own branch, a merge of this branch into each unit, the merges of the units into this branch on their grants, the closing planning-store commit, and the merge into local main. No push, tag or release.

## Log

- 2026-10-18: wave opened from main at be1731d.
- 2026-10-18: bench-nouns-in-ess merged at 21030e8 without a grant, in error: ess/ is a judge surface under AGENTS.md (every path but docs/**, README.md, CHANGELOG.md). The planner's reviewer reviews it after the merge; a needs-revision verdict lands as its own unit before the wave closes.
- 2026-10-19: the planner's close rule: wave 12 closes with the units merged so far (case-id, bench-nouns, the three wave 11 carry-overs, hours) and the follow-ups. loop-scorecard, scan-before-push and improvement turn 4 (script imports, started on this branch) move to wave 13 and merge there on their grants.
- 2026-10-19: 5990815 and 3c8746a name the benchmark instrument in `ess/` without its repository; 5e6f115, 9dea7ba and a931f5d give the hour-budget premises their own budgets, so they hold under load.
- 2026-10-19: closed. main merged in at 3352534 (8d7c194 passes the call report on the step discarded at the run's end, which the merge left with two of three fields); `task check` at 8d7c194 exit 0, 814 passed, 0 failed, 6 ignored, citations 578 of 578; a timing case of `hours_bounds_a_harness_step_adv1` failed once under a load average of 87 and passed alone, 5 of 5. Seven review-results recorded, Landed sections written, and case-id-is-a-valid-branch-name, bench-nouns-in-ess, pr-opens-when-change-is-done, no-model-step-for-approval, no-inspect-for-stateless-harness, hours-bounds-a-harness-step and wave11-review-follow-ups moved to implemented.
