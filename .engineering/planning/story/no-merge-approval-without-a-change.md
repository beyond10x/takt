---
format: aep.planning-md/3
id: story:no-merge-approval-without-a-change
kind: story
status: draft
title: No merge approval is asked for before the case branch has a change
tags:
- finding
refs:
- provider: finding
  reference: no-merge-approval-without-a-change
relations:
- informed_by: review-result:variety-runs-a0b8576
revision: 1
---
## Outcome

A case asks for the merge approval only once its branch carries a change: on the unchanged base, `repository.merge` stays blocked, whatever the tests say.

## Context

From the planner's variety runs of 2026-10-18 on four real tasks from public beyond10x repositories (canon, worktree, metaharness), takt at the promoted a0b8576: in all four, the decision written right after step 1 (`decisions/0002.json`) already had `repository.merge` approval-required with zero commits on the case branch. When a case opens, `implementation` is bound to the base HEAD (`crates/takt/src/case.rs:1364` on main, `:1098` at a0b8576), so a passing test run on the unchanged base makes `implementation.verified` true. The model then spends a step asking for an approval of nothing, and an operator could grant a merge of an empty change. The efficiency unit gates `pr.open` on `change.made` in `story.delivery/1`; the `software.change/1` protocol that `takt run --intent` uses has no such gate. Reproduced on scenario 1 at 9922f12: exit 4, `stopped: ApprovalRequired (repository.merge); steps 2` with no commit beyond the base.

```expected_before
scenario: 1
command: "$TAKT" run --repo "$SCENARIO" --intent "add a CHANGELOG entry" --forge none --harness none --iterations 3 --test-cmd true
exit: 4
output: stopped: ApprovalRequired (repository.merge); steps 2
source: .engineering/planning/review-result/variety-runs-a0b8576.md
```

## Scope

- `crates/takt/src/case.rs`: the `implementation` revision set at open (cited)
- `protocols/` built-in `software.change/1`: the claim `repository.merge` requires (inferred)

## Acceptance

On the predicate's run, takt does not stop at `ApprovalRequired (repository.merge)` while the case branch has no commit beyond its base outside `.engineering/`; once a change is committed and its tests pass, the approval is asked for as before.
