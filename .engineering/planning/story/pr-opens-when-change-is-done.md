---
format: aep.planning-md/3
id: story:pr-opens-when-change-is-done
kind: story
status: implemented
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
revision: 10
transitions:
- {from: "draft", to: "proposed", at: "2026-10-18T18:27:20Z", actor: "human:timo", revision: 6}
- {from: "proposed", to: "active", at: "2026-10-18T18:27:20Z", actor: "human:timo", revision: 7}
- {from: "active", to: "implemented", at: "2026-10-19T04:04:07Z", actor: "human:timo", revision: 9, decided_on: {"recorded":{"test_result":1,"review_outcome":2}}}
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

## The evidence rule

The evidence rule in `AGENTS.md` § Rules gains its one exception (decided 2026-10-18, D35): choice records written by takt (producer `takt.choice`, first `handover` from story:pr-opens-when-change-is-done) attest which action the harness chose; they gate an action, never make a claim about the work true.

## Landed (wave 12)

Read from `git diff --name-only 7e9b2ef^1 7e9b2ef` (the unit's merge into the wave branch) at the close of wave 12 (wave branch head 63a9d02). pr.open once per handed-over change: change.handover is a choice record, change.read records ahead or none, and pr.open waits on ahead. Shared merge with story:no-model-step-for-approval and story:no-inspect-for-stateless-harness.

- `crates/takt-cli/src/main.rs`
- `crates/takt-cli/tests/merge_cli.rs`
- `crates/takt-cli/tests/protocol_defaults_cli.rs`
- `crates/takt-cli/tests/scenario_run.rs`
- `crates/takt-cli/tests/trial_fetch_adv1.rs`
- `crates/takt/src/git.rs`
- `crates/takt/src/harness.rs`
- `crates/takt/src/run.rs`
- `crates/takt/src/validator.rs`
- `crates/takt/tests/approval_record.rs`
- `crates/takt/tests/approval_record_adv1.rs`
- `crates/takt/tests/approval_record_adv2.rs`
- `crates/takt/tests/approval_riders.rs`
- `crates/takt/tests/approval_riders_adv1.rs`
- `crates/takt/tests/approval_riders_adv2.rs`
- `crates/takt/tests/ci_pending_waits.rs`
- `crates/takt/tests/ci_pending_waits_adv1.rs`
- `crates/takt/tests/efficiency_adv1.rs`
- `crates/takt/tests/efficiency_adv2.rs`
- `crates/takt/tests/forge.rs`
- `crates/takt/tests/forge_scan_policy.rs`
- `crates/takt/tests/integration_adv1.rs`
- `crates/takt/tests/loop_gaps.rs`
- `crates/takt/tests/merge_under_authority.rs`
- `crates/takt/tests/no_inspect_for_stateless_harness.rs`
- `crates/takt/tests/no_model_step.rs`
- `crates/takt/tests/pr_opens_when_done.rs`
- `crates/takt/tests/records_reach_base.rs`
- `crates/takt/tests/records_reach_base_adv1.rs`
- `crates/takt/tests/records_reach_base_adv2.rs`
- `crates/takt/tests/reviewer_efficiency_probe.rs`
- `crates/takt/tests/reviewer_probe3.rs`
- `crates/takt/tests/run_record.rs`
- `crates/takt/tests/scorecard_adv1.rs`
- `crates/takt/tests/story_delivery.rs`
- `crates/takt/tests/tree_hygiene.rs`
- `crates/takt/tests/validator_confinement.rs`
- `ess/domains/takt.citations.tsv`
- `ess/domains/takt.yaml`
- `protocols/story-delivery/1.yaml`
