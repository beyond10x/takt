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
- depends_on: story:loop-scorecard
- depends_on: story:scorecard-save-checks-binary
scope:
- confidence: cited
  path: crates/takt-cli/src/main.rs
- confidence: cited
  path: crates/takt/src/case.rs
- confidence: inferred
  path: crates/takt/src/improvement.rs
- confidence: inferred
  path: crates/takt/src/run.rs
- confidence: cited
  path: crates/takt/src/validator.rs
- confidence: cited
  path: crates/takt/tests/improvement_protocols.rs
revision: 13
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

Derived 2026-10-19 by `story-scoper` against `main` at 3e87bca (code unchanged at a388adf), `headstart/promotion` at
df9681a and `wave/2026-10-18-w12` at 3b0b748; story:loop-scorecard's code is not in 3b0b748 and was read on
`impl/loop-scorecard` at 76508b7. Every line is **cited** (read from the story or the tree) or **inferred** (a reading that could be wrong).

- **Primary surface:** `crates/takt/src/validator.rs`: validators behind the `Validator` trait (`crates/takt/src/validator.rs:45-85@3e87bca`) for `finding.file`, `scenario.reproduce`, `scenario.verify`, `scorecard.compare`, `binary.promote` and the two candidate merges, each also a row of `BUILT_IN`, the table of takt's built-in validators (`crates/takt/src/validator.rs:1611-1671@3b0b748`) — cited (story Context; trait and table read)
- **Files:** `crates/takt/src/case.rs`: `CaseDir::open` writes `format`, `id`, `protocol`, `artifacts` and `inputs` and no `independence` (`crates/takt/src/case.rs:1371-1399@3e87bca`), and opens every artifact but `intent` and `implementation` at `r0` (`crates/takt/src/case.rs:1361-1366@3e87bca`), while a validator observes at its subject's current revision (`crates/takt/src/run.rs:3442-3445@3e87bca`) — cited
- **Files:** `crates/takt/tests/improvement_protocols.rs:78-103@3e87bca`: `open` sets every artifact to `<artifact>-1` (`:91-94`) and writes `independence` by hand (`:98-101`), which the second Check removes; its module doc says no validator exists for these actions (`:7-8`) — cited
- **Files:** `crates/takt-cli/src/main.rs`: every validator is built in `validators_of` and `Forged::register` (`crates/takt-cli/src/main.rs:541-555,712-724@3e87bca`; `:631,804@df9681a`; `:554-569@3b0b748`) and picked only by admissible action (`crates/takt/src/run.rs:131-146,3415-3425@3e87bca`) — cited
- **Also likely:** a merge validator about `candidate`: `PrMerged` performs only `repository.merge` about `implementation` (`crates/takt/src/validator.rs:1316-1331@3e87bca`, `ACTION` at `crates/takt/src/validator.rs:1326@3b0b748`), and after wave 12 no action that may produce `merge_observation` is offered to the harness (`crates/takt/src/run.rs:662-672@3b0b748`), so without one no step takes `candidate.merge` or `candidate.merge_judge_surface` (`protocols/self-improvement/1.yaml:149-176@3e87bca`) — inferred
- **Also likely:** `crates/takt/src/run.rs` or `crates/takt/src/case.rs` gives the case its revisions and its independence: today the loop moves only `implementation` (`crates/takt/src/run.rs:2783,4287@3e87bca`); the baseline could come from the promotion store's latest record (`crates/takt/src/promotion.rs:488-494@df9681a`) and be opened as story:loop-scorecard opens `loop_baseline` (`crates/takt/src/case.rs:1884-1897@76508b7`, `crates/takt-cli/src/main.rs:416-430,513-517@76508b7`); independence could be declared for each validator's `name()`, the producer of its records (`crates/takt/src/validator.rs:46-47@3e87bca`), since a validator is no agent run — inferred
- **Also likely:** turn 1's hand-written records (`.engineering/cases/turn-1-scenario-help/`@3e87bca, "written by hand by a recorder program": `docs/runs/2026-10-18-improvement-turn-1/README.md:5-6@3e87bca`) and their writers: `evidence/0001-e1.json` by `finding.file` (`notes.md:3`); `evidence/0002-e2.json` by `scenario.reproduce` and `evidence/0003-e3.json` by `scenario.verify` (`scripts/check-finding` at the baseline and at the candidate, `notes.md:4-5`); `evidence/0004-e4.json` by `scorecard.compare` (`notes.md:7-11`); `evidence/0005-e5.json` by a candidate-merge validator (`notes.md:14`); `evidence/0006-e6.json` by `binary.promote` (`notes.md:15`); `case.yaml:4-12` (artifact revisions) and `case.yaml:15-17` (`independence`) by the kernel, no validator; `story.yaml:1` by `CaseDir::set_story` (`crates/takt/src/case.rs:1600@3e87bca`); `authority.yaml:1-10` stays a grant and `decisions/` stays Canon's evaluation (`crates/takt/src/case.rs:1786@3e87bca`) — inferred
- **Also likely:** the validators read what exists and edit none of it: `scripts/check-finding` (one result line, `scripts/check-finding:59-66@3e87bca`), `Scorecard::load` and `Scorecard::against` (`crates/takt/src/scorecard.rs:613,640@df9681a`) with the baseline's scorecard copy in the promotion store (`crates/takt/src/promotion.rs:109-111@df9681a`), and `promotion::records` and `promotion::baseline` (`crates/takt/src/promotion.rs:273,490@df9681a`) — inferred
- **Documents:** none edited; `protocols/self-improvement/1.yaml` is read, and no Acceptance or Check line requires changing it — cited
- **Confidence:** medium — every dependency's code now exists (`scripts/check-finding` and `crates/takt/src/scorecard.rs` on main, `crates/takt/src/promotion.rs` at df9681a) and the files above are read, but how revisions and independence enter the case, and where `scorecard.compare` gets the candidate's scorecard, are design choices the tree does not make (the planner's decisions below make them)
- **Would collide with:** `BUILT_IN`, a fixed-length array (`[BuiltIn; 6]` at `crates/takt/src/validator.rs:1628@3b0b748`, `[BuiltIn; 9]` with story:loop-scorecard's rows at `crates/takt/src/validator.rs:1624@76508b7`), and `PrMerged`; `validators_of` in `crates/takt-cli/src/main.rs` (story:loop-scorecard pushes its three validators at `crates/takt-cli/src/main.rs:593-595@76508b7`) and the case-opening block of `run` there (`crates/takt-cli/src/main.rs:416-430,513-517@76508b7`); `CaseDir::open` and the code around `set_revision` in `crates/takt/src/case.rs` (story:loop-scorecard extracts `opening_base` from `open`, `crates/takt/src/case.rs:54-76@76508b7`, and adds `open_loop_baseline` above `set_revision`, `crates/takt/src/case.rs:1884-1900@76508b7`); `Shape` and the validator loop in `crates/takt/src/run.rs` (`crates/takt/src/run.rs:161-191,4002-4140@3b0b748`); the module doc of `crates/takt/tests/improvement_protocols.rs:7-10@76508b7`; story:loop-scorecard's row reader (`crates/takt/src/loop_scorecard.rs:1-16,39@76508b7`); and story:judge-guard's `PrMerged`, `AuthorityDecision` and `prepare_grant`. No case independence is written by wave 12 or by story:loop-scorecard (`git grep -i independen` over `crates/takt/src` and `crates/takt-cli/src` is empty at 3b0b748 and 76508b7) — cited
- **Safety fact:** the L1 actions and the kinds `finding_record`, `scenario_result`, `scorecard_result` and `promotion_record` appear in no protocol file but `protocols/self-improvement/1.yaml` (`git grep` over `protocols/` at 3e87bca and 3b0b748), and a validator is due only when its action is admissible (`crates/takt/src/run.rs:131-146@3e87bca`), so the new validators change nothing for `story.delivery/1` or `loop.improvement/1` cases; `software.change/1` comes from an external crate and was not read — step 2, unproven

Not established: (1) the Context's `case.rs:481-530` is stale, `CaseDir::open` is at `crates/takt/src/case.rs:1330-1468@3e87bca`; (2) no agent-run identity at any of the three revisions (`crates/takt/src/events.rs:67-77@3e87bca`); (3) answered: a judge-surface grant is recordable on an improvement case, checked at the revision of the capability's `grant_subject` (`crates/takt/src/case.rs:1685-1704@3e87bca`), and turn 1 holds one (`.engineering/cases/turn-1-scenario-help/authority.yaml:1-5@3e87bca`); (4) answered: the dependencies exist; (5) partly answered: `scripts/check-finding` writes only under its cache (`scripts/check-finding:54-57@3e87bca`) but reads the finding from the working tree (`:143-146`) and is itself a tree file; under wave 12 a child of up to 900 s also wants `observe_within` (`crates/takt/src/validator.rs:71-84@3b0b748`); (6) answered: validators are registered unconditionally and chosen by action (`crates/takt/src/protocol.rs:62-69@3e87bca`).

Typed entries: `crates/takt/src/validator.rs`, `crates/takt/src/case.rs`, `crates/takt/tests/improvement_protocols.rs`, `crates/takt-cli/src/main.rs` (cited); `crates/takt/src/run.rs`, `crates/takt/src/improvement.rs` (inferred).

## Ordering

- depends_on story:finding-intake and story:validator-confinement, both on main: `scenario.reproduce` and `scenario.verify` run `scripts/check-finding`, and every validator runs under the cases-directory guard (`crates/takt/src/run.rs:3549-3600@3e87bca`).
- depends_on story:judge-guard: `candidate.merge_judge_surface` needs typed grants, and `binary.promote` reads story:promotion's record through it; both edit `crates/takt/src/case.rs` and `crates/takt/src/validator.rs`, so this story follows it on those files.
- depends_on story:loop-scorecard (files only): both register validators in `validators_of` (`crates/takt-cli/src/main.rs:593-595@76508b7`), grow the fixed-length `BUILT_IN` array (`crates/takt/src/validator.rs:1624@76508b7`) and open a case with a baseline revision (`crates/takt/src/case.rs:54-76,1884-1900@76508b7`).
- depends_on story:scorecard-save-checks-binary: `scorecard.compare` reads a saved scorecard (D2 below), which must name the binary that produced its runs.

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

## Planner decisions

The planner's decisions on what the 2026-10-19 scope left open:

- D1, revisions and independence: at open, the kernel sets `baseline` to the revision of story:promotion's latest record and `candidate` to the case input's revision, and writes `independence` into `case.yaml`, one entry per validator name, since a validator is no agent run. A case opened with no promotion record stops at open naming it.
- D2, the candidate's scorecard: `scorecard.compare` reads a `takt scorecard --save` file named in the case's inputs, refuses one whose runs' binary is not the candidate revision (story:scorecard-save-checks-binary), and compares it with the baseline's scorecard copy in the promotion store under the promotion rule.
- D3, `binary.promote`: the validator runs `takt promote` in a clean managed checkout at the candidate revision, so `HEAD` equals the candidate (`crates/takt-cli/src/main.rs:69-73@df9681a`); it writes no promotion record itself.
- D4, `finding.file`: it checks a finding filed before the case, named in the case's inputs; it writes nothing to the planning store.
- D5, `scripts/check-finding` as a judge: the validators run the copy at the case's pinned `base_commit` (`git show <base_commit>:scripts/check-finding`), never the tree's, under `observe_within`.
- D6, the merge: one merge validator taking the action and the subject from the protocol serves `candidate.merge`, `candidate.merge_judge_surface` and `loop.merge`; `PrMerged`'s fixed `repository.merge` about `implementation` becomes one configuration of it.
- D7, placement: the new validators live in `crates/takt/src/improvement.rs`, as story:loop-scorecard's live in `loop_scorecard.rs`, so `validator.rs` changes only at `BUILT_IN` and the merge validator.
