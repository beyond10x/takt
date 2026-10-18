---
format: aep.planning-md/3
id: review-result:design-round-1
kind: review-result
status: active
title: Design critic, round 1
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
- reviews: epic:composable-agentic-work
revision: 1
---
needs-revision
story:forge-pull-request-evidence — its acceptance needs the tests and gate validators green, and the `gate_result` record is written by the `gate.run` validator that story:gate-validator adds, yet no `depends_on` edge runs from it to story:gate-validator, so `aep plan artifact waves` can place it beside its producer; add `depends_on story:gate-validator` — .engineering/planning/story/forge-pull-request-evidence.md:55, .engineering/planning/story/gate-validator.md:33
story:confinement-seam — its outcome says nothing a run launches, including the forge, escapes the seam, but its scope and acceptance cover only the three existing spawn sites, and the sibling stories add spawn sites (the forge adapter's `b10x-gates` calls, the frontier adapter's `aep` calls, a possible `canon-engineering` child in the gate validator) with no edge in either direction; add `depends_on` from it to story:forge-pull-request-evidence, story:frontier-next-ready-story and story:gate-validator and put their files in scope, or narrow the outcome to the three current spawn sites — .engineering/planning/story/confinement-seam.md:31, .engineering/planning/story/confinement-seam.md:35, `aep plan artifact graph`

What I read: 12 draft stories, the epic and the blocker in full, via `aep plan artifact list --kind story --status draft`, `show` on each id, `relations`, `graph` and `validate` (valid, nothing relayed). I also read the design's § 4 milestone table. I walked 30 declared edges, covering the epic and the 5 archived stories through their `decomposes` edges: 9 `depends_on`, 2 `blocks` and 19 `decomposes`. I found no cycle, and the `depends_on` chains are shallow (at most 3 deep), so they do not serialise the set. I traced 9 acceptance reads to a producer in the set, and 8 of them have a recorded edge. The 9th is the gate validator read in the first finding.

What I could not establish:
- **Out of my lane, scope critic:** the epic acceptance (one ready story finished with only the merge approval manual) has no end-to-end story, and the blocker says so at `decision-blocker/approval-grant-and-case-resume.md` under "What this stops".
- **Not a finding:** story:run-record-and-story-evidence and story:declared-loops accept on a case that "terminated accepted". The kernel can reach `Stop::Completed` (`crates/takt/src/run.rs:179`) with a fixture protocol, so I did not call it a hidden dependency on story:merge-under-authority. If the tests use `protocols/story-delivery/1.yaml`, that edge is needed.
- **Out of my lane, parallel safety:** several stories edit `run.rs`, `main.rs`, `harness.rs` and `validator.rs` (the workflow loader, the second harness adapter and the confinement seam all touch `harness.rs`). I did not judge whether those can run at once.

```findings
- file: .engineering/planning/story/forge-pull-request-evidence.md
  line: 55
  category: design
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "its acceptance needs the tests and gate validators green, and the gate_result record is written by the gate.run validator that story:gate-validator adds, yet no depends_on edge runs from it to story:gate-validator, so aep plan artifact waves can place it beside its producer; add depends_on story:gate-validator"
- file: .engineering/planning/story/confinement-seam.md
  line: 31
  category: design
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "its outcome says nothing a run launches, including the forge, escapes the seam, but its scope and acceptance cover only the three existing spawn sites, and the sibling stories add spawn sites (the forge adapter's b10x-gates calls, the frontier adapter's aep calls, a possible canon-engineering child in the gate validator) with no edge in either direction; add depends_on from it to story:forge-pull-request-evidence, story:frontier-next-ready-story and story:gate-validator and put their files in scope, or narrow the outcome to the three current spawn sites"
```
