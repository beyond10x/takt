---
format: aep.planning-md/3
id: story:scorecard-binds-binary-digest
kind: story
status: draft
title: A scorecard row names the sha256 of the binary that ran
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
- depends_on: story:scorecard-save-checks-binary
revision: 1
---
## Outcome

Each scorecard row carries the sha256 of the takt binary that ran the case, read from the runner's record beside the case (`binary.txt`, path and sha256), and `takt promote` refuses a scorecard whose rows name more than one binary or a binary other than the one its confined build produced for the revision.

## Context

From the planner's ruling on the wave-13 promotion stories (E2, 2026-10-19, decision 4): a row binds the revision its `opened` line names; two binaries built from one revision with different toolchains or a stale embedded revision are not told apart (a local build in a tree whose branch history was rewritten embedded the old sha until `build.rs` reran). The runner already records the binary's path and sha256 per run.

## Scope

- `crates/takt/src/scorecard.rs`: the row and the reader (inferred)
- `crates/takt/src/promotion.rs`: the binary check (inferred)

## Acceptance

A scorecard whose five rows name two different binary digests is refused naming both; one naming a digest other than the confined build's is refused naming the two digests.
