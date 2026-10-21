---
format: aep.planning-md/3
id: story:replay-offers-granted-actions
kind: story
status: implemented
title: takt replay rebuilds a step's offer with the grants the step had
summary: replay reads authority.yaml at the step's revisions, so a step taken after a grant replays; wave 29 fix of the governor unit's replay gap
tags:
- loom
relations:
- decomposes: epic:takt-on-loom-sdk
- serves: vision:safe-agentic-coding
- depends_on: story:governor-holds-takts-cases
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-21T18:44:12Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-21T18:44:12Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-10-21T22:46:01Z", actor: "human:timo", revision: 4, decided_on: {"recorded":{"test_result":1}}}
---
# Story: replay-offers-granted-actions

## Outcome

`takt replay` of a step taken after a grant rebuilds the same offer the step was given, and replays it, where today it refuses on a prompt mismatch.

## Context

- Since story:governor-holds-takts-cases (32a0085), each `decisions/NNNN.json` is the governor's `canon-decision/1` document. The governor supplies no authority, so the document finds a granted action approval-required. The grant reached the run through `CaseAuthority`, which reads `authority.yaml` (`crates/takt/src/case.rs:590-617`).
- `takt replay` reads the step's offer back from that document alone (`crates/takt/src/replay.rs:213-220`, then `judgement_offer` in `crates/takt/src/run.rs`). For a step taken after a grant, the granted action is missing from the rebuilt offer, the prompt differs from the recorded one, and replay refuses.
- The owner's grant of the wave 29 merge requires this gap to be fixed in wave 29 (decision-blocker:governor-judge-surface-merge).

```expected_before
scenario: 1
command: grep -c 'AUTHORITY_FILE\|CaseAuthority\|Authority::read' crates/takt/src/replay.rs
exit: 1
output: "0"
source: crates/takt/src/replay.rs:213-220
```

## Scope

- `crates/takt/src/replay.rs`
- `crates/takt/src/run.rs` (`judgement_offer`, read only unless the offer needs the grant passed in)
- `crates/takt/src/case.rs` (reading `authority.yaml` at the step's case revision)
- new: `crates/takt/tests/replay_after_grant.rs`
- `ess/domains/takt.yaml`, `ess/domains/takt.citations.tsv`

## Acceptance

1. `cargo test -p b10x-takt --test replay_after_grant` passes. A run whose step was offered an action only through a grant in `authority.yaml` replays that step to the same prompt digest and exits 0.
2. The same test shows that replay grants nothing the run was not granted: with the grant removed from `authority.yaml`, or given at another case revision or subject revision, replay rebuilds the offer without the action and refuses on the prompt mismatch.
3. Every existing replay test passes unchanged.
4. `ess specify validate --path ess` and `python3 -I scripts/check-citations --require-digests` pass.

## Out of Scope

A completion document from Loom (G1) and artifact revisions in Loom's authority request (G2).
