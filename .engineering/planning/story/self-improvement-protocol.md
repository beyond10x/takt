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
- depends_on: story:validator-confinement
scope:
- confidence: inferred
  path: crates/takt-cli/src/main.rs
- confidence: cited
  path: crates/takt/src/case.rs
- confidence: inferred
  path: crates/takt/src/run.rs
- confidence: cited
  path: crates/takt/src/validator.rs
- confidence: cited
  path: crates/takt/tests/improvement_protocols.rs
- confidence: cited
  path: protocols/self-improvement/1.yaml
revision: 7
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
- Case → Finding: an improvement case references exactly one Finding; a Finding may have many cases (a retried or re-opened turn). Many-to-one, the case side references (`ess/domains/takt.yaml`, takt.work.Case `finding`). Decided 2026-10-18.
- promotion_record result: takt.work.PromotionResult.

## Scope

Derived 2026-10-18 by `story-scoper` against `fix/2026-10-18-review` at 768db1c, with
`impl/validator-confinement` at c0e8f4e for validator code. Every line is **cited** (read from the
story or the tree) or **inferred** (a reading that could be wrong).

- **Primary surface:** `crates/takt/src/validator.rs`, new structs behind the `Validator` trait (`validator.rs:45-56`) for `finding.file`, `scenario.reproduce`, `scenario.verify`, `scorecard.compare` and `binary.promote` — cited (story Context; trait at `validator.rs:45-56`)
- **Files:** `crates/takt/src/case.rs` gets producer independence in `case.yaml`. `CaseDir::open` is at `case.rs:780-808` on the integration branch (`main:481`, the story's `481-530`). Today it writes no `independence` key — cited
- **Files:** `crates/takt/tests/improvement_protocols.rs:76-103`, the `open` helper that writes `independence` by hand (`:98-101`), which a check says must go — cited
- **Files:** `protocols/self-improvement/1.yaml` — cited (story Scope). No acceptance line requires an edit to it
- **Also likely:** `crates/takt-cli/src/main.rs:342-394` registers new validators. Every validator is built there, and nothing else picks one by action name — inferred
- **Also likely:** `crates/takt/src/run.rs` names the producer's agent run so independence can be derived. Validators' records name `Validator::name()` as producer (`validator.rs:46`), and nothing in the tree carries a run identity — inferred
- **Also likely:** `crates/takt/src/case.rs:794-799` sets revisions for `baseline` and `candidate`. Today every artifact other than `intent` and `implementation` opens at `r0`, and validators observe at `case.revision(subject)` (`run.rs:89-96`) — inferred
- **Also likely:** a candidate-merge validator, or a change to `PrMerged`. `PrMerged` is fixed to action `repository.merge` and subject `implementation` (`validator.rs:714-724`). The protocol's `candidate.merge` and `candidate.merge_judge_surface` need `merge_observation` about `candidate` — inferred
- **Documents:** none beyond the protocol file above — cited
- **Confidence:** medium — the story names its files, but `scorecard.rs`, `promotion.rs` and `scripts/check-finding` do not exist yet (dependencies' Scopes, `ls scripts` fails), so how far the code spreads is read from the design
- **Would collide with:** any unit touching `crates/takt/src/validator.rs`, `crates/takt/src/case.rs` (`open`, grants keyed on `implementation` at `case.rs:434-450`), `crates/takt/src/run.rs` (validator loop and cases-directory guard) or the validator assembly in `crates/takt-cli/src/main.rs:342-394` — cited
- **Safety fact:** no action of `self.improvement/1` shares a name with `story.delivery/1` (`protocols/story-delivery/1.yaml`) or `software.change/1`. A validator is due only when its action is admissible (`run.rs:89`), so the new validators never fire in an L0 case — step 3, unproven

Not established: the story's own `case.rs:481-530` is stale (now 780-808); no agent-run identity exists in the tree (`git grep run_id` empty); grant revisions are read from `artifacts.implementation`, which `self.improvement/1` does not declare, so a judge-surface grant may not be recordable on such a case (overlaps judge-guard); unbuilt dependencies (`scripts/check-finding`, `scorecard.rs`, `promotion.rs`); whether `scripts/check-finding` runs through validator-confinement's `writes()`/`run_child`; whether `main.rs` picks the right validators for a protocol given as a file path.

Typed entries: `crates/takt/src/validator.rs`, `crates/takt/src/case.rs`, `crates/takt/tests/improvement_protocols.rs`, `protocols/self-improvement/1.yaml` (cited); `crates/takt-cli/src/main.rs`, `crates/takt/src/run.rs` (inferred).

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
