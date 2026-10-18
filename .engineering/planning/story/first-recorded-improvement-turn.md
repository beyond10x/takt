---
format: aep.planning-md/3
id: story:first-recorded-improvement-turn
kind: story
status: draft
title: The first improvement turn recorded as a self.improvement/1 case
summary: the default-protocol defect, scenario 1 before and after
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
- depends_on: story:scenario-corpus
- depends_on: story:self-improvement-protocol
- depends_on: story:cli-protocol-defaults
revision: 2
---
# Story: first-recorded-improvement-turn

## Outcome

The first improvement turn is recorded end to end as a `self.improvement/1` case and ends in a
promotion: the finding "a story taken from the store runs under `software.change/1`", its
`expected_before` predicate run on scenario 1 at the baseline (reproduced) and at the candidate
(not reproduced), the fix merged, the scorecard compared, and the candidate's binary promoted.

## Context

design:recursive-self-improvement § One improvement turn; epic:recursive-self-improvement's
acceptance asks for one promotion decided by the scorecard, and this turn is where it happens. The
defect was found by the first live trial on 2026-10-18 (`takt run` with no `--protocol` took the
story under the default `software.change/1`, `crates/takt-cli/src/main.rs:66`); its fix is filed as
story:cli-protocol-defaults (epic:composable-agentic-work, on a branch not yet on main on
2026-10-18). The other trial defect, the pull request opened before any commit, was fixed by hand
before a case could record it, so it has no baseline record and is not this turn.

## Scope

- the scratch repository's scenario 1 (cited by the design)
- `.engineering/cases/<id>/` of the turn's case (inferred)

## Ordering

- depends_on story:scenario-corpus: the before and after runs need a fresh base each.
- depends_on story:self-improvement-protocol: the case runs under it.
- story:cli-protocol-defaults is the candidate; its edge is added once its branch is on main.

## Acceptance

A case directory whose `case.yaml` names protocol `self.improvement` holds a last decision in which
outcome `accepted` is legitimate and claim `promoted` is true.

## Checks

- The baseline run's case shows the story taken under `software.change/1`; the candidate run's under `story.delivery/1`.
- The finding's story carries an evidence record whose `--ref` names that case directory.

## Out of Scope

Fixing the defect itself (story:cli-protocol-defaults).
