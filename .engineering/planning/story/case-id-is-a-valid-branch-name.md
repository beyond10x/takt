---
format: aep.planning-md/3
id: story:case-id-is-a-valid-branch-name
kind: story
status: draft
title: A case id takt accepts is a valid git branch name
tags:
- finding
refs:
- provider: finding
  reference: case-id-is-a-valid-branch-name
relations:
- informed_by: story:cli-small-fixes
revision: 1
---
# Story: case-id-is-a-valid-branch-name

## Outcome

`takt run --case-id <id>` refuses, with takt's own message and before anything is written, every id whose case branch `takt/<id>` git would not accept as a branch name, instead of passing its own check and then failing inside `git checkout -b`.

## Context

Split out of story:cli-small-fixes, item (c), found by an independent code review of 2026-10-18 (`check_id`, `crates/takt/src/case.rs`), and reproduced by the planner on takt at main 9922f12 on scenario 1's seed: `takt run --repo <seed> --case-id 'a~b' --intent probe --harness none --forge none --iterations 1 --test-cmd ./check.sh` exits 1 with `takt: git checkout -q -b takt/a~b … exited exit status: 128: fatal: 'takt/a~b' is not a valid branch name`. It is the finding of the second recorded improvement turn: checkable offline by `scripts/check-finding`.

```expected_before
scenario: 1
command: "$TAKT" run --repo "$SCENARIO" --case-id 'a~b' --intent probe --harness none --forge none --iterations 1 --test-cmd ./check.sh
exit: 1
output: is not a valid branch name
source: .engineering/planning/story/cli-small-fixes.md
```

## Scope

- `crates/takt/src/case.rs`: `check_id` (inferred)

## Acceptance

`takt run --case-id 'a~b'` (and `foo.`, `a..b`, `a@{b`, a trailing `.lock`) exits 1 with a takt message naming the id and why git refuses it, creating no case directory and no branch; `scripts/check-finding` on this finding prints `reproduced` at 9922f12 and `not_reproduced` at the fix.
