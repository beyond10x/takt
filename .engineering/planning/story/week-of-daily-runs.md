---
format: aep.planning-md/3
id: story:week-of-daily-runs
kind: story
status: draft
title: Seven daily bounded runs on three repositories, reported in numbers
summary: 'takt report: units accepted, tokens, interventions, defects escaped, traceable to run records'
relations:
- decomposes: epic:composable-agentic-work
- depends_on: story:run-record-and-story-evidence
- depends_on: story:declared-loops
- depends_on: story:merge-under-authority
- serves: vision:safe-agentic-coding
- depends_on: story:confinement-seam
- depends_on: story:scorecard-reader
- depends_on: story:report-sums-cases
scope:
- confidence: cited
  path: crates/takt-cli/src/main.rs
- confidence: inferred
  path: crates/takt/src/lib.rs
- confidence: inferred
  path: crates/takt/src/report.rs
- confidence: inferred
  path: crates/takt/tests/report.rs
- confidence: inferred
  path: docs/reports
revision: 7
---
# Story: week-of-daily-runs

## Outcome

A reader has, for three repositories and seven days of one bounded run each, the number of units accepted, tokens spent, human interventions and defects escaped, every number traceable to run records, and so can judge whether bounded validated runs do work worth their cost.

## Context

`docs/design/takt.md` § 4 M5: "three repositories, each a daily `run --hours 2` for a week; units accepted, tokens, human interventions, defects escaped | the numbers". The run records exist after story:run-record-and-story-evidence (`events.jsonl` and the decision documents in each case directory); `--hours` exists (`crates/takt/src/run.rs:124-128`); several units per run exist after story:declared-loops; a unit reaches `accepted` only after story:merge-under-authority.

This story adds `takt report --cases <dir>...` reading those records: units accepted = cases terminated `accepted`; tokens = the sum of harness-reported tokens (`run.rs:212-214`); human interventions = `takt approve` grants, one per `approval` evidence record on the story of a case in the period (story:merge-under-authority, from the cleared decision-blocker:approval-grant-and-case-resume), plus operator actions the events record; defects escaped = a number no validator can observe, so the operator enters it per repository and day with its source (the issues opened against units merged in the period), and the report prints it beside the computed three. The schedule, one `run --hours 2` per repository per day, is a cron entry or a CI schedule outside this repository; which three repositories is the operator's choice when scheduling, and the week's report is committed under `docs/reports/` with no private name in it (`AGENTS.md:35-36`).

## Domain relations

- Case → Decision: one-to-many, owned by the case. Inferred from `crates/takt/src/case.rs:175-198`; the report reads the last decision of each case for its termination.
- Run → Case: one-to-many after story:declared-loops, inferred from `crates/takt/src/run.rs:105` as it is today.

## Scope

- new: `crates/takt/src/report.rs`
- `crates/takt/src/lib.rs` (the `report` module registered)
- `crates/takt-cli/src/main.rs` (`report`)
- new: `crates/takt/tests/report.rs` with recorded case directories as fixtures
- new: `docs/reports/` (the week)

## Ordering

- depends_on story:run-record-and-story-evidence: both edit `crates/takt/src/lib.rs`; this story reads the `events.jsonl` that story writes.
- depends_on story:declared-loops: both edit `crates/takt-cli/src/main.rs`.
- depends_on story:merge-under-authority: both edit `crates/takt-cli/src/main.rs`; this story counts the `approval` records that story writes.
- depends_on story:confinement-seam: both edit `crates/takt-cli/src/main.rs` and `crates/takt/src/lib.rs`; this story lands last, after every code story of the epic, so the week's runs use the binary with all five seams in place. story:second-harness-adapter and story:story-done-end-to-end come before this story through story:confinement-seam; `crates/takt-cli/src/main.rs` is shared with them along that chain.

## Acceptance

`takt report` over the case directories of three repositories and seven daily `run --hours 2` invocations prints, per repository and day, units accepted, tokens, human interventions and defects escaped, where the first three are computed from `events.jsonl`, the decision documents and the stories' `approval` records, the fourth is the number the operator entered with its source, and the totals for one chosen day equal a hand count of that day's case directories.

## Out of Scope

Choosing the repositories; a dashboard or a hosted service (`docs/design/takt.md` § 5); running the schedule from inside takt. The per-run figures and the comparison against a promoted baseline are story:scorecard-reader's; this story sums them over a week and adds defects escaped.

## Ambiguities

- How an intervention is recorded: decided, a grant is a `takt approve` invocation recorded as an `approval` evidence on the story (story:merge-under-authority); the report counts those, and any other operator action only if the events record it.
- Which repositories: the operator's choice at scheduling time; not a relation, not blocked.
