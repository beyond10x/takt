---
format: aep.planning-md/3
id: story:git-excluded-files-are-not-step-changes
kind: story
status: proposed
title: Git-excluded files are not changes a step made
summary: a deployment's excluded files beside the tree are not set aside as step changes (trial run 2)
tags:
- loom-agent
relations:
- serves: vision:safe-agentic-coding
- decomposes: epic:takt-as-a-loom-agent
scope:
- confidence: inferred
  path: crates/takt/src/git.rs
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-10-26T13:48:10Z", actor: "human:timo", revision: 3}
---
# Story: git-excluded-files-are-not-step-changes

## Outcome

A file the repository's git excludes (`.git/info/exclude`, a global excludes file) is not counted
as a change a step made, so a deployment's own files beside the tree do not refuse or revert a
step.

## Context

- Trial run 2: `.autodev/run-2.log`, written by the trial's wrapper into the workspace and excluded
  through `.git/info/exclude`, was set aside as a step change.

## Acceptance

1. A step whose only new file is excluded by `.git/info/exclude` records no change for it and is
   not refused for it.
2. A file excluded only by a path takt does not trust (none is named here; to be decided when the
   story is scoped) stays a change.

## Scope

- Inferred: `crates/takt/src/git.rs` (the uncommitted and changed paths), `crates/takt/src/run.rs`.
