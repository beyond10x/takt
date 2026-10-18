---
format: aep.planning-md/3
id: review-result:acceptance-round-1
kind: review-result
status: active
title: Acceptance critic, round 1
relations:
- reviews: story:confinement-seam
- reviews: story:declared-loops
- reviews: story:forge-pull-request-evidence
- reviews: story:frontier-next-ready-story
- reviews: story:gate-validator
- reviews: story:merge-under-authority
- reviews: story:run-record-and-story-evidence
- reviews: story:second-harness-adapter
- reviews: story:story-delivery-protocol
- reviews: story:takt-domain-model
- reviews: story:week-of-daily-runs
- reviews: story:workflow-schema-and-loader
- reviews: decision-blocker:approval-grant-and-case-resume
revision: 1
---
needs-revision

story:run-record-and-story-evidence — the acceptance joins three independent outcomes in one sentence (the `events.jsonl` contents, the `test_result` evidence record in the store, the move to `implemented`), so each can pass while the others fail; it should state one outcome, with the rest as their own checks — .engineering/planning/story/run-record-and-story-evidence.md:49
story:second-harness-adapter — the acceptance joins two independent outcomes ("decision documents ... are equal" across the two adapters, "and a scripted adapter that answers an action it was not offered is refused and reverted"), so parity can pass while refusal fails — .engineering/planning/story/second-harness-adapter.md:47
story:workflow-schema-and-loader — the acceptance joins two independent outcomes (flag run and `--workflow` run produce equal decision documents, "and a workflow naming an action the protocol does not declare is refused before any case is opened"), so equivalence can pass while the refusal fails — .engineering/planning/story/workflow-schema-and-loader.md:54
story:declared-loops — the acceptance joins two independent mechanisms (a declared outer-loop budget of three that opens exactly three cases, "and the same workflow with `--until accepted` ... stops before its budget is spent"), so the budget can pass while `--until` fails — .engineering/planning/story/declared-loops.md:49

What I read: 13 artifacts, all opened whole. They are the 12 draft stories from `aep plan artifact list --kind story --status draft` and `decision-blocker:approval-grant-and-case-resume`. I used `aep plan artifact show` on each, `aep plan artifact kinds` and `lifecycle` for story and blocker, and `git grep` for `repository.merge` in `crates`. The other eight stories have one checkable acceptance each. I treated a pass case plus a negative case of one rule as one statement: frontier, forge, gate, merge, confinement.

Could not establish:
- The blocker has no acceptance section. Blockers are cleared by a decision, so I did not judge it as a story.
- story:week-of-daily-runs counts "human interventions" from events whose form the open blocker defines. That is a dependency, and story:week-of-daily-runs is already marked blocked, so I did not raise it. Coupling is `plan-critic-design`'s lane.
- story:second-harness-adapter compares live runs of two models, which may not reproduce. I could not judge that without running them, so I did not raise it as a finding.

```findings
- file: .engineering/planning/story/run-record-and-story-evidence.md
  line: 49
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the acceptance joins three independent outcomes (the events.jsonl contents, the test_result evidence record in the store, the move to implemented) in one sentence, so each can pass while the others fail"
- file: .engineering/planning/story/second-harness-adapter.md
  line: 47
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the acceptance joins two independent outcomes (decision documents equal across the two adapters, and a scripted adapter answering an unoffered action is refused and reverted), so parity can pass while refusal fails"
- file: .engineering/planning/story/workflow-schema-and-loader.md
  line: 54
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the acceptance joins two independent outcomes (flag run and --workflow run produce equal decision documents, and a workflow naming an undeclared action is refused before any case is opened), so equivalence can pass while the refusal fails"
- file: .engineering/planning/story/declared-loops.md
  line: 49
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the acceptance joins two independent mechanisms (an outer-loop budget of three opening exactly three cases, and --until accepted stopping before the budget is spent), so the budget can pass while --until fails"
```
