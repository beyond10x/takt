---
format: aep.planning-md/3
id: decision-blocker:unattended-merge-grant
kind: decision-blocker
status: cleared
title: Who grants the merge of a run nobody watches
relations:
- blocks: story:an-unattended-run-merges-under-a-delegated-grant
revision: 3
transitions:
- {from: "open", to: "cleared", at: "2026-10-29T21:29:55Z", actor: "human:timo", revision: 3}
---
# Decision: who grants the merge of a run nobody watches

Decider: operator

The cluster example's goal is three stories merged to `main` from a pod with no person's step.
Today every merge needs `takt approve` by a person at the case's revision.

| option | what it does | cost |
|---|---|---|
| A (recommended) | the owner records once, in `.engineering/delegations.yaml` of each target repository, a revocable delegation of `repository.merge` and `repository.merge.judge_surface` to one agent role; the run grants as that role only after a reviewer pass at the exact revision; CI on the pull request still gates the merge | one story in takt; one owner commit per repository; revoking is one line |
| B | the owner runs `takt approve` for each of the three merges | no takt change; the goal's "no operator step" is not met |
| C | the run file names a standing grant of `repository.merge` for the run | no reviewer at the revision; weakest of the three |

Recommendation: A. It reuses the delegation record story:judge-guard built, keeps the grant bound
to one revision, and the owner can revoke it.

## Answer

A. The owner chose the goal whose outcome reads "each ending as a bot PR merged to main, no operator step" (owner, 2026-10-29: "goal -> A", relayed by the orchestrating session). Of the three options only A meets that outcome with a review bound to the revision. The grant covers the goal's pod runs only, through the bot route, for the stories the orchestrating session accepts.
