---
format: aep.planning-md/3
id: story:ci-progress-events
kind: story
status: draft
title: While takt waits for CI, each check's state change is an event
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
- depends_on: story:run-event-contract
- depends_on: story:run-events-on-the-cli
- depends_on: story:ci-wait-watches-the-check
revision: 1
---
## Outcome

While `ci.wait` polls (story:ci-wait-watches-the-check), each check whose state changed since the last poll is one `takt-events/1` event `ci.check` (pull request, head revision, check name, state, bucket, link, started and completed instants); the wait's end is one event `ci.done` with the overall result.

## Context

Owner decision of 2026-10-19: "emit events while CI is progressing". The states come from the polled JSON form of `gh pr checks`; `--watch` prints text only.

## Scope

- `crates/takt/src/forge.rs`: the checks read (inferred)
- `crates/takt/src/run.rs`: the wait loop (inferred)
- `crates/takt/src/events.rs`: the two event types (inferred)

## Acceptance

With a fake `gh` that replays a scripted sequence (queued, in_progress, completed for two checks), the stream holds exactly the state changes in order and one `ci.done`; on scenario 1 against the real forge each check shows at least two events and `ci.done` precedes the next step's event.
