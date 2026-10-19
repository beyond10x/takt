---
format: aep.planning-md/3
id: story:a-store-move-keeps-its-citations
kind: story
status: draft
title: Moving a story in the planning store does not leave ESS citations unresolved
summary: an aep move shifts every cited body line by one
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
revision: 1
---
# Story: a-store-move-keeps-its-citations

## Outcome

Moving an artifact through its lifecycle does not leave the ESS citations into its body unresolved.

## Context

On 2026-10-19 the wave 14 implemented moves appended a transition line to each story's front matter,
shifting every cited body line by one; 12 ESS citations went unresolved, and a later move shifted 2
more. `scripts/check-citations fix` re-pointed them with digests unchanged. Citing story files by
line makes every store move a spec change.

## Acceptance

- Citations into `.engineering/planning/**` resolve by needle and heading, not by line number, or
  the store's write path runs the re-point: a test that moves a cited story and runs the citation
  check.
