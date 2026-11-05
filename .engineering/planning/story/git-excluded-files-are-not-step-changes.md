---
format: aep.planning-md/3
id: story:git-excluded-files-are-not-step-changes
kind: story
status: proposed
title: Paths the run file names as the deployment's are not changes a step made
summary: a deployment's own files beside the tree, named in the run file and read at open, are not step changes (trial run 2)
tags:
- loom-agent
relations:
- serves: vision:safe-agentic-coding
- decomposes: epic:takt-as-a-loom-agent
- informed_by: decision-blocker:excluded-files-and-user-excludes
scope:
- confidence: inferred
  path: crates/takt/src/git.rs
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-10-26T13:48:10Z", actor: "human:timo", revision: 3}
---
## Outcome

A path the run's configuration file names as the deployment's (`crates/takt/src/run_config.rs`) is not counted as a
change a step made, so a deployment's own files beside the tree do not refuse or revert a step. The list is read once
at open, outside any step's reach. A file the repository's git excludes (`.git/info/exclude`, a global excludes file)
is not trusted for this: story:git-reads-no-user-excludes holds.

## Context

- Trial run 2: `.autodev/run-2.log`, written by the trial's wrapper into the workspace and excluded
  through `.git/info/exclude`, was set aside as a step change.
- decision-blocker:excluded-files-and-user-excludes chose option 3: the guard does not read an excludes file a step
  can write, and a deployment names its paths in the run file instead.

## Acceptance

1. With the run file naming `.autodev/` as the deployment's, a step whose only new file is `.autodev/run-2.log`
   records no change for it and is not refused for it.
2. A step that writes the run file or `.git/info/exclude` does not change the list for that step or later ones: the
   list is the one read at open.
3. A file excluded only through `.git/info/exclude` or a global excludes file, and not named in the run file, stays a
   change.

## Scope

- Inferred: `crates/takt/src/run_config.rs` (the list), `crates/takt/src/git.rs` (the uncommitted and changed paths),
  `crates/takt/src/run.rs` (the step guard).
