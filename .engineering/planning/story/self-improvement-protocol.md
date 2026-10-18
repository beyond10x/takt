---
format: aep.planning-md/3
id: story:self-improvement-protocol
kind: story
status: draft
title: Validators drive self.improvement/1 cases, with producer independence in the case
summary: validators for the L1 protocol's actions; independence written by the kernel
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
- depends_on: story:finding-intake
- depends_on: story:scorecard-reader
- depends_on: story:judge-guard
revision: 2
---
# Story: self-improvement-protocol

## Outcome

A `self.improvement/1` case is driven by takt's validators, not by hand-written records: each
action of `protocols/self-improvement/1.yaml` that produces evidence has a validator, and the
kernel writes each producer's independence into `case.yaml`, so `scenario.passes_after` is earned
only by a run other than the one that delivered the candidate.

## Context

design:recursive-self-improvement § The loops as protocols. The protocol document exists and
`crates/takt/tests/improvement_protocols.rs` drives it to every outcome with records the test
writes itself, and declares the verifier's independence by editing `case.yaml`. Validators are
Rust structs chosen by action name (`crates/takt/src/validator.rs`) and `case.yaml` carries no
`independence` today (`crates/takt/src/case.rs:481-530`). What no validator can observe (escaped
defects, a review's value) stays a `decision:` requirement or human-sourced evidence.

## Domain relations

- Improvement turn → Case: a turn is a Canon case under `self.improvement/1`; not redeclared in
  `ess/domains/takt.yaml`.
- Case → Finding: one turn verifies one finding. Inferred from the design.

## Scope

- `crates/takt/src/validator.rs` (`finding.file`, `scenario.reproduce`, `scenario.verify`, `scorecard.compare`, `binary.promote` validators) (inferred)
- `crates/takt/src/case.rs` (producer independence in `case.yaml`) (inferred)
- `protocols/self-improvement/1.yaml` (cited)

## Ordering

- depends_on story:finding-intake: `scenario.reproduce` and `scenario.verify` run `scripts/check-finding`.
- depends_on story:judge-guard: `candidate.merge_judge_surface` needs typed grants, and `binary.promote` reads story:promotion's record through it; both edit `crates/takt/src/case.rs` and `crates/takt/src/validator.rs`, so this story follows it on those files.
- depends_on story:validator-confinement (epic:composable-agentic-work, on a branch not yet on main on 2026-10-18; the edge is added once it lands): validators run under the cases-directory guard.

## Acceptance

`takt run --protocol protocols/self-improvement/1.yaml` over a filed finding whose predicate
reproduces at the baseline and not at the candidate reaches `accepted` with every record in the
case written by a validator.

## Checks

- Over a candidate that still reproduces, the case reaches `declined`.
- `crates/takt/tests/improvement_protocols.rs` no longer writes `independence` by hand.
- No evidence record in the case names the harness or an agent's text as its producer.

## Out of Scope

Recording the first turn (story:first-recorded-improvement-turn); the loop protocol's validators
(story:loop-scorecard).
