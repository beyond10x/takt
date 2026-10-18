---
format: aep.planning-md/3
id: story:scorecard-reader
kind: story
status: draft
title: takt scorecard reads run events per scenario against the promoted baseline
summary: outcome, steps, tokens, refusals, errors, wall time from events.jsonl only
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
- depends_on: story:run-record-and-story-evidence
- depends_on: story:scenario-corpus
revision: 2
---
# Story: scorecard-reader

## Outcome

`takt scorecard` prints, per scenario run, outcome, steps, tokens, refusals, `NoProgress` and
errors, and wall time, read only from the runs' events under `.engineering/cases/`; `--save`
writes the scorecard as a file named by its revision; `--against <saved>` compares each scenario
with the same scenario in a saved scorecard, marks a row worse when its outcome is worse, and
prints the token difference.

## Context

design:recursive-self-improvement § Scorecard. Tokens come from the runs' events, never from a
`task_spend` file a candidate could plant. The events exist after story:run-record-and-story-evidence
(`events.jsonl` per case). Which saved scorecard is the promoted baseline, and the median and
daily rules, are story:promotion's. A metric's definition is a judge surface: for one cycle after
a definition changes, the scorecard prints the figure under the previous definition beside the new
one. story:week-of-daily-runs reads the same records for its weekly report and builds on this
reader.

## Domain relations

- Scorecard → Scenario run: one row per run. UNMAPPED: whether a scorecard covers one candidate
  revision or a period.

## Scope

- new: `crates/takt/src/scorecard.rs` (inferred)
- `crates/takt/src/lib.rs` (inferred)
- `crates/takt-cli/src/main.rs` (`scorecard`) (inferred)

## Ordering

- depends_on story:run-record-and-story-evidence: the events are its output.
- depends_on story:scenario-corpus: the rows are scenario runs; both edit `crates/takt-cli/src/main.rs`, so this story follows it on that file.

## Acceptance

Over the case directories of two scenario 1 runs, `takt scorecard` prints each run's tokens equal
to the sum of the harness-reported tokens in that run's events.

## Checks

- A planted `task_spend` evidence file in the candidate tree changes no number the scorecard prints.
- `--against` a saved scorecard in which scenario 1 was `accepted` marks a `NoProgress` row worse.
- A changed metric definition prints both figures for one cycle.

## Out of Scope

Choosing the baseline and promoting (story:promotion); the weekly report and defects escaped
(story:week-of-daily-runs); the loop scorecard (story:loop-scorecard).
