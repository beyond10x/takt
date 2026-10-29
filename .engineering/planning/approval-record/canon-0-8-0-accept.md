---
format: aep.planning-md/3
id: approval-record:canon-0-8-0-accept
kind: approval-record
status: accepted
title: Accepts task canon-0-8-0 for one wave and its release (D212)
summary: The Canon 0.8.0 move with engineering-protocols 0.14.0 and Loom 0.50.0, the whole-second wait removed
tags:
- wave-approval
relations:
- decides: task:canon-0-8-0
revision: 2
transitions:
- {from: "draft", to: "accepted", at: "2026-10-29T14:46:23Z", actor: "human:timo", revision: 2}
---
Decider: operator

Accepts task:canon-0-8-0 for one wave and its release (D212): Canon 0.7.0 -> 0.8.0, with
engineering-protocols 0.14.0 and Loom 0.50.0 moved together so the lock holds one Canon; the
whole-second wait (`now_after`, `NOW_AFTER_BOUND`, `second_for`'s wait) goes; `tests_change` and
`test_result` are written at one fractional precision; `Predicate::Since` is handled.

Quoted from the orchestrator's item, 2026-10-29, under the operator's standing grant for class-C
items:

- "D212: takt wave for task:canon-0-8-0 and its release. Approved under the grant."
- "Key behaviour to prove: with sub-second timestamps at one precision, a red run 500 ms after a
  tests write in the same second admits the edit under story.delivery/2 (ep 0.14.0), with no
  waiting."
- "do NOT move the ESS pin" (the operator, 2026-10-29).
