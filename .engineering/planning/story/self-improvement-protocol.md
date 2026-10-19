---
format: aep.planning-md/3
id: story:self-improvement-protocol
kind: story
status: active
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
- informed_by: story:confinement-seam
- depends_on: story:check-finding-run-step-confined
scope:
- confidence: cited
  path: crates/takt-cli/src/main.rs
- confidence: cited
  path: crates/takt/src/case.rs
- confidence: cited
  path: crates/takt/src/improvement.rs
- confidence: inferred
  path: crates/takt/src/lib.rs
- confidence: cited
  path: crates/takt/src/validator.rs
- confidence: cited
  path: crates/takt/tests/improvement_protocols.rs
- confidence: cited
  path: crates/takt/tests/pr_opens_when_done.rs
- confidence: inferred
  path: crates/takt/tests/self_improvement.rs
revision: 22
transitions:
- {from: "draft", to: "proposed", at: "2026-10-19T06:32:42Z", actor: "human:timo", revision: 21, decided_on: {"recorded":{"review_outcome":3}}}
- {from: "proposed", to: "active", at: "2026-10-19T06:32:43Z", actor: "human:timo", revision: 22, decided_on: {"recorded":{"review_outcome":3}}}
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

Derived 2026-10-19 by `story-scoper`, read-only at `wave/2026-10-19-w13` 4e1aa08 (main plus wave 13). `crates/`, `scripts/` and `protocols/` are identical at bbe5bdb and 4e1aa08, so story:judge-guard's citations hold here. Line numbers are `path:line@4e1aa08`. Every line is **cited** (read from the story or the tree) or **inferred** (a reading that could be wrong).

- **Primary surface:** `crates/takt/src/improvement.rs`, new (D7): one validator type per action — `finding.file`, `scenario.reproduce`, `scenario.verify`, `scorecard.compare`, `candidate.merge`, `candidate.merge_judge_surface`, `binary.promote` — placed as `LoopMeasure`, `LoopDefinitionCheck`, `LoopCompare` are in `crates/takt/src/loop_scorecard.rs:1226,1420,1627` — cited
- **Files:** `crates/takt/src/validator.rs`: `BUILT_IN`, `[BuiltIn; 9]` (1829-1880), one row per new type; the row is what keeps the harness off the new kinds (`crates/takt/src/run.rs:472-490,667-677`); `ended_by` (415-429), the deadline rule a `check-finding` child needs, is private — cited
- **Files:** `crates/takt/src/case.rs`: a turn-opening method beside `open_loop_baseline` (1916-1929) that sets `baseline`, `candidate`, `finding` and `promotion` (open leaves them at `r0`, 1589-1599), calls `set_story` (1810) and writes `independence` into `case.yaml` (`git grep -i independen` over `crates/*/src` is empty); a protocol constant beside `LOOP_PROTOCOL`/`LOOP_BASELINE` (43-48) — cited
- **Files:** `crates/takt-cli/src/main.rs`: the `run` arm takes the turn's opening values out of `--input` before the undeclared-input refusal, as it does `loop_baseline` (677-691; refusal 692-710), and opens them after the case (774-778); `validators_of` pushes the new validators (838-858) — cited
- **Files:** `crates/takt/tests/improvement_protocols.rs` (its module doc says no `self.improvement/1` validator exists, 7-10; `open` writes `independence` by hand, 80-105) and `crates/takt/tests/pr_opens_when_done.rs:513-573` (every `impl Validator for` under `src/` needs a `BUILT_IN` row and one instance, 572) — cited
- **Also likely:** `crates/takt/src/lib.rs:17-24` (`pub mod improvement;`) and an end-to-end test `crates/takt/tests/self_improvement.rs` (`takt run` to `accepted` and to `declined`) — inferred
- **Documents:** none edited; `protocols/self-improvement/1.yaml` is read, its actions (124-196) need no change on the minimal path — cited
- **Reuse:** `scripts/file-finding --predicate` (243-251), `scripts/check-finding <finding> <revision>` (5-6, 76-89), `promotion::promote` dry run (`WouldWrite`/`WouldRefuse`, `promotion.rs:274-289,2030`) and real run (`Promoted`/`Regression`/`BuildFailed`), `promotion::baseline` (818-824), `CaseDir::grants(action)` (`case.rs:1890-1914`), the `loop_baseline` opening pattern (`case.rs:80-168,1916-1929`; `main.rs:677-691,774-778`) — cited
- **Confidence:** medium
- **Would collide with:** `BUILT_IN` and `pr_opens_when_done.rs:540-572` (every unit adding a validator); in `main.rs` the `run` opening block (677-778) and `validators_of` (838-858); in `case.rs` the code around `open_loop_baseline`/`set_revision` (1916-1940). story:judge-guard edits other hunks of `case.rs`, `main.rs` and `validator.rs` (`case.rs:488-1022`, `main.rs:274-286,468-490,1015-1027,1509-1520`, `PrMerged`), so the two are disjoint by hunk and collide by file; this story follows judge-guard on lane A
- **Safety fact:** the new actions and the kinds `finding_record`, `scenario_result`, `scorecard_result` and `promotion_record` appear in no protocol and no `crates/*/src` file but `protocols/self-improvement/1.yaml`, and a validator is due only when its exact action is admissible (`run.rs:149-157`), so the new validators change nothing for `story.delivery/1` or `loop.improvement/1` cases — level 2, unproven

## Planner's decisions at scoping (2026-10-19, for release 0.2.0)

- S1 (the merge): the two merge validators observe, they do not merge. `candidate.merge` and `candidate.merge_judge_surface` record `merged` when the case holds the grant its protocol requires for the candidate revision (`CaseDir::grants`) and the candidate is an ancestor of the mainline's tip as the forge reports it (judge-guard's D7 method); the merge itself happens in the candidate's own delivery case under judge-guard's guard. One type per action, sharing one function. Turns 1-3 recorded e5 the same way, by hand.
- S2 (`candidate.deliver`): delivery happens outside the case (D1); the candidate revision is given at open. No `run.rs` change.
- S3 (the finding): the turn opens with `--intent` and the inputs `finding=<story id>` and `candidate=<full sha>`, taken out of `--input` as `loop_baseline` is; `story.yaml` names the finding (`set_story`); the `finding` artifact is the case's `base_commit`; `baseline` is the latest promoted revision (`promotion::baseline`).
- S4 (the candidate's scorecard): read from a fixed location, `$XDG_STATE_HOME/takt/scorecards/scorecard-<candidate>.json` (default `~/.local/state/takt/scorecards/`), written beforehand with `takt scorecard --save`; a missing file leaves `scorecard.compare` waiting, naming the path.
- S5 (`surface`): computed at open from `.engineering/judge-allow.yaml` at the base (judge-guard) over `base..candidate`; an operator-given `surface` that disagrees is refused naming the first judge-surface path.
- S6 (the judge): `scenario.reproduce`, `scenario.verify` and `scorecard.compare` run with the installed promoted binary as the judge, never the candidate's; the candidate binary only runs inside `check-finding`'s confined run step (story:check-finding-run-step-confined).

## Ordering

- depends_on story:finding-intake and story:validator-confinement, both on main: `scenario.reproduce` and `scenario.verify` run `scripts/check-finding`, and every validator runs under the cases-directory guard (`crates/takt/src/run.rs:3549-3600@1ded28f`).
- depends_on story:judge-guard: `candidate.merge_judge_surface` needs typed grants, and `binary.promote` reads story:promotion's record through it; both edit `crates/takt/src/case.rs` and `crates/takt/src/validator.rs`, so this story follows it on those files.
- depends_on story:loop-scorecard (files only): both register validators in `validators_of` (`crates/takt-cli/src/main.rs:593-595@2052a61`), grow the fixed-length `BUILT_IN` array (`crates/takt/src/validator.rs:1624@2052a61`) and open a case with a baseline revision (`crates/takt/src/case.rs:54-76,1884-1900@2052a61`).
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

The planner's decisions on what the 2026-10-19 scope left open (revised after the reviewer's verdict on them, 2026-10-19):

- D1, revisions and independence: at open, the kernel sets `baseline` to the revision of story:promotion's latest record and `candidate` to the case input's revision, and writes `independence` into `case.yaml`, one entry per validator name, since a validator is no agent run. A case opened with no promotion record stops at open naming it.
- D2, the candidate's scorecard: `scorecard.compare` reads a `takt scorecard --save` file named in the case's inputs, refuses one whose runs' binary is not the candidate revision (story:scorecard-save-checks-binary), and compares it with the baseline's scorecard copy in the promotion store under the promotion rule.
- D3, `binary.promote`: the validator runs `takt promote` in a clean managed checkout at the candidate revision, so `HEAD` equals the candidate (`crates/takt-cli/src/main.rs:69-73@2d10b7f`); it writes no promotion record itself.
- D4, `finding.file`: it checks a finding filed before the case, named in the case's inputs; it writes nothing to the planning store.
- D5, the judge's scripts and finding: before reading anything at `base_commit`, the validators apply story:judge-guard D4 (`base_commit` must be an ancestor of the target's tip) and stop the case if it fails; then they run `scripts/check-finding` and every script it calls (`scripts/file-finding`, `check-finding:85@1ded28f`) from a checkout of the case's base at `base_commit`, and read the finding from the planning store there, never from the candidate's tree (`check-finding:143-146@1ded28f` reads `--repo`'s working tree today), under `observe_within`.
- D6, the merge: one merge validator taking the action and the subject from the protocol serves `candidate.merge`, `candidate.merge_judge_surface` and `loop.merge`; `PrMerged`'s fixed `repository.merge` about `implementation` becomes one configuration of it, and story:judge-guard's guard sits in it.
- D7, placement: the new validators live in `crates/takt/src/improvement.rs`, as story:loop-scorecard's live in `loop_scorecard.rs`, so `validator.rs` changes only at `BUILT_IN` and the merge validator.
- D8, candidate code cannot write the judge's records: every validator that runs candidate code (the whole `scripts/check-finding` call in `scenario.verify`, its `cargo build` and the candidate's `build.rs` included, and the candidate's scenario runs) runs it under the bubblewrap profile story:promotion builds with (the real home, the promotion store and the install root absent), so the routes M19 and C6 (story:confinement-seam) are closed for these validators without waiting for the general seam. D1 and D2 read the promotion store only from the validator's own process.

Acceptance, added: with a candidate whose `scenario.verify` run tries to rewrite the newest promotion record's scorecard copy (the reviewer's C6 on story:promotion), the record is unchanged and `scorecard.compare` judges against the original; with a candidate tree whose `scripts/file-finding --predicate` emits another predicate, `scenario.verify` still runs the base's copy and reports `reproduced` for an unfixed defect; a candidate whose `build.rs` writes to the promotion store during that build leaves the store unchanged.
