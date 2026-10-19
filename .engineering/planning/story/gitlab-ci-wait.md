---
format: aep.planning-md/3
id: story:gitlab-ci-wait
kind: story
status: draft
title: ci.wait and its progress events work on GitLab
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
- depends_on: story:gitlab-forge
- depends_on: story:ci-progress-events
revision: 1
---
## Outcome

`ci.wait` and its `ci.check`/`ci.done` events work on GitLab: takt polls `glab ci status -F json` for the merge request's head pipeline, with the same deadline, records and event types as on GitHub.

## Context

Owner decision of 2026-10-19: "later same thing with gitlab". `glab ci status -F json` is not compatible with `--live` (glab 1.115.0 `--help`), so the JSON form is polled, as on GitHub.

## Scope

- `crates/takt/src/forge.rs`: the GitLab checks read (inferred)

## Acceptance

story:ci-progress-events' fake-tool test passes with a fake `glab`; one real run on the GitLab trial project records `ci_result` and the events.
