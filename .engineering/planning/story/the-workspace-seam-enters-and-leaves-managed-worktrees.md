---
format: aep.planning-md/3
id: story:the-workspace-seam-enters-and-leaves-managed-worktrees
kind: story
status: active
title: The dispatcher's Workspace seam creates, leases, archives and finishes managed worktrees
summary: continuous orchestration owns the outer lifecycle, no --worktree flag
tags:
- priority:medium
relations:
- serves: vision:safe-agentic-coding
- decomposes: epic:continuous-orchestration
- depends_on: story:declared-setup-runs-before-the-first-check
- informed_by: design:continuous-orchestration
scope:
- confidence: inferred
  path: crates/takt/src/case.rs
- confidence: inferred
  path: crates/takt/src/run.rs
- confidence: inferred
  path: crates/takt/src/workspace.rs
- confidence: inferred
  path: crates/takt/tests/workspace_seam.rs
- confidence: inferred
  path: ess/domains/takt.yaml
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-10-27T12:00:46Z", actor: "human:timo", revision: 5, decided_on: {"recorded":{"review_outcome":3}}}
- {from: "proposed", to: "active", at: "2026-10-27T12:00:46Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"review_outcome":3}}}
---
# Story: the-workspace-seam-enters-and-leaves-managed-worktrees

## Outcome

The continuous-orchestration dispatcher owns the outer lifecycle of a case's working tree through its `Workspace`
seam (`design:continuous-orchestration` § 6). `takt run` gets no `--worktree` flag.
- **On `case.dispatch`.** The seam runs `worktree create --purpose takt:<case>` and takes a lease. The child case works
  in that tree. Its records are written to the root's cases directory, never into the child's tree, as the design's
  "one cases root" says. `case.yaml` records `parent` and `worktree`.
- **On a non-terminal stop of a child** (any stop other than accepted or abandoned, for example a failed setup). The
  seam runs `worktree archive` and releases only its own lease. The child stays resumable in its tree.
- **On a terminal outcome** (accepted, abandoned). The seam runs `worktree finish --discard-cache --archive`. The
  child's records are unaffected, because they live at the root.
- Declared setup (`story:declared-setup-runs-before-the-first-check`) runs in each child's tree.
- The seam never runs `gc`, never force-removes, and never touches a tree it did not create.

The seam is called only through the `worktree` CLI.

## Context

- Operator decision, 2026-10-27: continuous orchestration owns entering and leaving managed worktrees, not a flag on
  `takt run`.
- A downstream trial (2026-10-27) did the whole enter/leave by hand: worktree, lease, base, setup, case copy and
  cleanup.
- `epic:continuous-orchestration` step 2 is the root case, `case.dispatch`, the children artifact and the Workspace
  seam. This story builds the seam together with the minimal `case.dispatch` it needs. Pause (step 7) is not needed
  here: a non-terminal stop is any stop that is not accepted or abandoned. No `continuous` or `Workspace` symbol
  exists in `crates/` yet.
- `story:a-cases-root-survives-worktree-cleanup` keeps the records of a single `takt run` started inside a tree
  somebody else made. The seam does not need it, because a child writes to the root.

## Acceptance

Each is a named conformance scenario run by `task check`, against the real `worktree` binary on a fixture repository.

1. `workspace_seam::a_dispatched_child_gets_a_leased_tree`: after dispatch, `worktree list --format json` shows one
   tree with purpose `takt:<case>` and a lease held by the run. The child's `case.yaml` under the root names `parent`
   and that tree's path as `worktree`.
2. `workspace_seam::a_stopped_child_is_archived_and_released`: a child stopped by a failing setup command has an
   archive and no lease of the run's, and `takt run --case <id>` resumes it in the same tree.
3. `workspace_seam::a_terminal_child_is_finished_and_its_records_stay_at_the_root`: after acceptance the tree is
   finished. The root's `.takt/cases/<id>/` holds every record, and the child's tree held none.
4. `workspace_seam::setup_runs_in_the_childs_tree`: a setup command writing `pwd` to its output records the child's
   tree path.
5. `workspace_seam::enter_is_idempotent`: a second dispatch of the same case reuses its tree.
6. `workspace_seam::a_foreign_tree_is_never_touched`: a tree created outside the seam is unchanged after a full run.

## Scope

- `crates/takt/src/workspace.rs` (new: the seam)
- `crates/takt/src/case.rs` (`parent`, `worktree` in `case.yaml`)
- `crates/takt/src/run.rs` (`case.dispatch`)
- `ess/domains/takt.yaml` (Workspace and dispatch, before code)
- `crates/takt/tests/workspace_seam.rs`
