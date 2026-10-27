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
  path: Cargo.toml
- confidence: inferred
  path: crates/takt/src/run.rs
- confidence: inferred
  path: crates/takt/src/validator.rs
- confidence: inferred
  path: ess/domains/takt.yaml
revision: 9
transitions:
- {from: "draft", to: "proposed", at: "2026-10-19T00:42:13Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-19T00:42:13Z", actor: "human:timo", revision: 4}
---
## Outcome

A story case writes its tests before its implementation, and opens the pull request once, on the complete change. `story.delivery/2` orders the steps: `tests.write`; a `tests.run` that fails at that revision, recorded as evidence that the new test detects the missing behaviour; `implementation.edit`, which requires that recorded failure; `tests.run` green; and `pr.open`, which requires a `change.complete` claim: test paths and implementation paths both changed since the base, with tests and gate green at the same revision. One push, one CI cycle.

## Context

Owner decision of 2026-10-19: "why is takt committing the code, then PR, and only after add the tests? its weird, change it!" Cause, read by the headstart session: `protocols/story-delivery/1.yaml` sets no order between `tests.write` and `implementation.edit`, and `pr.open` needs only `tests.pass` and `gate.green`; the seed's existing tests pass, so after `implementation.edit` the pull request is admissible and takt opens it before `tests.write`, and CI runs a second time after the tests are pushed (the headstart journey comparison: takt 72-75 s of forge time per run against 11-16 s for the free arm). Related: story:pr-opens-when-change-is-done (one CI cycle; the efficiency unit's `change.handover`/`change.read` in wave 12), story:ci-wait-watches-the-check.

## Scope

- engineering-protocols `story:story-delivery-2-with-settings`: `protocols/story-delivery/2.yaml` with the inputs `tests_first` and `gate_on`. Protocols are engineering-protocols built-ins since 0.10.0 (takt `Cargo.toml:13`), so the protocol file is not in this repository. The edge is `dependency-blocker:story-delivery-2-with-settings` (amended 2026-10-27).
- `crates/takt/src/validator.rs`: the failing-first `tests.run` claim and `change.complete` (inferred)
- `crates/takt/src/run.rs`: the printed and recorded gate deferral (inferred)
- `ess/domains/takt.yaml`: the new claims (inferred; spec first)
- `Cargo.toml`: the engineering-protocols pin, taken as a pin-only hop after the end-to-end check on an uncommitted `[patch]` path override. No intermediate release.

## Acceptance

On scenario 1 with a fake harness, a story case runs `tests.write` before `implementation.edit`. A `tests.run` between them records `fail` and is cited by the implementation's precondition. `pr.open` is offered only after both paths changed and tests and gate are green at one revision, and the forge records one push. A harness that edits the implementation first is refused by the protocol, not after the fact.

Added 2026-10-27, from a downstream trial in which the gate ran three times per case, once on a known-red revision:

- `gate_deferral::a_red_revision_runs_no_gate`: under `gate_on: after_handover` (the default of `story:takt-ships-default-protocol-settings`), the gate runs once per case, on the handed-over revision. Each deferral is printed as `gate.run: deferred (precondition ... not true at <rev>)` and recorded as an event, never as a pass.
- `protocol_settings::tests_optional_keeps_todays_order`: `tests_first: optional`, or `protocol: story.delivery/1`, keeps today's order.
