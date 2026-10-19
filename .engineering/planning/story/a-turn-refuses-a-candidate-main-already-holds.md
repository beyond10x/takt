---
format: aep.planning-md/3
id: story:a-turn-refuses-a-candidate-main-already-holds
kind: story
status: active
title: A self-improvement turn refuses at open a candidate the mainline already holds
summary: the first real turn's candidate was merged before its grant and the turn still opened on it
tags:
- rsi
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
scope:
- confidence: inferred
  path: crates/takt-cli/tests/self_improvement_cli.rs
- confidence: cited
  path: crates/takt/src/case.rs
- confidence: inferred
  path: crates/takt/src/improvement.rs
- confidence: cited
  path: ess/domains/takt.citations.tsv
- confidence: cited
  path: ess/domains/takt.yaml
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-10-19T22:44:32Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-19T22:44:33Z", actor: "human:timo", revision: 3}
---
# Story: a-turn-refuses-a-candidate-main-already-holds

## Outcome

`takt improve` refuses to open a turn on a candidate revision the mainline's tip already contains,
naming the revision and what to do instead, so a candidate merged before its grant is caught at open.

## Context

Found by the review of story:a-judge-surface-candidate-merges-after-its-grant (2026-10-19): on
2026-10-19 the first real turn's candidate reached main before the owner's grant, and the turn
opened on it anyway; nothing at open compares the candidate with the mainline. The refusal message
should name a remedy (open on a fresh candidate, or record the incident).

## Acceptance

- A turn opened on a candidate that is an ancestor of the mainline's tip is refused before any step,
  naming both revisions and the remedy: a test.
- A candidate the mainline does not contain opens as today.
