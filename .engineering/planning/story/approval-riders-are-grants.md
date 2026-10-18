---
format: aep.planning-md/3
id: story:approval-riders-are-grants
kind: story
status: draft
title: Only approval records of granted capabilities ride on a granted revision
summary: approvals_only reads each riding evidence file and accepts only approval records matching a grant in authority.yaml
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
scope:
- confidence: cited
  path: crates/takt/src/case.rs
- confidence: inferred
  path: crates/takt/tests/approval_riders.rs
revision: 4
---
# Story: approval-riders-are-grants

## Outcome

A commit that rides on a granted revision to the base carries only the approval records of the grants in the case's `authority.yaml`: each added file is a record of kind approval for the case's story, naming a capability and a grantor that `authority.yaml` holds as granted, and one record per grant. Any other file in the story's evidence directory, such as a `test_result`, stops the push and the merge with nothing merged.

## Context

Found by the operator's review of story:approval-record-reaches-base at 5a41e71 (merged into main at 7b5aca3). `approvals_only` (`crates/takt/src/case.rs:751`) accepts a commit above the granted revision when it has one parent, the approve subject, and `adds_records_only` (`case.rs:790`) finds only additions of new regular files directly in `.engineering/evidence/story/<name>/`; their count is bounded by the grants. The contents of those files are not read. A commit with the approve subject that adds, for example, a `test_result` record would reach the base with the merge, and the planning store reads evidence there to admit a move such as `implemented`.

Today making such a commit needs a commit takt did not make (the judgement-step guard reverts a step that moves `HEAD`), so the gap is narrow; the merge still only takes approve-subject commits, bounded by the number of grants.

## Expected before

On takt at 7b5aca3, holds; after the fix, fails:

- A test that builds a case at a granted revision, then a commit with subject `takt: approve <story> repository.merge` adding `.engineering/evidence/story/<name>/x.json` holding a `test_result` record, finds `approvals_only(tree, revision, head, case_dir)` true.

## Scope

- `crates/takt/src/case.rs`: `approvals_only`, `adds_records_only` (cited)
- `crates/takt/tests/`: a riding-record test (inferred)

## Acceptance

`approvals_only` is true only when each added file parses as the planning store's approval record for the case's story, its capability and grantor match a `Granted` decision in `authority.yaml`, and no grant is recorded twice; a file of any other kind, or one naming a capability not granted, makes it false and the merge merges nothing.
