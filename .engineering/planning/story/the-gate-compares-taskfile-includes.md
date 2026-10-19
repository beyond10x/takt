---
format: aep.planning-md/3
id: story:the-gate-compares-taskfile-includes
kind: story
status: draft
title: The gate compares the files a Taskfile includes at the base
summary: 'an edit to a file a Taskfile pulls in through includes: ran and recorded pass'
tags:
- rsi
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
revision: 1
---
# Story: the-gate-compares-taskfile-includes

## Outcome

`gate.run` compares at the base every file a compared Taskfile pulls in through `includes:`, so a
step cannot change a gate command by editing an included Taskfile.

## Context

Found by the review of story:the-gate-compares-its-scripts-and-tool-config-at-the-base
(2026-10-19): an edit to `tasks/ci.yml`, included from `Taskfile.yml`, ran and recorded pass.

## Acceptance

- A step that edits a Taskfile named in `includes:` is refused before the gate runs, naming the
  file: a test.
