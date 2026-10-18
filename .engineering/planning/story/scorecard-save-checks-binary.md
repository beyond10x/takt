---
format: aep.planning-md/3
id: story:scorecard-save-checks-binary
kind: story
status: draft
title: takt scorecard --save checks the runs' binary against --revision
tags:
- finding
refs:
- provider: finding
  reference: scorecard-save-checks-binary
relations:
- informed_by: review-result:findings-hunt-3fe3f21
revision: 1
---
## Outcome

`takt scorecard --save --revision R` is refused unless every run it saves has an `opened` line whose `binary` is R. Runs from other binaries are refused or listed separately.

## Context

`report` and `save` (`scorecard.rs:750-754`, `:550`) take the operator's `--revision` on trust. `runs()` never reads `binary` (`:313-347`). In the observed run, the `opened` line recorded `"binary":"3fe3f219…"`, the save as `0000000` was accepted, and the output said `named by the revision of takt under test`. This is not the filed `--against` provenance finding: that one is about reading a saved file, this one is about writing it.

```expected_before
scenario: 1
command: "$TAKT" run --repo . --harness none --forge none --test-cmd true --iterations 1 >/dev/null 2>&1; "$TAKT" scorecard --repo . --save "$HOME/scorecards" --revision 0000000
exit: 0
output: named by the revision of takt under test
source: .engineering/planning/review-result/findings-hunt-3fe3f21.md
```

## Scope

`crates/takt/src/scorecard.rs` (`runs`, `Row`, `save`, `report`).

## Acceptance

Saving under a revision that differs from a run's `binary` (an `unknown revision` binary included) exits 1 and names the case. Saving under the matching sha succeeds.
