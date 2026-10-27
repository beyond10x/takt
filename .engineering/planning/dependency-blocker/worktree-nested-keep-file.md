---
format: aep.planning-md/3
id: dependency-blocker:worktree-nested-keep-file
kind: dependency-blocker
status: open
title: worktree reads only the root .worktreekeep
relations:
- blocks: story:a-cases-root-survives-worktree-cleanup
revision: 2
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
