---
format: aep.planning-md/3
id: story:grant-binds-to-action-subject
kind: story
status: implemented
title: A grant binds to the revision of the artifact its action is about
summary: grants read artifacts.implementation only, so a self.improvement/1 case cannot pass its merge gate
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
scope:
- confidence: cited
  path: crates/takt/src/case.rs
- confidence: inferred
  path: crates/takt/tests/grant_subject.rs
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-10-18T15:28:21Z", actor: "human:timo", revision: 5}
- {from: "proposed", to: "active", at: "2026-10-18T15:28:21Z", actor: "human:timo", revision: 6}
- {from: "active", to: "implemented", at: "2026-10-18T18:21:50Z", actor: "human:timo", revision: 8, decided_on: {"recorded":{"test_result":1,"review_outcome":1}}}
---
# Story: grant-binds-to-action-subject

## Outcome

A grant is bound to the revision of the artifact the granted action is about, read from the protocol, not to an artifact that must be named `implementation`. A case on `self.improvement/1` can be granted `repository.merge.judge_surface` at its `candidate` revision, and the grant authorises nothing once the candidate moves.

## Context

Observed by the planner on 2026-10-18 while recording the first improvement turn (story:scenario-run-help-exit-codes) as a `self.improvement/1` case in phase R0. With `finding`, `baseline` and `candidate` set to real revisions (eb54f3a, 0a776d2, 447423c) and three records held, Canon returned `scenario.reproduced_before: true`, `scenario.passes_after: true` and `candidate.merge_judge_surface: approval-required`. The grant was refused: `the case turn-1-scenario-help has no implementation revision for a grant of repository.merge.judge_surface to authorise`.

Grants read the revision from `artifacts.implementation` only: `decided_at` (`crates/takt/src/case.rs:439-445`) and `implementation_of` (`case.rs:447-455`), and `self.improvement/1` declares no `implementation` artifact (its artifacts are `finding`, `baseline`, `candidate`, `promotion`). So no improvement turn can pass its merge gate, and the RSI chain's first recorded turn (story:first-recorded-improvement-turn) cannot reach `accepted` until this changes. story:self-improvement-protocol names it under "Not established"; this story lands it on its own, early, because it is small and every recorded turn waits on it.

## Expected before

On takt at main, holds; after the fix, fails:

- A test that opens a case on `protocols/self-improvement/1.yaml`, sets `candidate` and records the three records above, then calls `takt::case::grant(tree, id, "repository.merge.judge_surface", "agent:planner")`: it returns the error `has no implementation revision`.

## Scope

- `crates/takt/src/case.rs`: `decided_at`, `implementation_of`, `prepare_grant`, `Authority::grants_at` (cited)
- `crates/takt/tests/`: a grant test on `self.improvement/1` (inferred)

## Acceptance

On a `self.improvement/1` case at the merge gate, `grant` of `repository.merge.judge_surface` records the grant at the `candidate` revision and the next decision makes `candidate.merge_judge_surface` admissible; after `candidate` moves to another revision the grant authorises nothing; a `story.delivery/1` case is granted at its `implementation` revision exactly as today.

## Landed (wave 10)

Read from `git diff --stat 93bd7c3^1 93bd7c3` (the unit's merge into the wave branch) at the close of wave 10 (wave branch head 8858b49). The grant's subject is read from the protocol's precondition evidence; a grant naming no subject, or more than one, is refused. Granted by the owner. Low reviewer findings carried by story:judge-guard.

- `crates/takt/src/case.rs`
- `crates/takt/src/frontier.rs`
- `crates/takt/src/protocol.rs`
- `crates/takt/tests/grant_subject.rs`
- `crates/takt/tests/grant_subject_adv1.rs`
- `ess/domains/takt.citations.tsv`
- `ess/domains/takt.yaml`
