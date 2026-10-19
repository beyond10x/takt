---
format: aep.planning-md/3
id: story:promote-binds-scorecard-and-merge
kind: story
status: implemented
title: takt promote installs only a merged revision, judged on runs of that revision's binary
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
- informed_by: review-result:main-fe93286-areas
scope:
- confidence: cited
  path: crates/takt/src/promotion.rs
- confidence: cited
  path: crates/takt/src/scorecard.rs
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-10-19T02:52:53Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-19T02:52:53Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-10-19T06:14:33Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"test_result":1}}}
---
## Outcome

`takt promote` refuses a revision that is not an ancestor of the mainline tip, and refuses a scorecard whose rows do not each name the binary digest of the revision being promoted (from the run record's `opened.binary`), naming the first row that does not; `--baseline-only` holds to the same row check.

## Context

From the review of main at 9cc158b on 2026-10-19: ten independent reviewers, one per area, each finding then fact-checked by a second reviewer against the code (review-result:main-fe93286-areas), findings P1.2 (confirmed) and X10.3 (confirmed): `Scorecard::load` (`scorecard.rs:613-625@9cc158b`) checks only the format, and rows never carry the opened event's binary (`events.rs:76`, `scorecard.rs:337-378`), so a card saved with `--revision <HEAD>` over runs of another binary is judged as the candidate's; `promotion.rs:1016-1096` checks HEAD, a clean tree and the scorecard, with no ancestry and no grant. story:self-improvement-protocol D3 enforces both inside a case only; story:scorecard-save-checks-binary covers the save side.

## Scope

- `crates/takt/src/promotion.rs`: the ancestry and row checks (cited)
- `crates/takt/src/scorecard.rs`: rows carry the binary digest (cited)

## Acceptance

`takt promote` on a clean branch tip that is not an ancestor of `main` exits non-zero naming `main`; on a merged revision with a scorecard whose runs name another binary digest it exits non-zero naming the row; with matching rows it proceeds as today.

## Landed (wave 13)

Read from `git diff --name-only 69b4d35^1 69b4d35` (the unit's merge into the wave branch) at the close of wave 13 (wave branch head 16ff976). takt promote refuses a revision not on origin's main and rows another binary ran; --rebaseline binds each row to the stored run. Shared merge with the three other E2 stories. Wave 13 E2.

- `crates/takt-cli/src/main.rs`
- `crates/takt-cli/tests/promote_cli.rs`
- `crates/takt-cli/tests/scorecard_adv1.rs`
- `crates/takt-cli/tests/scorecard_cli.rs`
- `crates/takt/src/promotion.rs`
- `crates/takt/src/scorecard.rs`
- `crates/takt/tests/scorecard.rs`
- `crates/takt/tests/scorecard_adv1.rs`
- `crates/takt/tests/scorecard_adv2.rs`
- `crates/takt/tests/trace.rs`
- `ess/domains/takt.citations.tsv`
- `ess/domains/takt.yaml`
