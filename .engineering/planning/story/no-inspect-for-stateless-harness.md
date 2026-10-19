---
format: aep.planning-md/3
id: story:no-inspect-for-stateless-harness
kind: story
status: implemented
title: A stateless harness is not offered repository.inspect as a way forward
summary: read-only actions are not offered to a memoryless harness, or the prompt says they end the run with NoProgress
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
scope:
- confidence: cited
  path: crates/takt/src/harness.rs
- confidence: cited
  path: crates/takt/src/run.rs
- confidence: inferred
  path: crates/takt/tests/no_inspect_for_stateless_harness.rs
revision: 9
transitions:
- {from: "draft", to: "proposed", at: "2026-10-18T18:27:21Z", actor: "human:timo", revision: 5}
- {from: "proposed", to: "active", at: "2026-10-18T18:27:21Z", actor: "human:timo", revision: 6}
- {from: "active", to: "implemented", at: "2026-10-19T04:04:07Z", actor: "human:timo", revision: 8, decided_on: {"recorded":{"test_result":1}}}
---
# Story: no-inspect-for-stateless-harness

## Outcome

A harness that keeps nothing between steps is not offered an action that can only end the run. Either read-only actions such as `repository.inspect` are not offered to a stateless harness, or the prompt says that choosing one ends the run with `NoProgress`.

## Context

Found by an independent code review of 2026-10-18, takt at 2a22093 (line numbers unchanged at 2ec88d4). Verified by the review with a fake harness; the operator's scenario logs show the same, a `repository.inspect` step right before `NoProgress`.

- `repository.inspect` is a protocol action with `effect: read` (`protocols/story-delivery/1.yaml:99-101`); the run offers every admissible action no validator owns (`crates/takt/src/run.rs:695-699`), and the prompt describes it as "read files to understand the code; change nothing" (`crates/takt/src/harness.rs:83`).
- Each `claude -p` step runs with `--no-session-persistence` (`harness.rs:189`): what an inspect step read is gone at the next step.
- A step that changes no file marks the state idle (`run.rs:1055-1058`); the next evaluation is identical, and the run returns `Stop::NoProgress`, exit 3 (`run.rs:683-685`).
- Overlaps: `story:no-model-step-for-approval` (a model step offered where none can help) and `story:ci-pending-waits` (the same idle path reached while waiting on the forge).

## Expected before

On takt at 2ec88d4, holds; after the fix, fails:

- A test with a fake harness that answers `repository.inspect` and changes nothing, on a fresh story case: the run stops with `Stop::NoProgress` after that one judgement step, and the prompt the fake received lists `repository.inspect`.

## Scope

- `crates/takt/src/run.rs`: the offered actions (cited)
- `crates/takt/src/harness.rs`: whether a harness is stateless, the prompt's description of read-only actions (cited)
- `crates/takt/tests/`: a fake-harness inspect test (inferred)

## Acceptance

With a stateless fake harness on a fresh story case, the prompt either does not offer `repository.inspect` or states that choosing it ends the run with `NoProgress`, and in the first case the run never stops with `NoProgress` from an inspect step.

## Landed (wave 12)

Read from `git diff --name-only 7e9b2ef^1 7e9b2ef` (the unit's merge into the wave branch) at the close of wave 12 (wave branch head 63a9d02). No read step for a stateless harness. Shared merge with story:pr-opens-when-change-is-done.

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
