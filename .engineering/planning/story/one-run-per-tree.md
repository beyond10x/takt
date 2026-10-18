---
format: aep.planning-md/3
id: story:one-run-per-tree
kind: story
status: draft
title: Two takt runs on one tree are refused, not interleaved
tags:
- finding
refs:
- provider: finding
  reference: one-run-per-tree
relations:
- informed_by: review-result:findings-hunt-33ab42c
- depends_on: story:signal-releases-the-case
- decomposes: epic:composable-agentic-work
revision: 1
---
## Outcome

A second `takt run` on a tree where a run is in progress is refused with exit 1 before it opens or checks out anything. The first run's case, branch and HEAD stay as they were.

## Context

The only lock is per case, in `run.lock` (`case.rs:137-160`). The clean-tree check at open passes while the first run's step is still running. So run b opened case `b` and checked out `takt/b` underneath run a, which was in its `tests.run` step. Run a's step guard (`run.rs:3507` and the cases snapshot) then treated b's writes as its own validator's:
- it put back the cases directory and the refs, deleting `.engineering/cases/b/` and `refs/heads/takt/b`;
- it left `.git/HEAD` pointing at `refs/heads/takt/b`;
- run a stopped `StepRefused (tests.run wrote what its validator does not: .engineering/cases/b/…, .git/HEAD, refs/heads/takt/b from nothing to 865eaa8b)`, exit 3;
- run b exited 5 with `git rev-parse HEAD … unknown revision`.

Afterwards `git status` showed every tracked file as a staged addition on a branch with no commits. 3 out of 3 runs gave this. This is not story:case-scoped-step-guard, which is about a `takt approve` during a step. Here two runs open cases in one worktree and nothing serializes them.

```expected_before
scenario: 1
command: "$TAKT" run --repo . --intent a --case-id a --harness none --forge none --iterations 1 --test-cmd "sleep 3" >/dev/null 2>&1 & sleep 1; "$TAKT" run --repo . --intent b --case-id b --harness none --forge none --iterations 1 --test-cmd "sleep 3" >/dev/null 2>&1; wait; git rev-parse -q --verify HEAD >/dev/null || echo "HEAD names no commit: $(git symbolic-ref HEAD)"
exit: 0
output: HEAD names no commit: refs/heads/takt/b
source: .engineering/planning/review-result/findings-hunt-33ab42c.md
```

## Scope

`crates/takt/src/case.rs` (a tree-level hold taken before open, resume or take), `crates/takt/src/run.rs` (`open_unit`, `resume`, `take_next`), `crates/takt-cli/src/main.rs` (`scenario_run`, which checks out a trial branch before any hold).

## Acceptance

In the predicate's run, the second `takt run` exits 1 and says another run holds the tree. No case `b` and no branch `takt/b` are created. Run a's step is judged only on its own writes, and HEAD stays on `takt/a`.
