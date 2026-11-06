---
format: aep.planning-md/3
id: story:an-unrelated-taskfile-does-not-refuse-every-gate
kind: story
status: draft
title: A fixture Taskfile the gate does not read does not refuse every gate
summary: one unresolvable fixture include or over 256 Taskfiles refuses every gate
tags:
- from-finding
- rsi
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
- informed_by: approval-record:wave-55-review-correction
revision: 1
---
# Story: an-unrelated-taskfile-does-not-refuse-every-gate

## Outcome

The include walk of the gate starts from the Taskfiles the gate's commands can read, so one fixture or vendored
Taskfile with an unresolvable include (`{{...}}`, a remote URL, an anchor), or a tree with more than 256 Taskfiles, does
not refuse every gate of the repository.

## Context

Found by the wave 55 security review at d0e8fba54 (`crates/takt/src/validator.rs:3608`): the walk starts from every
Taskfile-named file at any depth in the base, and fails closed on any one it cannot resolve, and past 256 in total. It
fails closed, so it is a usability gap, not a security one.

## Acceptance

- A repository whose gate runs `task check` from the root, with a fixture `tests/fixtures/Taskfile.yml` holding a
  templated include, records a gate result: a test.
- An edit to a Taskfile the root Taskfile includes is still refused: the existing cases stay green.
