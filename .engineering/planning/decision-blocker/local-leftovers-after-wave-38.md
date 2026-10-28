---
format: aep.planning-md/3
id: decision-blocker:local-leftovers-after-wave-38
kind: decision-blocker
status: cleared
title: Local trees, a stash and unpushed branches hold work nobody has saved; deleting them is the operator's call
summary: 'Decider: operator. A delete, B keep, C bundle then remove (recommended)'
revision: 4
transitions:
- {from: "open", to: "cleared", at: "2026-10-28T15:31:29Z", actor: "human:timo", revision: 4}
---
# Blocker: local leftovers that hold uncommitted or unpushed work

Decider: operator. Deleting data is outside the orchestrator's grant.

Read from the primary checkout on 2026-10-28 (`git worktree list`, `git status --porcelain` in
each tree, `git stash list`, `git rev-list --count origin/main..<branch>`, `git cherry`):

| leftover | state |
|---|---|
| 13 harness sub-agent trees under `.claude/worktrees/agent-*`, from 2026-10-22 and 2026-10-23 | each has 1 to 25 changed files; each base commit is on `main`; no process has its working directory in one |
| `stash@{0}`, "wave-38 U1-U8 state" | the stories it carried are `implemented`; the commit it was cut from, 2db3e1d, is on `integration/takt` |
| 8 local branches with no remote: `trial-cand/2026-10-20-8eb9b3a5-1`, `trial-cand/2026-10-20-bee77822-1`, `trial-cand/2026-10-20-fd395d22-1`, `trial/w5`, `site-fix-pages`, `headstart/tests-first`, `takt/story-run-record-and-story-evidence-20261018T090034Z`, `wave/37` | each has 1 to 14 commits `main` does not; of `wave/37`'s 14, 2 have no patch on `main` (`git cherry`) |

## Options

| option | what it does |
|---|---|
| A | delete them as they are |
| B | keep them as they are |
| C (recommended) | write each tree's diff, the stash and each branch into a git bundle in a local archive directory outside the repository, then remove them |

Nothing is removed until the operator answers.

## Decision (2026-10-28)

The operator, 2026-10-28: option A, "A delete". Done the same day: 13 trees removed with
`git worktree remove --force`, `stash@{0}` dropped (a979d54), 8 branches deleted with
`git branch -D`. No tree had a process in it; no branch in the list had a remote.
