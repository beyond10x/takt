---
format: aep.planning-md/3
id: story:the-approval-stop-names-a-missing-delegation
kind: story
status: draft
title: The approval stop names a judge-surface merge an agent grant cannot pass
summary: the missing delegation is named at exit 4 and at takt approve, not after the resume
relations:
- decomposes: epic:downstream-story-delivery
- serves: vision:safe-agentic-coding
revision: 2
---
# Story: the-approval-stop-names-a-missing-delegation

## Outcome

When a case stops `ApprovalRequired (repository.merge)` and its change touches a judge surface,
the stop line says so before anyone grants anything. It names the judge surfaces, as the judge
guard would at the merge target's tip, and says that only the owner's grant (`human:<id>`)
merges them unless `.engineering/delegations.yaml` at that tip records an unrevoked delegation of
`repository.merge.judge_surface` to the agent role. `takt approve --by agent:<role>` refuses a
grant the merge guard would refuse, before it commits anything, and names the missing delegation.

## Context

- A trial on 0.15.1 (2026-10-27) merged nothing. The seed had neither `judge-allow.yaml` nor
  `delegations.yaml`, so every changed path was a judge surface. The `ApprovalRequired` stop did
  not mention this. `takt approve --by agent:<role>` succeeded, and the resumed run's
  `repository.merge` refused: "no delegation of repository.merge.judge_surface to it is on
  record" (`crates/takt/src/validator.rs`, `approver_refusal`).
- The judge guard runs only inside the merge validator (`PrMerged::judge_guard`), after the grant
  and the resume. The same read at the stop and at `takt approve` would save a resume and a
  wasted grant.

## Acceptance

1. `approval_names_delegation::the_stop_names_the_judge_surfaces_and_the_missing_delegation`: a case
   whose change touches a path no allow-list admits stops `ApprovalRequired`, exit 4, and its
   stop line names the path and `.engineering/delegations.yaml`.
2. `approval_names_delegation::an_agent_grant_the_guard_would_refuse_is_refused_at_approve`:
   `takt approve --by agent:<role>` on that case exits 1 and commits nothing, naming the
   delegation it needs. `--by human:<id>` is recorded as before.
3. `approval_names_delegation::a_delegated_role_is_granted`: with the delegation at the tip,
   the agent's grant is recorded and the resumed run merges.
4. `approval_names_delegation::an_admitted_change_says_nothing_extra`: a change every path of
   which `judge-allow.yaml` admits stops as it does today.
