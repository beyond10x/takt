---
format: aep.planning-md/3
id: story:loop-case-opener
kind: story
status: draft
title: takt run opens a loop.improvement/1 case its validators can drive
relations:
- depends_on: story:loop-scorecard
- decomposes: epic:recursive-self-improvement
revision: 1
---
## Outcome

`takt run` on `protocols/loop-improvement/1.yaml` opens a `loop.improvement/1` case that its validators can drive: at open it sets `loop_baseline` to the promoted revision, takes the baseline snapshot (`snapshot_baseline`, `loop-baseline.json`) and records the case's independence, so the L2 turn in `.agents/planner-agent.md` can complete through takt.

## Context

From the reviewer's verdict on story:loop-scorecard at 20ab1c5 (needs-revision; this was its medium finding 3, not blocking): no production path calls `snapshot_baseline` or sets the loop artifacts' revisions. `Run::open` on loop-improvement/1 gives `loop_baseline` "r0", `loop_candidate` "r0", no `loop-baseline.json`, and the run stops NoProgress. `set_revision` has callers only for "implementation" (`crates/takt/src/case.rs:1362-1366`), and the independence the claims need is written into `case.yaml` by the tests only. It fails closed, so security holds, but the story's Outcome "a loop.improvement/1 case is driven by validators" is not met through `takt run`.

## Scope

- `crates/takt/src/case.rs`: revisions set at open (cited)
- `crates/takt/src/run.rs`: protocol-specific open steps (inferred)
- `crates/takt/src/loop_scorecard.rs`: `snapshot_baseline` (cited by the review)
- `crates/takt-cli/src/main.rs`: how the promoted revision reaches the opener (inferred)

## Acceptance

After `takt run --protocol protocols/loop-improvement/1.yaml` opens a case, `loop_baseline` names the promoted revision (not `r0`) and `<case>/loop-baseline.json` exists; with the three recorded turns present, `loop.measure` becomes due without a hand-written `case.yaml`.
