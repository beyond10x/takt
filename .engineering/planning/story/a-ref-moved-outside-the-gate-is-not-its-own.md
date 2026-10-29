---
format: aep.planning-md/3
id: story:a-ref-moved-outside-the-gate-is-not-its-own
kind: story
status: draft
title: A ref moved by another worktree is charged to the gate
summary: A validator step's guard charges every ref that moved while it ran, also one another worktree of the repository moved; the run stops StepRefused and the gate's result is lost.
tags:
- finding
- priority:medium
- rsi
refs:
- provider: finding
  reference: a-ref-moved-outside-the-gate-is-not-its-own
relations:
- serves: vision:safe-agentic-coding
revision: 1
---
## What is wrong

A validator step's guard takes every ref of the repository at the checkpoint (`guarded_refs` in
`crates/takt/src/git.rs`) and charges each one that moved by the observation's end to the step
(`wrote what its validator does not`, `crates/takt/src/run.rs`). A ref moved by another process
in another worktree of the same repository while the gate ran, such as a fetch moving
`refs/remotes/origin/*` or a new tag, is charged the same way: the run stops `StepRefused` and
the gate's result is discarded, though the gate's commands moved nothing. Found in wave 43, when
U6's `gate.run` was refused for `refs/remotes/origin/HEAD`, `refs/remotes/origin/main` and
`refs/tags/0.19.0`, written by a release made in another worktree while the gate ran; resumed on
the same case, the gate passed.

## Check

```expected_before
command: grep -rqE 'fn a_ref_moved_outside_the_(gate|step)' crates/takt/src crates/takt/tests crates/takt-cli/tests && { echo "a test pins a ref moved outside the step"; exit 0; } || { echo "no test pins a ref moved outside the step"; exit 9; }
exit: 9
healthy_exit: 0
output: no test pins a ref moved outside the step
```

## Outcome

A ref the step's own processes could not have written is not charged to it: a validator runs
with the repository's refs out of its reach, or the guard tells a write by the step's process
tree from one by another process, and a step that moved a guarded ref itself is still refused.

## Acceptance

1. The predicate above exits 0.
2. A test named `a_ref_moved_outside_the_gate_…`: a gate whose run sees another worktree of the
   same repository create a tag and move a remote-tracking ref records its result, and a gate
   whose own command creates a tag is still refused.
