---
format: aep.planning-md/3
id: story:resume-names-a-missing-branch
kind: story
status: implemented
title: Resuming a case whose branch is gone says how to recover
tags:
- finding
refs:
- provider: finding
  reference: resume-names-a-missing-branch
relations:
- informed_by: review-result:findings-hunt-33ab42c
- depends_on: story:abandon-a-case
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
scope:
- confidence: cited
  path: crates/takt/src/case.rs
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-11-05T02:14:03Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-05T02:14:03Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-11-06T03:10:31Z", actor: "human:timo", revision: 5, decided_on: {"recorded":{"test_result":1,"review_outcome":1,"verification":1}}}
---
## Outcome

`takt run --case <id>` on a case whose branch `takt/<id>` is gone exits 1 with takt's own message. The message names the missing branch, the last head the case recorded, and the next step: recreate the branch at that head, or abandon the case.

## Context

`Case::resume` runs `git checkout -q takt/<id>` and only adds context to the error (`case.rs:1528-1531`). Observed: exit 1, `takt: check out the branch of the case a: git checkout -q takt/a in <tree> exited exit status: 1: error: pathspec 'takt/a' did not match any file(s) known to git`. The case directory and its `branch.yaml` are still there, but nothing says how to recover.

```expected_before
scenario: 1
command: "$TAKT" run --repo . --intent a --case-id a --harness none --forge none --iterations 3 --test-cmd ./check.sh >/dev/null 2>&1; git checkout -q trial/0 && git branch -q -D takt/a && "$TAKT" run --repo . --case a --harness none --forge none --iterations 3 --test-cmd ./check.sh
exit: 1
output: did not match any file(s) known to git
source: .engineering/planning/review-result/findings-hunt-33ab42c.md
```

## Scope

`crates/takt/src/case.rs` (`Case::resume`).

## Acceptance

In the predicate's run, stderr names `takt/a` as missing and says how to recreate it or abandon the case. No git pathspec text appears, and nothing is written.

## Status

Built in wave 54 (unit c, 92aed883e, corrected in 7501e6380), released in 0.32.0 from 9e7a3904e, the merge of pull
request 61.

- `crates/takt/src/case.rs`: a resume whose case branch is gone exits 1 naming the branch, the last head the case
  recorded and where, and a shell-quoted `git branch` line that recreates it, before anything is written. A branch kept
  only on the one remote that holds it resumes, as git's checkout creates it (`checkout_source`).
- `crates/takt-cli/tests/resume_missing_branch_cli.rs` (red, then green); the adversary's cases in
  `w54_adversary_cli.rs` for the remote branch and the quoting.

Review: review-result:w54-adv-1, C1 and C2 fixed.
