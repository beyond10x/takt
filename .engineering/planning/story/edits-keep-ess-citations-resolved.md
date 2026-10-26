---
format: aep.planning-md/3
id: story:edits-keep-ess-citations-resolved
kind: story
status: proposed
title: Edits keep ESS read-from citations resolved
summary: re-point citations after line shifts before the gate (trial run 7)
tags:
- loom-agent
relations:
- serves: vision:safe-agentic-coding
- decomposes: epic:takt-as-a-loom-agent
scope:
- confidence: inferred
  path: crates/takt/src/run.rs
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-10-26T13:48:10Z", actor: "human:timo", revision: 3}
---
# Story: edits-keep-ess-citations-resolved

## Outcome

A step that moves lines an `ess/` `read from:` citation points at leaves the citations resolved,
so the `citations` gate does not fail a delivery on line shifts alone.

## Context

- Trial run 7, steps 3 and 6: gate.run fail on gate `citations`, 24 `read from` citations of
  `crates/takt/src/git.rs` unresolved after the model's edits (`scripts/check-citations
  --require-digests` on the run's head: 1132 in-repository, 1108 resolved). The run stopped at
  `Iterations (8)` without a green gate; its draft pull request is
  https://github.com/beyond10x/takt/pull/17.

## Acceptance

1. After an edit step, citations whose cited text moved but did not change are re-pointed
   (`scripts/check-citations fix` or its equivalent) before the gate runs, and the re-point is part
   of the step's change.
2. A citation whose cited text changed stays unresolved and fails the gate, as today.

## Scope

- Inferred: `crates/takt/src/run.rs` (after an edit step), `crates/takt/src/validator.rs`,
  `protocols/`.
