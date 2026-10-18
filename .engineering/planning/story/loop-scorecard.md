---
format: aep.planning-md/3
id: story:loop-scorecard
kind: story
status: draft
title: Validators drive loop.improvement/1 cases; the L2 turn runs after three L1 turns
summary: loop scorecard, definition check, comparison; the weekly L2 turn
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
- depends_on: story:first-recorded-improvement-turn
- depends_on: story:agent-profiles
revision: 2
---
# Story: loop-scorecard

## Outcome

A `loop.improvement/1` case is driven by validators: `loop.measure` prints the loop scorecard over
the recorded `self.improvement/1` cases (the share of findings that became fixes, the share of
fixes verified against declined, the time from finding to verified, tokens per verified fix, and
escaped defects as human-sourced counts, labelled so); `loop.definition.check` compares metric
definitions; `loop.compare` compares the metric after the candidate; and the operator profile
carries the L2 turn: once three L1 turns are recorded, then weekly.

## Context

design:recursive-self-improvement § Improving the loop, § The loops as protocols.
`protocols/loop-improvement/1.yaml` exists and `crates/takt/tests/improvement_protocols.rs`
drives it with hand-written records. A metric's definition is a judge surface: a candidate that
changes one without printing the previous figures beside it is `redefined` and cannot be merged.
Turn 1 is story:first-recorded-improvement-turn; turns 2 and 3 are ordinary improvement turns the
operator agent runs on findings filed after it, and this story stays in draft until they exist.

## Domain relations

- Loop scorecard → Improvement turn: one row per recorded turn. Inferred from the design.

## Scope

- `crates/takt/src/scorecard.rs` (the loop rows) (inferred)
- `crates/takt/src/validator.rs` (`loop.measure`, `loop.definition.check`, `loop.compare`) (inferred)
- `.agents/operator-agent.md` (the L2 turn, in the file story:agent-profiles creates) (inferred)

## Ordering

- depends_on story:first-recorded-improvement-turn: the first row.
- depends_on story:agent-profiles: the L2 turn goes into the profile it creates.
- Gate: three recorded L1 turns before this story moves past draft.

## Acceptance

Over three recorded turns, `loop.measure` writes a `loop_measurement` record whose figures each name
the case ids they were computed from.

## Checks

- A changed metric definition makes `loop.definition.check` write `dual_reported` when both figures print, and `redefined` when only the new one does.
- Escaped defects are marked human-sourced in the output.
- The operator profile's L2 turn names the lowest metric as the finding to file.

## Out of Scope

Automating the L2 turn and phase R3 (takt proposing stories from its scorecards).
