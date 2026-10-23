---
format: aep.planning-md/3
id: story:sibling-git-dirs-are-sealed
kind: story
status: draft
title: A test command cannot rewrite another worktree's git directory
relations:
- serves: vision:safe-agentic-coding
revision: 1
---
# A test command cannot rewrite another worktree's git directory

## Why

The grant review of wave 35 at 3e9aa15 (review-result:wave-35-3e9aa15) found that the guard and the
Seatbelt seal cover a linked worktree's `info` and `config.worktree`, but not the files that tell
git where a worktree's repository is. A test command that rewrites them points a later unconfined
git command at a config the command wrote. All four findings are pre-existing; none came from wave
35's diff.

## Findings

- `finding:sibling-commondir-is-writable` (medium, all platforms, inferred: the git behaviour was
  not run). `crates/takt/src/git.rs:1493-1558` and `crates/takt/src/seatbelt.rs:296-340` guard and
  seal neither `<common>/worktrees/<sibling>/commondir` nor `gitdir`. A test command rewrites
  `commondir` to a directory in its own tree holding a config with `core.fsmonitor`; the next
  unconfined git command in the sibling worktree runs that program. `expected_before`: a test
  command that writes a sibling's `commondir` leaves the step recorded, not refused, and the file
  changed after the run.
- `finding:linked-common-dir-can-be-moved` (low, macOS, inferred: Seatbelt rename behaviour not
  run). `crates/takt/src/seatbelt.rs:327-338` seals neither the common dir nor the tree's own git
  dir as a literal in a linked worktree, where `:311` seals the main worktree's `.git`.
  `expected_before`: on darwin, a test command running `mv <common> <tree>/x` exits 0 and
  `<common>` no longer exists.
- Note (low, macOS): `crates/takt/src/seatbelt.rs:391-392`, a hard link in the tree to a sealed git
  config or hook may pass the literal seal; the cross-platform content guard restores the file. No
  machine predicate until Seatbelt's treatment of hard links is observed.
- Note (low): `.github/workflows/darwin.yml:15-17`, the concurrency group of a release backfill
  (workflow_dispatch on main) collides with a manual darwin dispatch on main; `cancel-in-progress`
  cancels one, and the release fails closed.

## Acceptance

1. A test step that writes a sibling worktree's `commondir` or `gitdir` is refused, and the file is
   restored; a test in `crates/takt/tests` shows it on Linux, and on darwin through `darwin.yml`.
2. On darwin, a test command cannot rename or move the common dir of a linked worktree.

## Scope

`crates/takt/src/git.rs`, `crates/takt/src/seatbelt.rs`, `crates/takt/tests/`.
