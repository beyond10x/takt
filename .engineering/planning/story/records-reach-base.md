---
format: aep.planning-md/3
id: story:records-reach-base
kind: story
status: draft
title: takt's records for a story reach the base with its merge
summary: test_result and the move to implemented ride to the base with the merge, each file byte-equal to the case's own record
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
- depends_on: story:run-record-and-story-evidence
- depends_on: story:approval-riders-are-grants
scope:
- confidence: cited
  path: crates/takt/src/case.rs
- confidence: inferred
  path: crates/takt/src/validator.rs
revision: 5
---
# Story: records-reach-base

## Outcome

When a story's pull request merges, the planning store at the base holds what takt recorded for it: the story's `test_result` evidence and its move to `implemented`, alongside the approval record. They reach the base only through that merge, and only as records takt itself wrote for the case at the granted revision.

## Context

Observed by the operator on scenario 1, 2026-10-18: after `Completed (accepted)` the story at `origin/trial/0` still read `status: active`, with only the approval record in its evidence directory. story:run-record-and-story-evidence (wave 09) writes the `test_result` record and the move to `implemented` as commits on the case's own branch, touching only the store and the story's evidence directory, and leaves getting them to the base out of scope. story:approval-riders-are-grants (wave 09) admits above the granted revision only commits that add approval records matching a grant, so these commits would be refused by design.

Decided 2026-10-18 on the developer role's proposal: extend the riders the merge admits to (i) a `test_result` record and (ii) one move-to-implemented commit pushed just before the merge, under the condition that every riding file is byte-equal to a record takt wrote in the case directory for that case, bound to the granted revision. A record that merely has the right kind is not enough: the case directory is the source, the commit only carries it.

## Expected before

On takt after wave 09, holds; after the fix, fails:

- A scenario 1 run taken to `Completed (accepted)`: `git show origin/trial/<n>:.engineering/planning/story/remove-a-todo.md` reads `status: active`, and the story's evidence directory at `origin/trial/<n>` holds no `test_result` record.

## Scope

- `crates/takt/src/case.rs`: `approvals_only` and its rider rules (inferred)
- `crates/takt/src/validator.rs`: `pr.open` and `repository.merge` pushing the riders (inferred)
- `crates/takt/tests/`: a riders test (inferred)

## Acceptance

After a story's pull request merges under a grant, the base holds the story at `implemented` with its `test_result` and approval records, each file byte-equal to the record in the case directory; a riding commit with any file that is not byte-equal to such a record, or a second move commit, makes the merge merge nothing.

## Timing and resume

Measured by the developer role while implementing story:run-record-and-story-evidence (wave 09), once:

- story:run-record-and-story-evidence writes the commits `takt: test_result <story>` and `takt: implemented <story>` only when the case stops `accepted`, not when the tests validator writes its record: a store commit at observation time moved `HEAD` off the implementation revision, and `pr.open` then errored (`crates/takt/src/validator.rs:726`).
- So with a forge both commits land on the case branch above the merged head, locally, after the merge. Riding with the pull request needs them written before the merge: the `test_result` record at the granted revision, and the move to `implemented` as the last rider, pushed just before the merge. This story decides that order.
- A gap in the same rider rule: `takt run --case <an accepted case>` goes through `CaseDir::resume`, which moves the implementation revision onto those store commits because `approvals_only` does not recognise them (`crates/takt/src/case.rs:1093-1099`), so `pr.open` becomes due again. Only an explicit `--case` reaches it. The rider rule this story adds must be the one `CaseDir::resume` reads too, so an accepted case resumed by id stays at its granted revision and nothing becomes due again.
