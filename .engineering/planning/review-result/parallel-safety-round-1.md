---
format: aep.planning-md/3
id: review-result:parallel-safety-round-1
kind: review-result
status: active
title: Parallel-safety critic, round 1
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
revision: 1
---
needs-revision
story:confinement-seam — its scope lists `run.rs`, `main.rs`, `validator.rs`, `git.rs` and `harness.rs` (cited), so it collides with 9 of the other 11 stories, and only story:forge-pull-request-evidence is named in the body; the rest are unnamed (declared-loops, frontier, gate-validator, merge, run-record, second-harness, week, workflow). No `depends_on` edge or sentence records the order, so either add ordering edges that name the shared files as their reason, or split the surface so it stops claiming five modules — .engineering/planning/story/confinement-seam.md:45-49 (forge named only at :35)
story:second-harness-adapter — it shares `harness.rs`, `run.rs` and `main.rs` (cited) with story:workflow-schema-and-loader and `harness.rs` with story:confinement-seam, and `run.rs` or `main.rs` with frontier, declared-loops, merge, run-record, forge, gate-validator and week. The body names none of them and has no edge. Either add ordering edges, or narrow the surface: `run.rs` is listed beside "no behaviour change" — .engineering/planning/story/second-harness-adapter.md:39-43
story:workflow-schema-and-loader — it shares `run.rs` and `main.rs` (cited) with story:frontier-next-ready-story, `run.rs` with story:run-record-and-story-evidence, and `main.rs` with story:forge-pull-request-evidence; none is named and no edge orders them (only declared-loops and the unnamed `gate` are). Either add ordering edges that record the files, or split `run.rs` and `main.rs` so the two no longer share them — .engineering/planning/story/workflow-schema-and-loader.md:45-48
story:run-record-and-story-evidence — it shares `run.rs` (cited) with story:declared-loops and story:merge-under-authority; the body names neither, and its only edge runs to frontier. Either add ordering edges that record the file, or split `run.rs` — .engineering/planning/story/run-record-and-story-evidence.md:42-45
story:declared-loops — it shares `main.rs` with story:forge-pull-request-evidence and story:gate-validator and `run.rs` with story:merge-under-authority (all cited); none is named and no edge orders them (its edges run only to workflow and frontier). Either add ordering edges that record the files, or split the surface — .engineering/planning/story/declared-loops.md:43-46
story:frontier-next-ready-story — it shares `run.rs` with story:merge-under-authority and `main.rs` with story:forge-pull-request-evidence and story:gate-validator (cited); none is named and no edge orders them. Either add ordering edges that record the files, or split the surface — .engineering/planning/story/frontier-next-ready-story.md:45-49
story:gate-validator — it shares `validator.rs` (and `main.rs` with forge) with story:forge-pull-request-evidence and `validator.rs` with story:merge-under-authority (cited); the body names neither and has no edge to either. Either add ordering edges that record the file, or split `validator.rs` — .engineering/planning/story/gate-validator.md:42-45
story:frontier-next-ready-story — it is the only story whose scope lists `crates/takt/src/lib.rs`, yet confinement, forge, workflow, run-record and week each add a module (`confinement.rs`, `forge.rs`, `workflow.rs`, `events.rs`, `report.rs`) that needs a `pub mod` line there (inferred; every other scope omits it). The shared file goes unrecorded; either state it in each scope with an ordering edge, or give each new module's registration a single owner — .engineering/planning/story/frontier-next-ready-story.md:47 (`crates/takt/src/lib.rs:10-18` lists the module declarations)

What I read: 12 draft stories in full with `aep plan artifact show`-equivalent body reads. I also ran `aep plan artifact list --kind story --status draft`, `aep plan artifact waves --kind story --status draft` (9 waves, 63 collisions, 0 unassessed), `aep plan artifact graph` and `relations`, and checked `crates/takt/src/lib.rs` and file sizes.

Surfaces and acceptance reads:
- **Surfaces:** 12 of 12 placed by at least one cited existing file, 0 inferred-only, 0 unplaceable. New files such as `forge.rs` and `workflow.rs` are inferred, and none exists yet. Only the `lib.rs` finding rests on an inferred surface.
- **Acceptance reads:** 9 traced to a producer in the set (forge→protocol; merge→forge and protocol; declared-loops→workflow and frontier; run-record→frontier; week→run-record, declared-loops and merge). `depends_on` records all 9, so there is no reader-beside-producer finding.

What I could not establish:
- **Waves:** `waves` puts no two colliding items in one wave. The order of colliding pairs without an edge is the tool's choice, not recorded in any body or edge. That is why the unnamed pairs above are findings and not noise.
- **Out of my lane (not counted in the verdict):**
  - story:week-of-daily-runs is blocked by decision-blocker:approval-grant-and-case-resume yet sits in wave 9 as schedulable.
  - The `harness/<second>.rs` scope path in second-harness-adapter is a placeholder, not a file.

```findings
- file: .engineering/planning/story/confinement-seam.md
  line: 45
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "scope lists run.rs, main.rs, validator.rs, git.rs and harness.rs (cited), colliding with 9 of the other 11 stories while the body names only story:forge-pull-request-evidence and no ordering edge records the shared files; add ordering edges naming the files, or split the surface"
- file: .engineering/planning/story/second-harness-adapter.md
  line: 39
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "shares harness.rs, run.rs and main.rs (cited) with story:workflow-schema-and-loader and harness.rs with story:confinement-seam, and run.rs or main.rs with frontier, declared-loops, merge, run-record, forge, gate-validator and week; the body names none and has no edge; add ordering edges naming the files, or narrow the surface (run.rs is listed beside 'no behaviour change')"
- file: .engineering/planning/story/workflow-schema-and-loader.md
  line: 45
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "shares run.rs and main.rs (cited) with story:frontier-next-ready-story, run.rs with story:run-record-and-story-evidence and main.rs with story:forge-pull-request-evidence, none named and no edge orders them; add ordering edges that record the files, or split the surface"
- file: .engineering/planning/story/run-record-and-story-evidence.md
  line: 42
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "shares run.rs (cited) with story:declared-loops and story:merge-under-authority, neither named and no edge orders them; add ordering edges that record the file, or split run.rs"
- file: .engineering/planning/story/declared-loops.md
  line: 43
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "shares main.rs with story:forge-pull-request-evidence and story:gate-validator and run.rs with story:merge-under-authority (cited), none named and no edge orders them; add ordering edges that record the files, or split the surface"
- file: .engineering/planning/story/frontier-next-ready-story.md
  line: 45
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "shares run.rs with story:merge-under-authority and main.rs with story:forge-pull-request-evidence and story:gate-validator (cited), none named and no edge orders them; add ordering edges that record the files, or split the surface"
- file: .engineering/planning/story/gate-validator.md
  line: 42
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "shares validator.rs (and main.rs with forge) with story:forge-pull-request-evidence and validator.rs with story:merge-under-authority (cited), neither named and no edge orders them; add ordering edges that record the file, or split validator.rs"
- file: .engineering/planning/story/frontier-next-ready-story.md
  line: 47
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "inferred: only this story lists crates/takt/src/lib.rs, but confinement, forge, workflow, run-record and week each add a module needing a pub mod line there, and no scope or edge records the shared file; state it in each scope with an ordering edge, or give module registration a single owner"
```
