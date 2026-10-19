---
format: aep.planning-md/3
id: story:cases-root-link-is-undone
kind: story
status: draft
title: A step that replaces the cases directory with a link is undone, not resumed through
tags:
- priority:high
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
- informed_by: review-result:main-fe93286-areas
revision: 2
---
## Outcome

A harness step or a test command that replaces `.engineering/cases` (or a case directory) with a symbolic link is undone like any other foreign write: the link is removed, the directory is put back from the snapshot, the step's HEAD, index and commits are rolled back, and the run stops naming the link. A case is never resumed through a link.

## Context

From the review of main at 9cc158b on 2026-10-19: ten independent reviewers, one per area, each finding then fact-checked by a second reviewer against the code (review-result:main-fe93286-areas), finding R5.1, confirmed line by line. `run.rs:3783@9cc158b` snapshots the cases directory; after a step that swaps it for a link, `run.rs:241` sees no directory and the snapshot is empty, so `run.rs:3899` finds every path changed; `run.rs:3925-3926` calls `restore(..)?` before `undo()`, `restore` bails at `run.rs:307-311` ("replaced by a link"), and the `?` leaves `run_held` without `undo_step` or `Checkpoint::restore`. The validator path does the same at `run.rs:3596-3598`. `CaseDir::resume` (`case.rs:1514-1551`) has no link check, so the next run resumes through the link, every later snapshot is empty, and a later step can write `authority.yaml` through it; `PrMerged` checks only `grants_at` (`validator.rs:1409-1410`).

## Scope

- `crates/takt/src/run.rs`: `restore`, the undo order on the judgement and validator paths (cited)
- `crates/takt/src/case.rs`: `resume` refuses a linked cases root or case directory (cited)

## Acceptance

A step running `cp -r .engineering/cases ../x && rm -rf .engineering/cases && ln -s ../x .engineering/cases` leaves, after the run exits, `.engineering/cases` a directory equal to its snapshot, HEAD and refs as before the step, and a stop naming the link; `takt run --case <id>` on a tree whose cases root is a link refuses before the first step.
