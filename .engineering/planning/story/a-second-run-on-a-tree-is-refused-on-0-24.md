---
format: aep.planning-md/3
id: story:a-second-run-on-a-tree-is-refused-on-0-24
kind: story
status: draft
title: A second run on a tree is refused before it deletes the first run's case
summary: a tree-level hold before open, resume, take and scenario checkout
tags:
- finding
- priority:medium
- trial-0.24.0
relations:
- informed_by: story:one-run-per-tree
revision: 1
---
## Outcome

A second `takt run` (or resume, take or scenario checkout) on a tree where a run is in progress
is refused with exit 1 before it opens or checks out anything, and the first run's case and
branch stay as they were.

## Context

story:one-run-per-tree (draft) found this on `.engineering/cases`; the 0.24.0 trial cycle's
run-loop review reproduced it on 0.24.0 with `.takt/cases`. Run 1 was in a harness step; run 2
(`takt run --intent …`) opened its case on the same tree and checked out its branch. Run 1's step
undo put the tree back to its checkpoint, deleting run 2's branch and emptying run 2's case
directory (`case.yaml`, `events.jsonl`, `branch.yaml` and its `tests.run` evidence gone); run 2
exited 5 with `git rev-parse --abbrev-ref HEAD … unknown revision`.

Where, at 0.24.0: the only lock is per case (`crates/takt/src/case.rs:513-541`, `take_hold`);
the clean-tree check at open (`case.rs:3088-3101`) passes while the other run's step works in its
scratch copy; the step checkpoint puts back refs and the cases directory wholesale
(`crates/takt/src/git.rs:1895-1909`). In a pod with one volume tree and a restart policy, two
generations can overlap.

```expected_before
command: with run 1 in a harness step, takt run --repo <same tree> --intent x; then git branch --list 'takt/takt-*' and ls .takt/cases/<second id>/events.jsonl
output: the second run exits non-zero, the branch list is empty and events.jsonl does not exist
```

## Acceptance

1. A tree-level hold (`.takt/tree.lock`, created new, holding the pid and boot id) is taken
   before open, resume, take and scenario checkout, and released on every exit path.
2. A second run while it is held exits 1 naming the holder, and changes nothing in the tree.
3. A hold whose holder is dead (pid gone, or another boot id) is taken over, and the takeover is
   recorded.
4. Implementing this closes story:one-run-per-tree too.
