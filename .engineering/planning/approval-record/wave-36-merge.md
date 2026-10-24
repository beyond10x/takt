---
format: aep.planning-md/3
id: approval-record:wave-36-merge
kind: approval-record
status: approved
title: The merge of wave 36 into main is granted on the approve at 731d111
relations:
- decides: story:gitlab-forge
- decides: story:gitlab-ci-wait
- decides: story:harness-endpoint-is-configured
revision: 3
transitions:
- {from: "draft", to: "approved", at: "2026-10-24T00:18:21Z", actor: "human:timo", revision: 2}
---
# Approval: the merge of wave 36

## Decision

approval-record:wave-36-gitlab (the orchestrator under the owner's delegation, 2026-10-23) set
the wave's end: story:gitlab-forge and story:gitlab-ci-wait, each with one real run on the
private trial project; approval-record:trial-harness-arm-endpoint § Scheduling added
story:harness-endpoint-is-configured ahead of them, and the orchestrator's word of 2026-10-24
(loom 0.39.1 released, its verify-release exit 0) set its close: move the pin, drop the by-name
refusal, release. All three hold at 731d111: the real run (case takt-20261023T225103Z) opened
the merge request through glab, recorded `ci_result` pass and stopped at merge approval, exit 4;
U3c hands the secret's name to b10x-harness on loom 0.39.1. The reviewer rejected at 1782919 on
two stale assertions (review-result:wave-36-1782919-2) and approves at 731d111
(review-result:wave-36-731d111-2), fourteen findings fixed with tests, two filed; both records
supersede the ones first filed, which the reviewer's confirmation at 05c5708 found misstated
in four places, none of them a finding or a verdict. The diff touches judge surfaces
(`crates/**`, `Cargo.toml`, both lockfiles, `ess/**`, `.engineering/planning/**`);
`.engineering/delegations.yaml` names no delegation, so the grant is the owner's side's, recorded
here under the orchestrator's grant, and the planner merges on it, at the store commit after
731d111 that the reviewer confirms.

## What it does not grant

The release, which approval-record:release-0-11-0 decides; the trial agent's harness-arm run on
the two endpoints, which the orchestrator starts from the release tag.
