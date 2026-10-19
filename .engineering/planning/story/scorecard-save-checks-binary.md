---
format: aep.planning-md/3
id: story:scorecard-save-checks-binary
kind: story
status: active
title: takt scorecard --save checks the runs' binary against --revision
tags:
- finding
refs:
- provider: finding
  reference: scorecard-save-checks-binary
relations:
- informed_by: review-result:findings-hunt-3fe3f21
- depends_on: story:check-finding-builds-name-their-revision
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
scope:
- confidence: inferred
  path: crates/takt-cli/tests/promote_cli.rs
- confidence: inferred
  path: crates/takt-cli/tests/scorecard_adv1.rs
- confidence: inferred
  path: crates/takt-cli/tests/scorecard_cli.rs
- confidence: cited
  path: crates/takt/src/scorecard.rs
- confidence: inferred
  path: ess/domains/takt.citations.tsv
- confidence: inferred
  path: ess/domains/takt.yaml
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-10-19T06:11:33Z", actor: "human:timo", revision: 5}
- {from: "proposed", to: "active", at: "2026-10-19T06:11:33Z", actor: "human:timo", revision: 6}
---
## Outcome

`takt scorecard --save --revision R` is refused unless every run it saves has an `opened` line whose `binary` is R. Runs from other binaries are refused or listed separately.

## Context

`report` and `save` (`scorecard.rs:750-754`, `:550`) take the operator's `--revision` on trust. `runs()` never reads `binary` (`:313-347`). In the observed run, the `opened` line recorded `"binary":"8378cb29…"`, the save as `0000000` was accepted, and the output said `named by the revision of takt under test`. This is not the filed `--against` provenance finding: that one is about reading a saved file, this one is about writing it.

```expected_before
scenario: 1
command: "$TAKT" run --repo . --harness none --forge none --test-cmd true --iterations 1 >/dev/null 2>&1; "$TAKT" scorecard --repo . --save "$HOME/scorecards" --revision 0000000
exit: 0
output: named by the revision of takt under test
source: .engineering/planning/review-result/findings-hunt-3fe3f21.md
```

## Scope

Derived 2026-10-19 by `story-scoper`, read-only at `wave/2026-10-19-w13` bbe5bdb (`scorecard.rs`, `promotion.rs` identical to 2ad69b0). Every line is **cited** (read from the story or the tree) or **inferred** (a reading that could be wrong).

- **Primary surface:** `crates/takt/src/scorecard.rs` — cited
- **Files:** `crates/takt/src/scorecard.rs:1122-1150@bbe5bdb` (`report`: the save and the "named by the revision of takt under test" line at 1135-1143) and `:810-845` (`Scorecard::save`); the story's earlier line numbers are stale — cited
- **Symbols:** `report`, `Scorecard::save`, `Row.opened_binary` (127-131), `runs` (497-541), `is_revision` (590-596) — cited
- **Already covered by wave 13 (E2, 2ad69b0):** `runs()` reads each `opened` line's binary into `Row.opened_binary` (`scorecard.rs:509-511`), and `takt promote` refuses runs of another binary (`runs_judged`, `promotion.rs:1511-1537`; stated baselines through `runs_stating`, 1755-1798). What remains is the check at save — cited
- **Also likely:** `crates/takt-cli/tests/scorecard_cli.rs` (fixture `run` at 78-98 writes `binary: None`), `crates/takt-cli/tests/scorecard_adv1.rs` (71-78, 138, 170), `crates/takt-cli/tests/promote_cli.rs:391-406,440-465,556-583` (`World::save` saves cards of mismatched or no-revision binaries through the CLI) — inferred
- **Also likely:** `ess/domains/takt.yaml:885-888` (`ScorecardId` states the save rule) and `ess/domains/takt.citations.tsv` — inferred
- **Confidence:** medium
- **Would collide with:** any unit touching `scorecard.rs` `save`/`report`/`Row`, the CLI scorecard and promote fixtures, or the `ScorecardId`/`ScorecardRow` rows of the citations; story:scorecard-binds-binary-digest lands on the same files and follows this story
- **Safety fact:** the check reads only `Row.opened_binary`, which `runs()` already fills — level 2, unproven

## Planner's decisions at scoping (2026-10-19)

- D1: the check sits in `Scorecard::save`, so every caller gets it.
- D2: a scorecard whose rows name a revision other than `--revision` is refused naming the run and both revisions; rows that name no revision are accepted only when every row names none (a binary built before `--version` named its revision, the path `runs_stating` accepts); a mix is refused.
- D3: `--revision` for a save is the full 40-hex commit id, compared exactly with `opened_binary`; the Acceptance's `--revision 0000000` reads as a full id of zeros.

## Acceptance

Saving under a revision that differs from a run's `binary` (an `unknown revision` binary included) exits 1 and names the case. Saving under the matching sha succeeds.
