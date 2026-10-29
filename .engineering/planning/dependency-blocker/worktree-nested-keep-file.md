---
format: aep.planning-md/3
id: dependency-blocker:worktree-nested-keep-file
kind: dependency-blocker
status: cleared
title: worktree reads only the root .worktreekeep
relations:
- blocks: story:a-cases-root-survives-worktree-cleanup
revision: 4
transitions:
- {from: "open", to: "cleared", at: "2026-10-28T19:04:22Z", actor: "human:timo", revision: 4, decided_on: {"recorded":{"review_outcome":1,"verification":1}}, executor: "agent:planner"}
---
# Dependency blocker: worktree-nested-keep-file

## What blocks

`story:a-cases-root-survives-worktree-cleanup` needs worktree to honour a `.worktreekeep` in a subdirectory. That is
worktree `story:a-nested-keep-file-keeps-its-directory` (worktree store, branch `plan/takt-trial-review`). Today only
the root file is read (worktree-git `cache.rs:453`).

## What clears it

Upstream state: the worktree story's named tests (`keep::*`) pass on its working tree, and the `worktree` binary built
from that tree is on `PATH` for takt's workspace. takt's scenario `cases_kept::a_finished_tree_keeps_its_case` is then
the end-to-end check, and the worktree release follows. No intermediate release is needed to clear this.

## Cleared

Cleared 2026-10-28 by worktree 0.19.0 (tag 0.19.0, commit 0f912020, GitHub Release published 2026-10-27). Its changelog: "A `.worktreekeep` in a subdirectory keeps paths at and below that directory", kept by `discard-cache`, `sweep` and `finish --discard-cache --archive`.
