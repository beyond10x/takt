---
format: aep.planning-md/3
id: review-result:scheduled-runs-scope-round-1
kind: review-result
status: active
title: 'Scope critic, round 1: scheduled runs and connector actions'
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
# Scope critic, round 1: scheduled runs and connector actions

```findings
[
{"file":".engineering/planning/story/a-run-on-a-schedule.md","line":36,"category":"scope","severity":"warning","verdict":"needs-revision","origin":"introduced","message":"the design's tick rules are claimed by no item: a tick still running at its next slot skips that slot, the skip is counted and never queued, missed slots after a suspend run once, and SIGTERM/SIGINT passes the signal to the running tick, writes end.json with reason signal and exits 3"},
{"file":".engineering/planning/story/a-connector-read-action.md","line":31,"category":"scope","severity":"warning","verdict":"needs-revision","origin":"introduced","message":"the endpoint rule 'a served instance; https, or http to loopback' is claimed by no item; the read story, which owns connectors.endpoint, does not say that plain http to a non-loopback host is refused"},
{"file":".engineering/planning/story/a-connector-write-under-a-grant.md","line":41,"category":"scope","severity":"warning","verdict":"needs-revision","origin":"introduced","message":"spec-first for ConnectorAction is narrowed to the read shape: the write story adds operations and input_from but has no spec-first acceptance line, and ess/domains/takt.yaml is not in its scope"},
{"file":".engineering/planning/story/one-served-connectors-client.md","line":25,"category":"scope","severity":"warning","verdict":"needs-revision","origin":"introduced","message":"the outcome says the Connectors work sources (story:work-from-jira-github-and-gitlab) use it, an outcome that belongs to a draft story outside this set which does not depend on this one; this story cannot deliver it and should say the module can be used by that story"}
]
```
