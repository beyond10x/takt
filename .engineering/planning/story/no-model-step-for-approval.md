---
format: aep.planning-md/3
id: story:no-model-step-for-approval
kind: story
status: implemented
title: A run takes no model step when only a validator or a grant can advance the case
summary: 'R19: stop ApprovalRequired with no harness call when only an ungranted capability stands between the case and its outcome'
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
scope:
- confidence: inferred
  path: crates/takt/src/run.rs
- confidence: inferred
  path: crates/takt/tests/no_model_step.rs
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-10-18T18:27:21Z", actor: "human:timo", revision: 4}
- {from: "proposed", to: "active", at: "2026-10-18T18:27:21Z", actor: "human:timo", revision: 5}
- {from: "active", to: "implemented", at: "2026-10-19T04:04:07Z", actor: "human:timo", revision: 7, decided_on: {"recorded":{"test_result":1}}}
---
# Story: no-model-step-for-approval

## Outcome

When the only actions that could advance a case are deterministic (a validator) or wait for a grant, the run takes no judgement step: it runs the validator, or stops with `ApprovalRequired` at once, and spends no model tokens on asking for the approval.

## Context

Owner decision of 2026-10-18 (requirement R19 in product-requirements:takt). Observed in the operator's scenario 1 runs on the scratch repository:

- `~/.cache/takt-trial-2026-10-18/run5.log:13-16`: step 1 `pr.open` and step 2 `ci.read` are validators; step 3 is `repository.merge (claude-code)` with effect `none; repository.merge needs an approval`; the run stopped `ApprovalRequired (repository.merge); steps 3, tokens 29908`. All 29,908 tokens went to the one model step that only requested the approval.
- The same shape at `scenario-a.log:81` and `scenario-b.log:79-82` (`tokens 29896`).

A frontier whose only open claim is reached through an action that `requires` a capability not granted is decided by the protocol, not by the model.

## Expected before

On takt at 7b5aca3, holds; after the fix, fails:

- A test with a protocol whose remaining claim needs `repository.merge` (requires `repository.merge`, not granted) and a recording harness: the run calls the harness once before stopping `ApprovalRequired`.
- Scenario 1 resumed after CI passes: the stop line of that invocation reports tokens above 0 while every step before the stop is a validator.

## Scope

- `crates/takt/src/run.rs`: the step choice before a judgement step (inferred)
- `crates/takt/tests/`: a no-model-step test (inferred)

## Acceptance

With every claim but those behind an ungranted capability held, the run stops `ApprovalRequired` naming the capability with 0 harness calls in that invocation; a case with an admissible judgement action that could still change a claim still offers it.

## Landed (wave 12)

Read from `git diff --name-only 4ae2ea1^1 4ae2ea1` (the unit's merge into the wave branch) at the close of wave 12 (wave branch head 8d7c194). No harness call when only an ungranted capability remains. Shared merge with story:pr-opens-when-change-is-done.

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
