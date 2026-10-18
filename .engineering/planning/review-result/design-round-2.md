---
format: aep.planning-md/3
id: review-result:design-round-2
kind: review-result
status: active
title: Design critic, round 2
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
story:confinement-seam — the narrowing to three spawn sites stands, but the Checks line 68 still requires that no module but `confinement.rs` names `std::process::Command`, while its own edge (line 60) orders it after story:forge-pull-request-evidence, whose adapter shells out to `b10x-gates`, and after story:frontier-next-ready-story (via story:second-harness-adapter, story:workflow-schema-and-loader, story:run-record-and-story-evidence, story:merge-under-authority, story:forge-pull-request-evidence and story:gate-validator), whose adapter shells out to `aep`. `forge.rs` and `frontier.rs` are not in its scope, so that test fails when the story lands and the Out of Scope claim that later sites "go through the seam by construction" (line 73) is false for sites that land first. Either put `forge.rs` and `frontier.rs` in scope, or reverse the edge so the forge and frontier stories depend on this one, or limit the test to the three current sites — .engineering/planning/story/confinement-seam.md:68, .engineering/planning/story/confinement-seam.md:73, .engineering/planning/story/forge-pull-request-evidence.md:40, .engineering/planning/story/frontier-next-ready-story.md:34
story:story-done-end-to-end — its last Check reads the second adapter that story:second-harness-adapter produces ("Once story:second-harness-adapter is implemented, the same story run under the second adapter ends with decision documents equal…") and no `depends_on` edge runs to it, while story:second-harness-adapter defers the parity proof to this story, so if this story lands first nobody owns the proof. Add `depends_on story:second-harness-adapter`, or drop the check and give the parity proof to story:second-harness-adapter with an edge to this story — .engineering/planning/story/story-done-end-to-end.md:66, .engineering/planning/story/second-harness-adapter.md:31

**What I read.** 13 draft stories in full, the epic, the cleared blocker and review-result:design-round-1, plus `relations`, `graph`, `validate` (valid, nothing relayed) and `list --kind story --status draft`. I walked the full graph, including the epic, vision, blocker and 4 archived stories. That is 22 `depends_on` edges inside the set, with no cycle. The longest chain is 8 levels: frontier, gate, forge, merge, run-record, workflow, second-harness, confinement. I traced 15 acceptance and check reads to a producer in the set; 14 have an edge, and the one without is the story-done finding. Both round-1 findings landed: the forge to gate edge exists, and confinement is narrowed to the three current sites. The confinement finding is the leftover part of the second one.

**What I could not establish.**
- **Trade-off, not a finding.** Every `depends_on` edge records a shared file in its `## Ordering` section, so by the rubric the 8-level chain is not a serialising defect. The cost is that only story:takt-domain-model and story:story-delivery-protocol start at once. Splitting `main.rs` and `run.rs` (per-story CLI and loop modules) would buy width.
- **Out of lane, parallel safety.** Some pairs share files with no edge:
  - story:confinement-seam, story:declared-loops, story:story-done-end-to-end and story:week-of-daily-runs share `run.rs`, `main.rs` and `lib.rs`.
  - story:second-harness-adapter and story:declared-loops share `main.rs`.
- **Out of lane, scope.** I did not check the epic acceptance coverage. story:story-done-end-to-end now carries the M2 proof.

```findings
- file: .engineering/planning/story/confinement-seam.md
  line: 68
  category: design
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the narrowing to three spawn sites stands, but the Checks line still requires that no module but confinement.rs names std::process::Command, while its own edge (line 60) orders it after story:forge-pull-request-evidence, whose adapter shells out to b10x-gates, and after story:frontier-next-ready-story (via story:second-harness-adapter, story:workflow-schema-and-loader, story:run-record-and-story-evidence, story:merge-under-authority, story:forge-pull-request-evidence and story:gate-validator), whose adapter shells out to aep; forge.rs and frontier.rs are not in its scope, so that test fails when the story lands and the Out of Scope claim that later sites go through the seam by construction is false for sites that land first; either put forge.rs and frontier.rs in scope, or reverse the edge so the forge and frontier stories depend on this one, or limit the test to the three current sites"
- file: .engineering/planning/story/story-done-end-to-end.md
  line: 66
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "its last Check reads the second adapter that story:second-harness-adapter produces and no depends_on edge runs to it, while story:second-harness-adapter defers the parity proof to this story, so if this story lands first nobody owns the proof; add depends_on story:second-harness-adapter, or drop the check and give the parity proof to story:second-harness-adapter with an edge to this story"
```
