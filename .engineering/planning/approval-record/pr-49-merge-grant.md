---
format: aep.planning-md/3
id: approval-record:pr-49-merge-grant
kind: approval-record
status: approved
title: Pull request 49 merges into main at d4ce8808
summary: the owner's grant through the orchestrator, on the reviewer's approve at d4ce8808
relations:
- decides: story:scorecard-refuses-missing-repo
- informed_by: review-result:pr-49-grant-review
- informed_by: decision-blocker:wave-29-merge-grant
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-10-31T04:29:54Z", actor: "human:timo", revision: 2}
---
## Decision

The owner's side granted, through the orchestrator under the owner's grant, the merge of pull request 49 at `d4ce8808` into `main`. This follows the rule decision-blocker:wave-29-merge-grant set for later merges: "one grant per merge, from the coordinator, on a clean verdict at its sha".

- The diff touches judge surfaces only: the story, `crates/takt/src/scorecard.rs`, `crates/takt/tests/scorecard.rs` and `ess/domains/takt.yaml`. `.engineering/delegations.yaml` names no delegation in force: the planner's of 2026-10-29 was revoked the same day. This grant is therefore the owner's, not the planner's.
- It rests on review-result:pr-49-grant-review, whose verdict is `approve` at `d4ce8808`, with no blocking finding. The diff does not widen authority.
- It holds for `d4ce8808` alone. A new head needs a new verdict and a new grant.
- The merge goes through the bot route, `PUT /repos/beyond10x/takt/pulls/49/merge` with `sha` `d4ce8808`.
- The pod run's `agent:takt-delivery` has no merge right in `.engineering/delegations.yaml`, so its run stopped at the merge step. This grant does not give that role a merge right.
