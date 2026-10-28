---
format: aep.planning-md/3
id: approval-record:after-release-0-17-0
kind: approval-record
status: approved
title: 'After release 0.17.0: ESS 0.74.0, scenario 3''s declared red base, the scorecard rank'
summary: The pin moves to ESS 0.74.0 on main; scenario 3's seed declares its red base; Declined keeps its rank
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-10-28T09:13:36Z", actor: "human:timo", revision: 2}
---
# Approval: after release 0.17.0

## Decision

The orchestrator, under the owner's grant, 2026-10-28, after takt 0.17.0 was released and
verified. The pin follows the owner's standing rule to use the newest ESS release everywhere
(2026-10-05).

| item | decision | why |
|---|---|---|
| ESS pin | move to ESS 0.74.0 through a pull request into `main`, merged when its required checks are green; no release of its own | it rides the next takt release; takt uses the `ess` binary only, so the 0.74.0 library rename does not reach it |
| scenario 3's seed | todo-app-example's `.engineering/scenarios.conf` gives scenario 3 `red-base = expected`, one bot commit on its `main` | the trial is reproducible from that repository's `main` with no flag |
| `Declined` in the scorecard | keeps its default place in the ranking | no change |
| spelling | approval-record:wave-trial-0-16-findings names the key `red-base`, in a new revision | git's configuration syntax admits no `_` in a key |
