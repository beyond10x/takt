---
format: aep.planning-md/3
id: story:approval-record-reaches-base
kind: story
status: active
title: The approval record reaches the base's planning store
summary: An approval committed on the case branch after the granted revision is lost at merge
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
scope:
- confidence: inferred
  path: crates/takt/src/frontier.rs
- confidence: inferred
  path: crates/takt/src/validator.rs
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-10-18T12:07:50Z", actor: "human:timo", revision: 4}
- {from: "proposed", to: "active", at: "2026-10-18T12:07:50Z", actor: "human:timo", revision: 5}
---
# Story: approval-record-reaches-base

## Outcome

After a case merges under a grant, the base branch's planning store holds the approval record for the story, the way it holds the story's other evidence.

## Context

Found by the adversary pass over wave 06's integration fixes (`review-result:adversary-integration-pass-1`, J1): `takt approve` commits its store record on the case branch after the granted revision (`crates/takt/src/frontier.rs:257` at e48695d), and the bot route merges the granted revision, so the approve commit and its record never reach the base. Before wave 06 the record was not committed at all.

## Scope

- `crates/takt/src/frontier.rs` (`approve`, `commit_record`) (inferred)
- `crates/takt/src/validator.rs` (`PrMerged`, after the merge) (inferred)

## Acceptance

With the forge double, a case approved and merged leaves the story's approval evidence (approver, capability, revision) in the base branch's planning store after the run, and the merged revision is still the granted one.
