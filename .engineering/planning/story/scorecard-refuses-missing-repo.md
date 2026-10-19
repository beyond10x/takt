---
format: aep.planning-md/3
id: story:scorecard-refuses-missing-repo
kind: story
status: draft
title: takt scorecard refuses a --repo that does not exist
tags:
- finding
refs:
- provider: finding
  reference: scorecard-refuses-missing-repo
relations:
- informed_by: review-result:findings-hunt-3fe3f21
- decomposes: epic:recursive-self-improvement
revision: 1
---
## Outcome

A `--repo` that does not exist is refused with exit 1, and nothing is saved.

## Context

`Scorecard::read` treats a missing cases directory as "0 cases" (`scorecard.rs:421`), so a mistyped `--repo` prints an empty table and exits 0. The help says exit 1 when "the repository … does not read" (`crates/takt-cli/src/main.rs:81`). With `--save`, the empty scorecard is written. It cannot be overwritten later, and comparing against it reports every scenario `no baseline`, so it is never worse. Observed: exit 0, `saved: …/scorecard-3fe3f21.json`, `"rows": []`.

```expected_before
scenario: 1
command: "$TAKT" scorecard --repo ./no-such-repo --save "$HOME/scorecards" --revision 8378cb2
exit: 0
output: saved: 
source: .engineering/planning/review-result/findings-hunt-3fe3f21.md
```

## Scope

`crates/takt/src/scorecard.rs` (`read`, `report`).

## Acceptance

A `--repo` that is not an existing directory exits 1, names the path and writes no file. An existing repository with no `.engineering/cases/` still prints 0 cases.
