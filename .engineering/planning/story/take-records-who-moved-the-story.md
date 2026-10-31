---
format: aep.planning-md/3
id: story:take-records-who-moved-the-story
kind: story
status: draft
title: takt records who moved a story to active when it takes it
summary: the take's actor is agent:takt or AEP_ACTOR, never human:unknown
tags:
- finding
- priority:low
relations:
- informed_by: review-result:pr-49-grant-review
revision: 1
---
## Outcome

When `takt run` takes a story, the store records the move to active with a named actor, never `human:unknown`.

## Context

The move pull request 49 carried, at `d4ce8808`, reads:

`{from: "proposed", to: "active", at: "2026-10-31T04:07:18Z", actor: "human:unknown", revision: 4, executor: "agent:takt"}`

No person made this move; a takt run in a pod did. `Aep::take` (`crates/takt/src/frontier.rs:680-690` at `d4ce8808`) runs `aep plan artifact move <id> --to active --executor agent:takt` through `plan`, which sets no `AEP_ACTOR`. Inside a pod, with no person configured, `aep` therefore records the actor as `human:unknown`. `record_approval` on the same type goes through `plan_as` and sets `AEP_ACTOR` to the approver (`frontier.rs:697-720`). The cause is inferred from the code. The record itself is observed.

Found in the grant review of pull request 49 (review-result:pr-49-grant-review).

## Scope

`crates/takt/src/frontier.rs` (`Aep::take`), and the test that covers it.

## Acceptance

A take records `actor` as `agent:takt`, or as the actor the caller named in `AEP_ACTOR` when it named one, and never `human:unknown`. A test runs `take` against a scratch store with no `AEP_ACTOR` and no git identity, then reads the new transition's `actor`.
