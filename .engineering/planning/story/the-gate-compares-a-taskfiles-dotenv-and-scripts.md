---
format: aep.planning-md/3
id: story:the-gate-compares-a-taskfiles-dotenv-and-scripts
kind: story
status: draft
title: The gate compares a Taskfile's dotenv files and the scripts its cmds run
summary: 'dotenv: and cmds: scripts of a compared Taskfile are not compared at the base'
tags:
- from-finding
- rsi
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
- informed_by: approval-record:wave-55-review-correction
revision: 1
---
# Story: the-gate-compares-a-taskfiles-dotenv-and-scripts

## Outcome

`gate.run` compares at the base the files a compared Taskfile reads besides its includes: its `dotenv:` files and the
scripts its `cmds:` run, or refuses a Taskfile whose files it cannot name.

## Context

Found by the wave 55 adversary pass at d0e8fba54, outside the wave's acceptance: a Taskfile's `dotenv:` and the scripts
its `cmds:` run from the tree are not compared, so an edit there decides a gate the base defines. This is wider than
story:the-gate-compares-taskfile-includes.

## Acceptance

- A step that edits a file named in a compared Taskfile's `dotenv:` is refused before the gate runs, naming it: a test.
- A step that edits a script a compared Taskfile's `cmds:` runs from the tree is refused, or the Taskfile is refused at
  open, naming it: a test.
