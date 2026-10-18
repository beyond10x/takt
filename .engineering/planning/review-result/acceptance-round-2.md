---
format: aep.planning-md/3
id: review-result:acceptance-round-2
kind: review-result
status: active
title: Acceptance critic, round 2
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
- reviews: story:story-done-end-to-end
- reviews: story:takt-domain-model
- reviews: story:week-of-daily-runs
- reviews: story:workflow-schema-and-loader
revision: 1
---
needs-revision
story:confinement-seam — the check "No module but `crates/takt/src/confinement.rs` names `std::process::Command`" cannot pass inside this story's scope: it depends_on story:forge-pull-request-evidence (the `b10x-gates` adapter) and, through that story, story:gate-validator and story:frontier-next-ready-story (the `aep` adapter), so those spawn sites already exist and are routed by neither the scope nor the acceptance; the Out of Scope text calls them "later" stories, and the check should either name them or be limited to the three sites of today — .engineering/planning/story/confinement-seam.md:68
story:takt-domain-model — the acceptance demands an `UNMAPPED:` marker "on every relation listed above as undecided", but the only relations the body lists as undecided (Story → Case resume, who writes AuthorityDecision) were decided when decision-blocker:approval-grant-and-case-resume was cleared, so the set to be marked is stale and two readers get different counts; the acceptance should name the relations still open, or none — .engineering/planning/story/takt-domain-model.md:51

What I read: 13 draft stories under epic:composable-agentic-work, all opened whole with `aep plan artifact list --kind story --status draft` and `aep plan artifact show <id>`. I also read review-result:acceptance-round-1, the epic, decision-blocker:approval-grant-and-case-resume (status cleared) and the story lifecycle, and ran `git grep Command -- crates`. The four round-1 findings are fixed: story:run-record-and-story-evidence, story:second-harness-adapter, story:workflow-schema-and-loader and story:declared-loops each now have one acceptance outcome, with the rest in `## Checks`. story:story-done-end-to-end was not in round 1 and passes: its acceptance is one end-to-end scenario and its checks are observable.

Could not establish:
- story:story-delivery-protocol — "`repository.merge` approval-required before the last record" is ambiguous. It reads as true only after the fourth record, because the preconditions `ci.green` and `pr.opened` are unmet after records one to three. It was in round 1 and I passed it then. I could not tell whether Canon reports an unmet-precondition action as approval-required, so I did not raise a finding.
- story:second-harness-adapter — it compares live-model runs, which may not reproduce. This is the same unease as round 1.
- Out of my lane (scope or design): story:story-done-end-to-end has a check conditional on "Once story:second-harness-adapter is implemented", with no `depends_on` to it. story:second-harness-adapter defers the M4 parity proof to story:story-done-end-to-end, so that proof can end up with no unconditional owner.

```findings
- file: .engineering/planning/story/confinement-seam.md
  line: 68
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: undecided
  message: "the check 'No module but crates/takt/src/confinement.rs names std::process::Command' cannot pass inside this story's scope: it depends_on story:forge-pull-request-evidence (and through it story:gate-validator and story:frontier-next-ready-story), whose aep and b10x-gates adapters already spawn processes, yet the scope and acceptance route only today's three sites and Out of Scope calls those adapters 'later'"
- file: .engineering/planning/story/takt-domain-model.md
  line: 51
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: undecided
  message: "the acceptance demands an UNMAPPED marker on 'every relation listed above as undecided', but the relations the body lists as undecided were decided when decision-blocker:approval-grant-and-case-resume was cleared, so the set to mark is stale and two readers get different answers"
```
