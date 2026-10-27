---
format: aep.planning-md/3
id: story:a-cases-root-survives-worktree-cleanup
kind: story
status: implemented
title: takt marks its cases root as retained, so worktree cleanup keeps it
summary: writes .takt/.worktreekeep beside .takt/.gitignore
tags:
- priority:high
relations:
- serves: vision:safe-agentic-coding
- decomposes: epic:downstream-story-delivery
- depends_on: story:cases-live-under-dot-takt
scope:
- confidence: inferred
  path: crates/takt/src/case.rs
- confidence: inferred
  path: crates/takt/tests/cases_kept.rs
- confidence: inferred
  path: ess/domains/takt.yaml
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-10-27T12:00:42Z", actor: "human:timo", revision: 5, decided_on: {"recorded":{"review_outcome":1}}}
- {from: "proposed", to: "active", at: "2026-10-27T12:00:42Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"review_outcome":1}}}
- {from: "active", to: "implemented", at: "2026-10-27T15:59:15Z", actor: "human:timo", revision: 7, decided_on: {"recorded":{"test_result":1,"review_outcome":1}}}
---
# Story: a-cases-root-survives-worktree-cleanup

## Outcome

When takt creates its state root `.takt/`, it writes `.takt/.worktreekeep` holding `cases/` beside `.takt/.gitignore`.
As with `.gitignore`, it leaves an existing one alone. A managed worktree's `finish --discard-cache --archive`,
`discard-cache` and `sweep` then keep every case record, and the archive images them and verifies them. takt edits no
root file of the repository and adds no copy verb of its own.

## Context

- A downstream trial (2026-10-27): `worktree finish --discard-cache --archive` reported `discarded .engineering/cases
  (IgnoredOutput, 0 MiB)` before archiving. Only an operator's copy saved the records.
- worktree deletes every ignored entry the root `.worktreekeep` does not keep (worktree-git `cache.rs:453`).
- Once `story:cases-live-under-dot-takt` lands, `.takt/` is ignored by its own `.takt/.gitignore`. Every
  managed-worktree run would then lose its case at cleanup (inferred).
- A resume from an archive is `worktree` restore, then `takt run --case <id>`.

## Acceptance

Each is a named conformance scenario run by `task check`.

1. `cases_kept::a_finished_tree_keeps_its_case`: drives the real `worktree` binary on a fixture. It opens a case, runs
   `worktree finish --discard-cache --archive`, and checks that the archive's patch holds every file under
   `.takt/cases/<id>/` and replays to the same bytes.
2. `cases_kept::an_existing_keep_file_is_left_alone`: a `.takt/.worktreekeep` with other content is not rewritten.
3. `cases_kept::no_root_file_is_touched`: the repository's root `.gitignore` and `.worktreekeep` are byte-identical
   after a run.

## Scope

- `crates/takt/src/case.rs` (state-root creation)
- `crates/takt/tests/cases_kept.rs`
- `ess/domains/takt.yaml` (the retained-state rule)

## Delivery

This depends on worktree `story:a-nested-keep-file-keeps-its-directory`, recorded as
`dependency-blocker:worktree-nested-keep-file`, and on `story:cases-live-under-dot-takt`.
- Scenario 1 runs against the locally built `worktree` from that story's working tree, on `PATH`. No intermediate
  release.
- Then worktree releases, and this story's check is re-run on the released binary.
