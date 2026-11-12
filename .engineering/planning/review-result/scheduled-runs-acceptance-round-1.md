---
format: aep.planning-md/3
id: review-result:scheduled-runs-acceptance-round-1
kind: review-result
status: active
title: 'Acceptance critic, round 1: scheduled runs and connector actions'
tags:
- scheduled-runs
relations:
- reviews: epic:scheduled-runs-and-connector-actions
- reviews: story:a-run-on-a-schedule
- reviews: story:one-served-connectors-client
- reviews: story:a-command-action
- reviews: story:a-connector-read-action
- reviews: story:a-connector-write-under-a-grant
- reviews: story:a-reconcile-example
- reviews: story:connector-writes-under-a-delegation
revision: 1
---
# Acceptance critic, round 1: scheduled runs and connector actions

```findings
[
{"file":".engineering/planning/epic/scheduled-runs-and-connector-actions.md","line":14,"category":"acceptance","severity":"blocker","verdict":"needs-revision","origin":"introduced","message":"the body has no Acceptance section; only Outcome, Design, Stories and Out of scope, while sibling epics carry one"},
{"file":".engineering/planning/story/one-served-connectors-client.md","line":35,"category":"acceptance","severity":"blocker","verdict":"needs-revision","origin":"introduced","message":"the acceptance reads the same before the work as after, since existing forge tests pass today and no item shows the forge's private exchange/served_answer replaced by the new module"},
{"file":".engineering/planning/story/a-run-on-a-schedule.md","line":64,"category":"acceptance","severity":"warning","verdict":"needs-revision","origin":"introduced","message":"the item joins three independent outcomes, and 'sets the same' names nothing a test can compare"},
{"file":".engineering/planning/story/a-reconcile-example.md","line":41,"category":"acceptance","severity":"warning","verdict":"needs-revision","origin":"introduced","message":"'takt protocol (or the loader) validates' names no fixed check, and takt protocol has only show, which refuses an unknown protocol"},
{"file":".engineering/planning/story/a-reconcile-example.md","line":40,"category":"acceptance","severity":"warning","verdict":"needs-revision","origin":"introduced","message":"the item does not say what the request log must hold per tick, or whether the released / --until outcome:released behaviour is part of the test"},
{"file":".engineering/planning/story/a-connector-read-action.md","line":45,"category":"acceptance","severity":"warning","verdict":"needs-revision","origin":"introduced","message":"the Outcome promises fail on a refusal, but no acceptance item checks a refused invoke producing a fail record"},
{"file":".engineering/planning/story/a-connector-write-under-a-grant.md","line":30,"category":"acceptance","severity":"warning","verdict":"needs-revision","origin":"introduced","message":"the Outcome says the grant binds at the case and subject revision, but no item shows that a grant at another revision sends nothing"},
{"file":".engineering/planning/story/a-command-action.md","line":46,"category":"acceptance","severity":"warning","verdict":"needs-revision","origin":"introduced","message":"the acceptance cites validator.rs:97-103 for Validator::writes, but fn writes is at validator.rs:116 and 97-103 is observe_within"}
]
```
