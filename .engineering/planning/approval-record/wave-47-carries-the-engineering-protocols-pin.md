---
format: aep.planning-md/3
id: approval-record:wave-47-carries-the-engineering-protocols-pin
kind: approval-record
status: approved
title: Wave 47 carries the engineering-protocols 0.15.2 pin and a forge-neutral story.delivery/3
summary: 'two ride-along items: the 0.15.2 pin and pr.open through the run''s forge'
relations:
- decides: story:takt-carries-no-organisations-own-route
revision: 3
transitions:
- {from: "draft", to: "approved", at: "2026-10-30T04:53:53Z", actor: "human:timo", revision: 3}
---
# Approval: wave 47 carries the engineering-protocols 0.15.2 pin and a forge-neutral story.delivery/3

## Decision

The orchestrator, under the owner's grant (2026-10-30): two items ride along in wave 47.

1. takt's `b10x-canon-engineering` and `b10x-canon-engineering-assertions` pins move from
   engineering-protocols 0.14.0 to 0.15.2. Since 0.14.0 the `pr.open` action of
   `story.delivery/1` and `/2` and the vocabulary say "through the run's forge"; 0.15.0 added
   `protocol.adopt/1`. Only the test targets that read those protocols run for it.
2. takt's own `protocols/story-delivery/3.yaml` no longer says "bot route": its `pr.open` opens
   the pull request through the run's forge, as part of U1
   (story:takt-carries-no-organisations-own-route).

## Covered

| item | what |
|---|---|
| pin | engineering-protocols 0.14.0 to 0.15.2, a commit of its own in wave 47 |
| protocol | `protocols/story-delivery/3.yaml`, `pr.open` text, in U1's commit |
