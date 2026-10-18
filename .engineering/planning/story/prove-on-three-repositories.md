---
format: aep.planning-md/3
id: story:prove-on-three-repositories
kind: story
status: archived
title: 'M5: a week of daily runs on three repositories'
summary: units accepted, tokens, interventions and escaped defects, against the controllers baseline
relations:
- decomposes: epic:composable-agentic-work
revision: 3
transitions:
- {from: "draft", to: "archived", at: "2026-10-18T01:43:14Z", actor: "human:timo", revision: 3}
---
## What

Three repositories, each a daily `takt run --hours 2` for seven days. Each run writes its events
(JSONL), its decisions and its AEP evidence; a report sums units accepted, tokens, human
interventions and defects escaped per repository and day.

## Acceptance

The report for the seven days exists with one row per repository and day, every number traceable
to a run's event file, set beside the baseline of the controllers' week it replaces.
