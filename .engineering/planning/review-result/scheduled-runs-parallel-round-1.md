---
format: aep.planning-md/3
id: review-result:scheduled-runs-parallel-round-1
kind: review-result
status: active
title: 'Parallel-safety critic, round 1: scheduled runs and connector actions'
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
# Parallel-safety critic, round 1: scheduled runs and connector actions

```findings
[
{"file":"docs/design/scheduled-runs-and-connector-actions.md","line":200,"category":"parallel-safety","severity":"blocker","verdict":"needs-revision","origin":"introduced","message":"design § 7 puts story:a-reconcile-example in wave 59 next to story:a-connector-write-under-a-grant; its own depends_on edge orders it after that story and its three-tick acceptance reads the write action that story creates, so wave 59 holds a reader and its producer"},
{"file":".engineering/planning/story/a-command-action.md","line":52,"category":"parallel-safety","severity":"blocker","verdict":"needs-revision","origin":"introduced","message":"wave 58 pairs it with story:a-connector-read-action; both land on validator.rs, run_config.rs, ess/domains/takt.yaml and website/docs/reference/configuration.md and neither body names the other"},
{"file":".engineering/planning/story/one-served-connectors-client.md","line":11,"category":"parallel-safety","severity":"warning","verdict":"needs-revision","origin":"introduced","message":"the body's Scope names crates/takt/src/lib.rs but the scope field leaves it out, so the overlap with story:a-run-on-a-schedule in wave 57 on that file is invisible to aep plan artifact waves and neither body names the other"},
{"file":"crates/takt/src/lib.rs","line":9,"category":"parallel-safety","severity":"warning","verdict":"needs-revision","origin":"introduced","message":"inferred: story:a-connector-read-action and story:a-command-action each create a new module that needs a pub mod line in crates/takt/src/lib.rs, which neither scope lists"}
]
```
