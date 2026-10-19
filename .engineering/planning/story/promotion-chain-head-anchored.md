---
format: aep.planning-md/3
id: story:promotion-chain-head-anchored
kind: story
status: implemented
title: Removing the latest promotion record is detected
tags:
- priority:high
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
scope:
- confidence: inferred
  path: crates/takt-cli/tests/promote_cli.rs
- confidence: cited
  path: crates/takt/src/promotion.rs
- confidence: inferred
  path: ess/domains/takt.citations.tsv
- confidence: inferred
  path: ess/domains/takt.yaml
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-10-19T06:11:34Z", actor: "human:timo", revision: 6}
- {from: "proposed", to: "active", at: "2026-10-19T06:11:34Z", actor: "human:timo", revision: 7}
- {from: "active", to: "implemented", at: "2026-10-19T14:11:44Z", actor: "human:timo", revision: 8, decided_on: {"recorded":{"test_result":1}}}
---
## Outcome

`takt promote` detects a promotion record chain whose latest record was removed or replaced: the chain's head (the latest record's name and sha256) is kept outside the record directory, where a candidate binary's runs do not write, and a chain that ends before that head is refused, naming the missing record.

## Context

From the grant review of the wave-13 promotion stories (E2, 2026-10-19), finding W4: nothing anchors the last record of the chain (`crates/takt/src/promotion.rs:334-440`); deleting the latest `<rev>.refused.json` and promoting again without `--rejudge` exits 0, so a refusal can be undone by deleting one file. Already true before wave 13. Every record but the last is bound by the next one's `previous_sha256`.

## Scope

Derived 2026-10-19 by `story-scoper`, read-only at `wave/2026-10-19-w13` bbe5bdb (`promotion.rs` identical to 2ad69b0). Every line is **cited** (read from the story or the tree) or **inferred** (a reading that could be wrong).

- **Primary surface:** `crates/takt/src/promotion.rs`, the record chain — cited
- **Files:** `crates/takt/src/promotion.rs:395-532@2ad69b0` (`records`), `:726-781` (`chained`), `:969-985` (`write`), `:989-1007` (`unwrite`), `:1022-1044` (`lock`), `:292-317` (`store`, `state_dir`) — cited
- **Files:** write sites in `promote` (2194, 2268, 2301-2303) and `rebaseline` (2510); the module doc at 13-16 says a record removed from the end is noticed, which `installed_names_latest` (1603-1664) does only for baselines and promotions, never refusals — cited
- **Symbols:** `records`, `chained`, `write`, `unwrite`, `record_path`, `previous_sha256`, `installed_names_latest`, `STORE` — cited
- **Also likely:** `crates/takt-cli/tests/promote_cli.rs` — inferred
- **Also likely:** `ess/domains/takt.yaml:2960-2972,3042-3110` (`takt.work.Promotion`, the chain rule) and `ess/domains/takt.citations.tsv`; the head is a new noun and goes into `ess/` first (`AGENTS.md:95-97`) — inferred
- **Confidence:** medium
- **Would collide with:** any unit touching the record read and write in `promotion.rs`, the promote CLI tests, or the `Promotion` rows of `ess/`; story:scorecard-binds-binary-digest and story:rejudge-is-bounded land on `promotion.rs` too
- **Safety fact:** every record reaches the store through `write` (4 call sites) under the store lock and is undone only through `unwrite`, so the head is updated in exactly two places — level 2, unproven

## Planner's decisions at scoping (2026-10-19)

- D1: the head (the latest record's file name and sha256) is one file beside the record directory, `<state>/takt/promotions.head`, written by `write` and `unwrite` under the store lock.
- D2: a store with records and no head is read once on trust: the head is written from the chain as it stands, and the run says so on stderr; every later read requires the head.
- D3: `--dry-run` checks the head like every other read.
- D4: the limit is stated in the ESS and the module doc: the head detects a record removed or replaced by accident or by a tool; a process running as the same user can rewrite both.

## Acceptance

After a refusal, deleting the latest record and running `takt promote` on the same revision without `--rejudge` exits 1 naming the missing record; replacing it with an older record's bytes is refused the same way.
