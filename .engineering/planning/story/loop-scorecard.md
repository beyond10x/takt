---
format: aep.planning-md/3
id: story:loop-scorecard
kind: story
status: active
title: Validators drive loop.improvement/1 cases; the L2 turn runs after three L1 turns
summary: loop scorecard, definition check, comparison; the weekly L2 turn
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
- depends_on: story:first-recorded-improvement-turn
- depends_on: story:agent-profiles
scope:
- confidence: cited
  path: .agents/planner-agent.md
- confidence: inferred
  path: crates/takt-cli/src/main.rs
- confidence: inferred
  path: crates/takt/src/scorecard.rs
- confidence: cited
  path: crates/takt/src/validator.rs
- confidence: inferred
  path: crates/takt/tests/improvement_protocols.rs
revision: 12
transitions:
- {from: "draft", to: "proposed", at: "2026-10-18T20:13:47Z", actor: "human:timo", revision: 11, decided_on: {"recorded":{"review_outcome":3}}}
- {from: "proposed", to: "active", at: "2026-10-18T20:13:47Z", actor: "human:timo", revision: 12, decided_on: {"recorded":{"review_outcome":3}}}
---
# Story: loop-scorecard

## Outcome

A `loop.improvement/1` case is driven by validators: `loop.measure` prints the loop scorecard over
the recorded `self.improvement/1` cases, one row per improvement turn, and the figures computed
from those rows (turns, accepted share, promoted share, median tokens per accepted turn, and the
time from finding to verified); `loop.definition.check` compares metric
definitions; `loop.compare` compares the metric after the candidate; and the planner profile
carries the L2 turn: once three L1 turns are recorded, then weekly.

## Context

design:recursive-self-improvement § Improving the loop, § The loops as protocols.
`protocols/loop-improvement/1.yaml` exists and `crates/takt/tests/improvement_protocols.rs`
drives it with hand-written records. A metric's definition is a judge surface: a candidate that
changes one without printing the previous figures beside it is `redefined` and cannot be merged.
Turn 1 is story:first-recorded-improvement-turn; turns 2 and 3 are ordinary improvement turns the
planner agent runs on findings filed after it, and this story stays in draft until they exist.

## Domain relations

- Loop scorecard → Improvement turn: one row per improvement turn, holding the turn id, the finding, the baseline and candidate revisions, the verdict, whether it was promoted, and the tokens and wall time from the turn's records. Decided 2026-10-18.
- A turn whose case never reached an outcome gets a row too: its `verdict` is absent, which means open. Decided 2026-10-18.
- Each row carries `filed_at` and `verified_at` (`verified_at` absent until the finding is verified). Decided 2026-10-18.
- Loop scorecard figures: turns, accepted share (accepted rows / all rows), promoted share, median tokens per accepted turn, and the time from finding to verified (from `filed_at` and `verified_at`), computed from the rows and never stored (`ess/domains/takt.yaml`, takt.work.LoopScorecard). Decided 2026-10-18.

## Scope

Derived 2026-10-18 by `story-scoper`. Every line is **cited** (read from the story or the tree) or
**inferred** (a reading that could be wrong).

- **Primary surface:** `crates/takt` validators — cited, design § The loops as protocols: "the validators for these actions (Rust structs chosen by action name, `crates/takt/src/validator.rs`)"
- **Files:** `crates/takt/src/validator.rs:46-56` (the `Validator` trait the three new structs implement, beside `TestsRun`/`GateRun`/`PrOpen`/`CiRead`/`PrMerged`) — cited
- **Files:** `crates/takt/src/scorecard.rs` (the loop rows; the file does not exist on `fix/2026-10-18-review` and story:scorecard-reader creates it) — inferred
- **Symbols:** actions `loop.measure`, `loop.definition.check`, `loop.compare`; evidence kinds `loop_measurement`, `definition_check`, `loop_comparison`; results `below_target`/`at_target`, `unchanged`/`dual_reported`/`redefined`, `improved`/`not_improved` — cited, `protocols/loop-improvement/1.yaml:29-50,109-153`
- **Also likely:** `crates/takt-cli/src/main.rs:342-393` (every validator is registered here, unconditionally, into `validators`) — inferred
- **Also likely:** `crates/takt/tests/improvement_protocols.rs:393-500` (the loop cases it drives today with hand-written `loop_measurement`/`definition_check`/`loop_comparison` records) — inferred, the story's Context names this file but its Acceptance does not say whether the test changes or a new test is added
- **Documents:** `.agents/planner-agent.md` (the L2 turn: after three recorded L1 turns, then weekly; it names the lowest metric as the finding to file; the file does not exist yet and story:agent-profiles creates it) — cited, story Outcome and Ordering
- **Confidence:** medium — the design names `validator.rs` and the protocol names every action and result, but `scorecard.rs` and the profile do not exist yet, and nothing places the stored metric definitions that `loop.definition.check` compares
- **Would collide with:** any unit adding or changing validators in `crates/takt/src/validator.rs` (story:self-improvement-protocol's L1 validators; any judge-guard or confinement change at `validator.rs:193-216`), any unit on `crates/takt/src/scorecard.rs`, and any unit editing `.agents/planner-agent.md` (story:finding-intake's intake procedure)
- **Safety fact:** a new validator only runs when its action is admissible in the case's protocol (`crates/takt/src/run.rs:89` `is_due`, picked by action at `run.rs:473-477`), and `loop.*` actions exist only in `protocols/loop-improvement/1.yaml`, so registering the three validators changes nothing for `story.delivery/1` or `self.improvement/1` cases — step 2, unproven

Not established: where metric definitions live; the full depends_on chain to this story (taken from a review, not walked); the L2 turn's place in the not-yet-existing profile; whether `loop.measure` may read sibling case directories; the gate of three recorded L1 turns has no producer, so the story stays draft and this scope may be stale when picked up.

Typed entries: `crates/takt/src/validator.rs` (cited), `crates/takt/src/scorecard.rs` (inferred), `crates/takt-cli/src/main.rs` (inferred), `crates/takt/tests/improvement_protocols.rs` (inferred), `.agents/planner-agent.md` (cited).

## Ordering

- depends_on story:first-recorded-improvement-turn: the first row.
- depends_on story:agent-profiles: the L2 turn goes into the profile it creates.
- Gate: three recorded L1 turns before this story moves past draft.

## Acceptance

Over three recorded turns, `loop.measure` writes a `loop_measurement` record whose figures each name
the case ids they were computed from.

## Checks

- A changed metric definition makes `loop.definition.check` write `dual_reported` when both figures print, and `redefined` when only the new one does.
- With one turn open, its row has no verdict and the accepted share is accepted rows / all rows.
- The time from finding to verified is computed from each row's `filed_at` and `verified_at`.
- The operator profile's L2 turn names the lowest metric as the finding to file.

## Out of Scope

Automating the L2 turn and phase R3 (takt proposing stories from its scorecards).

Escaped defects as human-sourced counts: a later, human-sourced figure, since no validator observes it (decided 2026-10-18).
