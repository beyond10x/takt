---
format: aep.planning-md/3
id: story:pr-opens-when-change-is-done
kind: story
status: draft
title: The pull request opens once the change is complete, so CI runs once
summary: pr.open waits for the complete change; scenario 1 ran CI twice and probed the forge at the seed
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
scope:
- confidence: inferred
  path: crates/takt/src/run.rs
- confidence: cited
  path: protocols/story-delivery/1.yaml
revision: 4
---
# Story: pr-opens-when-change-is-done

## Outcome

A story run opens its pull request once the change is complete, so continuous integration runs once per accepted unit, not once per intermediate revision, and no forge step is taken at the seed before any change exists.

## Context

Measured by the operator's timed takt arm of the baseline comparison, scenario 1, takt at 0a776d2, run T-1 (one of three with the same shape):

- steps 3–4: `pr.open` and `ci.read` run at the seed revision, before any change: "has no commit beyond trial/8; nothing pushed, no pull request", then "no pull request is open for the case".
- step 8: `pr.open` opens pull request 10 at the `implementation.edit` revision, and steps 9–10 wait for its CI.
- step 11: `tests.write` commits the story's tests; steps 14–17 push again, wait for the pull request's head to move and for CI a second time.

`pr.open`'s precondition is only `tests.pass` and `gate.green` (`protocols/story-delivery/1.yaml:124-127`), which hold at the seed and after the implementation alone, and the loop takes a due deterministic step before the next judgement step. The free-running agents of the same comparison opened their pull request once, after implementation and tests. This is one of the two overheads behind takt's 2.1x wall time (133 s against 64 s for an isolated free agent); the other is story:no-model-step-for-approval.

## Expected before

On takt at 0a776d2, holds; after the fix, fails:

- A scenario 1 run (`takt scenario run --repo <clone> 1 --test-cmd ./check.sh --hours 1`) prints `pr.open` 3 or more times and `ci.read` reports `pass` for 2 different revisions before `ApprovalRequired`.

## Scope

- `protocols/story-delivery/1.yaml`: `pr.open`'s precondition (cited)
- `crates/takt/src/run.rs`: when a due deterministic step is taken against an admissible judgement action that would move the revision (inferred)

## Acceptance

On scenario 1, a run reaches `ApprovalRequired` with exactly one CI result recorded, at the revision the grant is given at, and no `pr.open` before the first implementation commit; the run's wall time and steps are recorded in docs/metrics.
