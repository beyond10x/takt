---
format: aep.planning-md/3
id: approval-record:live-trial-seed-delegates-the-merge
kind: approval-record
status: accepted
title: The trial seed delegates the judge-surface merge to agent:takt-trial
summary: scenario-1 seed 896382c adds .engineering/delegations.yaml
relations:
- decides: epic:downstream-story-delivery
- informed_by: story:the-approval-stop-names-a-missing-delegation
revision: 3
transitions:
- {from: "draft", to: "accepted", at: "2026-10-27T17:55:33Z", actor: "human:timo", revision: 3}
---
Decider: the operator's delegate for the live trials, 2026-10-27 17:46Z, recorded by the trial-review
coordinator.

The first trial on 0.15.1 (todo-app-example pull request #296, closed unmerged) stopped `NoProgress`,
exit 3. Its seed had neither `.engineering/judge-allow.yaml` nor `.engineering/delegations.yaml`, so
every changed path was a judge surface. The trial's grant was an agent role's (`--by agent:<role>`).
With no delegation of `repository.merge.judge_surface` on record at the merge target's tip,
`repository.merge` refused, after the grant and the resume.

Decided and applied:

- todo-app-example gets a new scenario-1 seed, `896382c`, on the old seed `865eaa8`. It adds
  `.engineering/delegations.yaml` with one delegation: `owner: human:operator`, `role: agent:takt-trial`,
  `capability: repository.merge.judge_surface`, `given_at: 2026-10-27T17:44:48Z`. Its `main`
  (`727bf54`) points scenario 1 at that seed.
- The delegation is given in the operator's name under the operator's standing instructions for the
  trials, and is revocable with `revoked_at`.
- The trial agent grants with `--by agent:takt-trial`, exactly the delegated role.
- The code gap, that the `ApprovalRequired` stop and `takt approve` do not name the missing
  delegation, is story:the-approval-stop-names-a-missing-delegation.
