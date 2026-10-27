---
format: aep.planning-md/3
id: dependency-blocker:worktree-nested-keep-file
kind: dependency-blocker
status: open
title: worktree reads only the root .worktreekeep
relations:
- blocks: story:a-cases-root-survives-worktree-cleanup
revision: 1
---
# Dependency blocker: worktree-nested-keep-file

## What blocks

`story:a-cases-root-survives-worktree-cleanup` needs worktree to honour a `.worktreekeep` in a subdirectory. That is
worktree `story:a-nested-keep-file-keeps-its-directory` (worktree store, branch `plan/takt-trial-review`). Today only
the root file is read (worktree-git `cache.rs:453`).

## What clears it

`cases_kept::a_finished_tree_keeps_its_case` passes against the `worktree` binary built from that story's working tree
on `PATH`. The worktree release follows. No intermediate release is needed to clear this.
