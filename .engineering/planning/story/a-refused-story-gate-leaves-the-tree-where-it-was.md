---
format: aep.planning-md/3
id: story:a-refused-story-gate-leaves-the-tree-where-it-was
kind: story
status: draft
title: A refused --story run leaves the tree where it was
summary: a --story refused for its gate or protocol tag does not leave the tree on the base
tags:
- finding
relations:
- serves: vision:safe-agentic-coding
- decomposes: epic:composable-agentic-work
revision: 2
---
## Outcome

A `takt run --story <id>` refused because of its story (its `--gate` names no approval-required action of the story's
protocol, or its protocol tag is refused) leaves the tree on the branch it was on before the run, with nothing
checked out by the run.

## Context

Found while implementing story:gate-flag-names-a-protocol-action (wave 54), by reading the code, not run. The `--gate`
check for a story runs inside the frontier's settings closure (`crates/takt-cli/src/main.rs`, `check_gate` in the
`settings_for` closure), once the story's tags are read. By then the listing step has checked out the base, and the
error path does not put the tree back. A refused protocol tag has the same gap today.

## Acceptance

With the tree on a branch other than the base, `takt run --story <id> --gate no.such.action` exits 1, and afterwards
`git symbolic-ref HEAD` names the branch the tree was on before, and no case directory and no `takt/` branch exist.
