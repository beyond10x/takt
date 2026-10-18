---
format: aep.planning-md/3
id: story:ci-pending-waits
kind: story
status: active
title: A story run waits for pending checks instead of stopping on NoProgress
summary: pending CI or a lagging pull-request head is polled within the budget; a run out of budget stops as waiting, not NoProgress
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
scope:
- confidence: cited
  path: crates/takt/src/run.rs
- confidence: cited
  path: crates/takt/src/validator.rs
- confidence: inferred
  path: crates/takt/tests/ci_pending_waits.rs
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-10-18T13:07:06Z", actor: "human:timo", revision: 5}
- {from: "proposed", to: "active", at: "2026-10-18T13:07:06Z", actor: "human:timo", revision: 6}
---
# Story: ci-pending-waits

## Outcome

A story run whose pull request's checks are still running waits for them, inside its budget, and then records the result, instead of stopping with `NoProgress`. A run that cannot wait stops with a stop that says it waits on the forge, and the next run resumes the case.

## Context

Found by the operator's live trial on the scratch repository (scenario 1, `story:remove-a-todo`), takt at 92b8cf3, 2026-10-18, run 4 of case `story-remove-a-todo-20261018T120340Z`:

- `pr.open` pushed the case branch and opened pull request #1; one step later `ci.read` reported `checks on pull request #1 at c69a8f97 through bot: pending; no record, the claim stays unknown`.
- After a `tests.write` commit, `pr.open` reported `pull request #1 has the head ... at c69a8f97, not the case's branch at the revision; no record`: GitHub had not yet moved the pull request's head to the pushed commit. `ci.read` at the new revision read `pending` again.
- The harness then took `repository.inspect` with no change, and the run stopped: `stopped: NoProgress; steps 8, tokens 75614`, exit 3.
- Run 5, a few minutes later and unchanged otherwise, recorded `pull_request open` and `ci_result pass` at the same revision and stopped at `ApprovalRequired (repository.merge)`, exit 4.

Mechanism, read in code: the CI validator writes no record for pending checks (`crates/takt/src/validator.rs:896-898`); the run marks a validator that observed without a record as not due again at that revision (`crates/takt/src/run.rs:674`, `:834`, `:97`); the next evaluation repeats the idle state and the loop returns `Stop::NoProgress` (`run.rs:683-685`). Nothing waits for the forge, so every story run whose CI takes longer than one judgement step ends in exit 3, and an unattended run never reaches the merge approval.

## Expected before

On takt at 92b8cf3, holds; after the fix, fails:

- Scenario 1, with the scratch repository's CI taking longer than one step: `takt run --repo <scratch> --test-cmd ./check.sh --forge bot --hours 1` exits 3 and its last line starts `stopped: NoProgress` while the case's latest `ci.read` effect ends `pending; no record, the claim stays unknown`.
- A test with a stub forge whose checks report pending for the first N reads and pass afterwards: the run stops with `NoProgress` before the N+1th read.

## Scope

- `crates/takt/src/run.rs`: the idle check and `observed_without_record` (cited)
- `crates/takt/src/validator.rs`: the CI and pull-request validators' pending branches (cited)
- `crates/takt/tests/`: a stub-forge wait test (inferred)

## Acceptance

With a stub forge whose checks stay pending for N reads, a run with budget left polls the checks at a bounded interval, records `ci_result` when they finish and reaches `ApprovalRequired`; when the budget ends first, the run stops with a stop and exit code that say it waits on the forge, distinct from `NoProgress`, and the next run resumes the case. A pull request whose head lags a push is read again after the same bounded wait.
