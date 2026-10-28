---
format: aep.planning-md/3
id: story:gates-project-checked-at-open
kind: story
status: active
title: A gates project file the gate would refuse refuses the case at open, before the first model call
summary: the pinned assertions project file is validated when the case opens
tags:
- priority:high
relations:
- decomposes: epic:downstream-story-delivery
- serves: vision:safe-agentic-coding
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-10-28T13:11:31Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-28T13:11:31Z", actor: "human:timo", revision: 3}
---
## Outcome

A gates project file that the gate would refuse (`timeout_seconds` outside `1..300`, an unknown format, a command whose program is found nowhere) refuses the case when it opens, before the first model call, naming the file and the reason.

## Context

A run of takt 0.17.0 on beyond10x/tools (2026-10-28; records outside the repository) had `timeout_seconds: 1800` refused at `gate.run`, after 7 steps and 559,856 tokens, exit 5. The project file is pinned when the case opens but checked only when the gate runs.

## Scope

- `crates/takt/src/run.rs`, `crates/takt/src/validator.rs`: the check at open
- `crates/takt/tests/`

## Acceptance

- `gates_project_checked_at_open`: a repository whose `.engineering/assertions-project.yaml` sets `timeout_seconds: 1800` stops before step 1 with 0 tokens and a message naming the file and `timeout_seconds`; a valid file opens as before.
