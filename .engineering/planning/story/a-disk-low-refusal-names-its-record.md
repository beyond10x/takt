---
format: aep.planning-md/3
id: story:a-disk-low-refusal-names-its-record
kind: story
status: draft
title: The DiskLow refusal names the record that holds the trial
summary: the refusal says the baseline for the series start and the rebaseline too
tags:
- rsi
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
revision: 1
---
# Story: a-disk-low-refusal-names-its-record

## Outcome

When `takt promote` refuses because a trial stopped DiskLow, the refusal names the record that holds
it: the latest promoted record, the series start, the rebaseline that added the scenario, or the
candidate.

## Context

Found by the review of the wave 15 merge of the judge lanes (2026-10-19): `enough_trials`
(`promotion.rs:3094-3101`) always says "the baseline has DiskLow trials", so an operator who reads
the latest baseline finds it clean while the trial sits in the series start. The rule holds; the
message misleads. The same review's probes cover DiskLow with a missing side, which only the fuzz
target checks (`scorecard.rs:1158-1168`).

## Acceptance

- A DiskLow trial in the series start is refused naming the series start's record; the same for the
  rebaseline and the candidate: a test each.
- A unit test covers a DiskLow trial with a missing side.
