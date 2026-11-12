---
format: aep.planning-md/3
id: review-result:scheduled-runs-design-round-1
kind: review-result
status: active
title: 'Design critic, round 1: scheduled runs and connector actions'
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
# Design critic, round 1: scheduled runs and connector actions

```findings
[
{"file":".engineering/planning/story/a-reconcile-example.md","line":26,"category":"design","severity":"blocker","verdict":"needs-revision","origin":"introduced","message":"its outcome has three scheduled ticks post and edit under option A, but each tick opens a fresh case with --intent, so a takt approve on tick N's case is never resumed and tick N+1 stops at exit 4 again"},
{"file":"docs/design/scheduled-runs-and-connector-actions.md","line":95,"category":"design","severity":"warning","verdict":"needs-revision","origin":"introduced","message":"the design states a schedule is not a case while design:continuous-orchestration § 6 makes every takt run open or resume one root case under run/1, and it does not say how the resident loop maps to that root case"},
{"file":"docs/design/scheduled-runs-and-connector-actions.md","line":91,"category":"design","severity":"warning","verdict":"needs-revision","origin":"introduced","message":"the design forwards SIGTERM to the running tick and exits 3, while design:continuous-orchestration § 5 makes SIGTERM a pause flag read at step boundaries with nothing forwarded"},
{"file":"docs/design/scheduled-runs-and-connector-actions.md","line":137,"category":"design","severity":"warning","verdict":"needs-revision","origin":"introduced","message":"the idempotency key includes the case and every tick opens a fresh case, so the key deduplicates only a resumed case, not a repeated tick as the design claims"},
{"file":".engineering/planning/story/a-run-on-a-schedule.md","line":36,"category":"design","severity":"warning","verdict":"needs-revision","origin":"introduced","message":"the body does not say what a second start with the same schedule id does when schedule.json names a live pid or end.json exists"},
{"file":".engineering/planning/story/work-from-jira-github-and-gitlab.md","line":1,"category":"design","severity":"warning","verdict":"needs-revision","origin":"introduced","message":"story:work-from-jira-github-and-gitlab is named as a user of the shared client by story:one-served-connectors-client and design § 4, but has no depends_on edge to story:one-served-connectors-client"}
]
```
