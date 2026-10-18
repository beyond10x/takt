---
format: aep.planning-md/3
id: review-result:parallel-safety-round-2
kind: review-result
status: active
title: Parallel-safety critic, round 2
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
story:story-done-end-to-end — it shares `crates/takt-cli/src/main.rs` and `crates/takt/src/run.rs` with story:workflow-schema-and-loader, story:declared-loops and story:confinement-seam, and `main.rs` alone with story:second-harness-adapter and story:week-of-daily-runs; its four `depends_on` edges reach none of the five and none of the five reaches it, so the pairs are unordered and unnamed (inferred, since its scope is inferred; the first three pairs are in the `waves` collision list); record an edge per pair naming the shared file, or split the surface — .engineering/planning/story/story-done-end-to-end.md:48
story:declared-loops — it shares `crates/takt-cli/src/main.rs` with story:second-harness-adapter, and `main.rs` and `crates/takt/src/run.rs` with story:confinement-seam; no `depends_on` path runs between either pair and the `## Ordering` section names neither (cited: `main.rs` and `run.rs` are cited in both bodies); record an edge per pair naming the shared file, or split the surface — .engineering/planning/story/declared-loops.md:50
story:week-of-daily-runs — it shares `crates/takt-cli/src/main.rs` with story:second-harness-adapter, and `main.rs` and `crates/takt/src/lib.rs` with story:confinement-seam; no `depends_on` path runs between either pair and the `## Ordering` section names neither (`main.rs` cited; `lib.rs` inferred in both bodies); record an edge per pair naming the shared file, or split the surface — .engineering/planning/story/week-of-daily-runs.md:52

What I read: 13 draft stories, using `aep plan artifact list --kind story --status draft`, `aep plan artifact waves --kind story --status draft` (11 waves, 99 collisions, 0 unassessed) and `aep plan artifact show` on each. I took the `## Ordering` sections and the `depends_on` edges from the story bodies and computed the transitive closure myself.

Surfaces: 13 of 13 stories have at least one cited surface and none was unplaceable. Every story has some inferred scope entries, and story:story-done-end-to-end has only inferred ones (3 of 3), so its collision claims are the weaker kind.

Reader beside producer: I traced 8 reads to a producer in the set, and `depends_on` records all 8:
- forge to gate (`gate_result`)
- week to run-record (`events.jsonl`)
- week to merge (`approval`)
- workflow to gate (`gate` block)
- merge to forge (`Forge`)
- run-record, declared-loops and e2e to their producers

Fixed since round 1: the 99 collisions reduce to 9 unordered pairs. Every other collision is ordered through the direct or transitive `depends_on` chain (frontier → gate → forge → merge → run-record → workflow → declared-loops and second-harness → confinement).

The 9 pairs:
- e2e–workflow
- e2e–declared-loops
- e2e–second-harness
- e2e–confinement
- e2e–week
- declared-loops–second-harness
- declared-loops–confinement
- week–second-harness
- week–confinement

`waves` happens to place these pairs in different waves, but that is a computed placement and no body records it. Which direction each edge runs is the operator's call.

What I could not establish:
- The shared files on every pair involving story:story-done-end-to-end rest on its inferred scope (the `waves` output marks them inferred).
- I did not read the full text of round 1's verdict (review-result:parallel-safety-round-1), so I judged only the current set.

```findings
- file: .engineering/planning/story/story-done-end-to-end.md
  line: 48
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "it shares crates/takt-cli/src/main.rs and crates/takt/src/run.rs with story:workflow-schema-and-loader, story:declared-loops and story:confinement-seam, and main.rs with story:second-harness-adapter and story:week-of-daily-runs, with no depends_on path in either direction and no mention in its Ordering section; the surfaces are inferred, so the claim is weaker; record an edge per pair naming the shared file, or split the surface"
- file: .engineering/planning/story/declared-loops.md
  line: 50
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "it shares crates/takt-cli/src/main.rs with story:second-harness-adapter and main.rs and crates/takt/src/run.rs with story:confinement-seam, with no depends_on path between either pair and no mention in its Ordering section (cited); record an edge per pair naming the shared file, or split the surface"
- file: .engineering/planning/story/week-of-daily-runs.md
  line: 52
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "it shares crates/takt-cli/src/main.rs with story:second-harness-adapter and main.rs and crates/takt/src/lib.rs with story:confinement-seam, with no depends_on path between either pair and no mention in its Ordering section (main.rs cited, lib.rs inferred); record an edge per pair naming the shared file, or split the surface"
```
