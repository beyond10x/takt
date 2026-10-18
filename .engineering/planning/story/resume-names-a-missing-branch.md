---
format: aep.planning-md/3
id: story:resume-names-a-missing-branch
kind: story
status: draft
title: Resuming a case whose branch is gone says how to recover
tags:
- finding
refs:
- provider: finding
  reference: resume-names-a-missing-branch
relations:
- informed_by: review-result:findings-hunt-33ab42c
revision: 1
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
