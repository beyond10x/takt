---
format: aep.planning-md/3
id: story:run-record-and-story-evidence
kind: story
status: implemented
title: Record each run as events and as evidence on the story in the planning store
summary: events.jsonl per case; test_result evidence on the story; implemented on a record
relations:
- decomposes: epic:composable-agentic-work
- depends_on: story:frontier-next-ready-story
- depends_on: story:merge-under-authority
- serves: vision:safe-agentic-coding
scope:
- confidence: inferred
  path: crates/takt/src/events.rs
- confidence: inferred
  path: crates/takt/src/frontier.rs
- confidence: inferred
  path: crates/takt/src/lib.rs
- confidence: cited
  path: crates/takt/src/run.rs
- confidence: inferred
  path: crates/takt/tests/run_record.rs
revision: 10
transitions:
- {from: "draft", to: "proposed", at: "2026-10-18T14:01:46Z", actor: "human:timo", revision: 7, decided_on: {"recorded":{"review_outcome":2}}}
- {from: "proposed", to: "active", at: "2026-10-18T14:01:46Z", actor: "human:timo", revision: 8, decided_on: {"recorded":{"review_outcome":2}}}
- {from: "active", to: "implemented", at: "2026-10-18T15:23:35Z", actor: "human:timo", revision: 10, decided_on: {"recorded":{"test_result":1,"review_outcome":4}}}
---
# Story: run-record-and-story-evidence

## Outcome

After a run, a person reads from one JSONL file what happened step by step, and the planning store holds a test-result evidence record on the story the run worked, so the story moves to `implemented` on a record rather than on an assertion.

## Context

`docs/design/takt.md` § 3: `record := run events (JSONL), every Canon decision, AEP evidence on the unit`; § 4 M2: "AEP evidence written". Today every decision is written under the case (`crates/takt/src/case.rs:193-196`), the loop prints human lines to `out` (`crates/takt/src/run.rs:138-262`) that nothing can read back, and nothing is written to the store.

This story: `events.jsonl` in the case directory, one object per event with the instant and the step number: run opened, decision (its sequence number and file), step (validator or harness, the action, tokens when reported), effect (paths committed, the new revision), evidence (record id and path), refusal, stop. And the store: when a validator writes a `test_result` pass record on a story unit, the run records `aep plan artifact evidence <story> --kind test_result --source takt --ref <record path> --at <the record's observed_at>`; never an instant the run chose, and never a record the run did not observe. When the case terminates `accepted`, the run moves the story `active → implemented` through `aep plan artifact move`, which the lifecycle allows once at least one `test_result` record exists, and relays a refusal verbatim instead of retrying. `AGENTS.md:14`: the store changes only through `aep plan artifact` verbs.

## Domain relations

- Story → evidence record (planning store): one-to-many, appended and never edited. Inferred from aep 0.74.0 `aep plan artifact evidence --help`.
- Story → `implemented`: needs at least one `test_result` record. Inferred from aep `artifacts/lifecycles/story.yaml`, `requires.implemented`.
- Case → Decision: one-to-many, owned by the case, one per evaluation. Inferred from `crates/takt/src/case.rs:175-198`.

## Scope

- new: `crates/takt/src/events.rs`
- `crates/takt/src/lib.rs` (the `events` module registered)
- `crates/takt/src/run.rs` (events emitted beside the printed lines)
- `crates/takt/src/frontier.rs` (`evidence` for `test_result` and `move` on the store adapter, beside the `approval` evidence story:merge-under-authority writes)
- new: `crates/takt/tests/run_record.rs`

## Ordering

- depends_on story:frontier-next-ready-story: both edit `crates/takt/src/run.rs`, `crates/takt/src/frontier.rs` and `crates/takt/src/lib.rs`.
- depends_on story:merge-under-authority: both edit `crates/takt/src/run.rs` and `crates/takt/src/frontier.rs`.

## Acceptance

After a run on a story unit in which the tests validator wrote a pass record, the case directory holds `events.jsonl` with one line per evaluation, step, evidence record and stop.

## Checks

- `aep plan artifact history <story>` shows one `test_result` evidence record whose instant equals that record's `observed_at` and whose source names takt.
- A story whose case terminated `accepted` is at status `implemented` afterwards.

## Out of Scope

Aggregating many runs into numbers (story:week-of-daily-runs); evidence kinds other than `test_result` (the `approval` record is story:merge-under-authority); writing to any store but the planning store of the repository the run is on.

## Ambiguities

- Whether takt or a person makes the move to `implemented`: `inferable`, the planning skill § 4 says a move is made when the store holds what the rung requires, and the lifecycle (`artifacts/lifecycles/story.yaml`) names the record; the operator can revert a move.

## ## Note (2026-10-18)

## Note (2026-10-18)

A live trial of `takt run` on this repository took this story as its unit and the harness drafted
an implementation (`events.rs`, `tests/run_record.rs` with three cases, the store adapter's
`test_result` evidence and move to `implemented`) in one judgement step; the loop committed it on
the case branch `takt/story-run-record-and-story-evidence-20261018T090034Z`, kept locally, outside
the wave process and without an adversary pass. It is reading material for the wave that delivers
this story, not a unit.

## Landed (wave 09)

Read from `git diff --stat 8ce2019 fe2e092` at the close of wave 09 (wave branch head 1425689). Records at `accepted` on the case branch; a judgement step touching `.engineering/` outside `cases/` is refused. Carrying the store records to the base is story:records-reach-base.

- `crates/takt-cli/src/main.rs`
- `crates/takt-cli/tests/run_record_cli.rs`
- `crates/takt/src/events.rs`
- `crates/takt/src/frontier.rs`
- `crates/takt/src/lib.rs`
- `crates/takt/src/run.rs`
- `crates/takt/tests/run_record.rs`
- `crates/takt/tests/run_record_adv1.rs`
- `crates/takt/tests/run_record_adv2.rs`
- `crates/takt/tests/validator_confinement.rs`
- `ess/domains/takt.yaml`
