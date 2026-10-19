---
format: aep.planning-md/3
id: story:story-delivery-tests-first
kind: story
status: active
title: A story case writes its tests first and opens the pull request once
tags:
- priority:high
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
- informed_by: story:pr-opens-when-change-is-done
scope:
- confidence: inferred
  path: crates/takt/src/validator.rs
- confidence: inferred
  path: ess/domains/takt.yaml
- confidence: inferred
  path: protocols/story-delivery/2.yaml
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-10-19T00:42:13Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-19T00:42:13Z", actor: "human:timo", revision: 4}
---
## Outcome

A story case writes its tests before its implementation, and opens the pull request once, on the complete change. `story.delivery/2` orders the steps: `tests.write`; a `tests.run` that fails at that revision, recorded as evidence that the new test detects the missing behaviour; `implementation.edit`, which requires that recorded failure; `tests.run` green; and `pr.open`, which requires a `change.complete` claim: test paths and implementation paths both changed since the base, with tests and gate green at the same revision. One push, one CI cycle.

## Context

Owner decision of 2026-10-19: "why is takt committing the code, then PR, and only after add the tests? its weird, change it!" Cause, read by the headstart session: `protocols/story-delivery/1.yaml` sets no order between `tests.write` and `implementation.edit`, and `pr.open` needs only `tests.pass` and `gate.green`; the seed's existing tests pass, so after `implementation.edit` the pull request is admissible and takt opens it before `tests.write`, and CI runs a second time after the tests are pushed (the headstart journey comparison: takt 72-75 s of forge time per run against 11-16 s for the free arm). Related: story:pr-opens-when-change-is-done (one CI cycle; the efficiency unit's `change.handover`/`change.read` in wave 12), story:ci-wait-watches-the-check.

## Scope

- `protocols/story-delivery/2.yaml` (new), the built-in protocol table (inferred)
- `crates/takt/src/validator.rs`: the failing-first `tests.run` claim and `change.complete` (inferred)
- `ess/domains/takt.yaml`: the new claims (inferred; spec first)

## Acceptance

On scenario 1 with a fake harness, a story case runs `tests.write` before `implementation.edit`; a `tests.run` between them records `fail` and is cited by the implementation's precondition; `pr.open` is offered only after both paths changed and tests and gate are green at one revision; the forge records one push. A harness that edits the implementation first is refused by the protocol, not after the fact.
