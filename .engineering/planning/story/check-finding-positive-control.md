---
format: aep.planning-md/3
id: story:check-finding-positive-control
kind: story
status: draft
title: check-finding says not_reproduced only when the command reached the scenario
tags:
- priority:high
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
- informed_by: review-result:main-fe93286-areas
revision: 2
---
## Outcome

`scripts/check-finding` returns `not_reproduced` only when the predicate's command ran to the scenario and showed the expected behaviour's absence; a command that crashes, fails to start, or exits with a status the predicate does not name returns `error` naming the status, and the planner's verify step reads `error` as no verification.

## Context

From the review of main at fe93286 on 2026-10-19: ten independent reviewers, one per area, each finding then fact-checked by a second reviewer against the code (review-result:main-fe93286-areas), finding S9.1, confirmed by running the block: `scripts/check-finding:293-299@fe93286` returns `not_reproduced` for any exit other than the predicate's or any missing text, so a candidate that panics (exit 101) or rejects its flags (exit 2) reads as a fixed defect; the probe printed `status=101 result=not_reproduced`. An improvement turn's `scenario.passes_after` rests on this result, so a broken candidate can earn it.

## Scope

- `scripts/check-finding`: the result rule (cited)
- `scripts/file-finding`: predicates name the healthy exit of the command (inferred)
- `crates/takt-cli/tests/finding_scripts.rs`: rows (inferred)

## Acceptance

With a predicate `exit: 3` and its output string, a candidate command that exits 101 printing a panic gives `error` naming 101; one that exits 0 without the string gives `not_reproduced`; one that exits 3 with it gives `reproduced`.
