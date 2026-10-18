# Wave 11: records reach the base, the binary's revision, one pull request per handed-over change

Skill: `aep:implementing` 0.23.1, wave mode. Stage: **running**. Approval: the planner role named these units for wave 11 and accepted their early start from the wave 10 branch; each judge-surface merge needs the planner's grant of its exact commit, on a reviewer verdict. Mode: interactive.

## Units

| unit | serves | acceptance (condensed) | scope | judge surface |
|---|---|---|---|---|
| `story:records-reach-base` | `vision:safe-agentic-coding` | after the merge the base holds the story `implemented` with its `test_result` and approval records, each byte-equal to the record takt kept for the granted revision; anything else above the revision makes the merge merge nothing | `crates/takt/src/case.rs`, `validator.rs`, `run.rs` (`record_accepted`), `frontier.rs` | yes |
| `story:binary-embeds-revision` | `vision:safe-agentic-coding` | `takt --version` and every run's `opened` event carry the revision the binary was built from, `-dirty` with uncommitted changes, `unknown revision` outside a checkout | new `crates/takt-cli/build.rs`, `crates/takt-cli/src/main.rs`, `crates/takt/src/events.rs` | yes |
| `story:pr-opens-when-change-is-done`, `story:no-model-step-for-approval`, `story:no-inspect-for-stateless-harness` (one unit) | `vision:safe-agentic-coding` | `pr.open` once per handed-over change and CI read once; no harness call when only an ungranted capability remains; no read step offered to a stateless harness | `protocols/story-delivery/1.yaml`, the step choice in `crates/takt/src/run.rs`, `crates/takt/src/harness.rs` | yes |

## Selection path

The planner named the wave 11 queue at the close of wave 10's selection: records-reach-base (left out of wave 10 by collision on `case.rs` and `validator.rs`), the efficiency unit (left out by collision on the `run.rs` step choice), and binary-embeds-revision (the next link before story:promotion). The planner decided the three efficiency stories are one unit, as they change which actions a step offers in one function.

All three units started early, from the wave 10 branch at 8b0afc5, while wave 10's last unit was under review; their file assignments kept them off the step guard. This branch opens from local main at 9922f12, after wave 10 landed; each unit merges it before its grant, and the reviewer checks that merge's delta.

## Left out

- `story:scan-before-push`: the pull-request code `records-reach-base` changes; wave 12.
- `story:run-records-harness-model`: `events.rs`, shared with binary-embeds-revision; wave 12.
- `story:judge-guard`: after story:promotion.

## Decisions taken during the units

- pr-opens-when-change-is-done: the planner's option C. The hand-over is protocol data (evidence kind `handover`, claim `change.handed_over`, action `change.handover`, `pr.open`'s precondition); the loop holds nothing and only records a choice kind, read from the protocol without a schema change. `--harness none` takes `change.handover` as it requests approvals.
- records-reach-base: when a case's revision moves from R1 to R2, the R1 records stay in history below R2 and reach the base; only R2's records ride above R2. The merge is not taken while the records for the granted revision are not both written.
- binary-embeds-revision: untracked, non-ignored files under `crates/` and `protocols/` make the revision `-dirty`, an untracked `Cargo.lock` does not; every tracked file is watched (a touch-only rebuild costs about 0.32 s).

## Pre-flight

| check | value |
|---|---|
| tree | `main` at 9922f12; `task check` exit 0 there (596 passed, 4 ignored; 443 of 443 citations) |
| model budget | not stated; N = 2 |

## The commits this wave makes

One opening commit (this page, the stories to `active`), each unit's commits on its own branch, a merge of this branch into each unit, the merges of the units into this branch on their grants, the closing planning-store commit, and the merge into local main. No push, tag or release.

## Log

- 2026-10-18: units started early from 8b0afc5. binary-embeds-revision: a295a53, two adversary passes, corrections dd9bf05 and 85c98bf (the last verified by the coordinator, 538 passed). records-reach-base: d0602ac, two adversary passes, corrections 98813e3 and e46d9d0 (the last verified by the coordinator, 530 passed). Efficiency unit: reworked to the planner's option C, 22a8389 (530 passed), adversary pass 1 running.
- 2026-10-18: wave opened from main at 9922f12.
