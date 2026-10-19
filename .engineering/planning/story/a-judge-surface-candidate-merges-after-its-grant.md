---
format: aep.planning-md/3
id: story:a-judge-surface-candidate-merges-after-its-grant
kind: story
status: active
title: A judge-surface candidate counts as merged only after the turn holds its grant
summary: the first real turn's candidate was merged before the grant
tags:
- rsi
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-19T18:37:47Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-19T18:37:47Z", actor: "human:timo", revision: 3}
---
# Story: a-judge-surface-candidate-merges-after-its-grant

## Outcome

`candidate.merge_judge_surface` records `merged` only for a candidate that reached the mainline after
the turn recorded its grant; a candidate already on the mainline when the grant is recorded is
refused, naming the merge and the grant instants.

## Context

In the first real improvement turn (2026-10-19) the candidate was merged to main at 18:02:18Z, before
the turn opened at 18:03:23Z and before the owner's judge-surface grant, so the validator recorded a
merge that was already made. The grant then authorised nothing.

## Acceptance

- A candidate merged before the turn's grant is refused by the merge validator, naming both
  instants (merge commit time on the mainline, grant record time): a test.
- A candidate merged after the grant records `merged` as today.
