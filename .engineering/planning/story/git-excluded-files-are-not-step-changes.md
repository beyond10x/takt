---
format: aep.planning-md/3
id: story:git-excluded-files-are-not-step-changes
kind: story
status: implemented
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
- confidence: inferred
  path: crates/takt/src/run.rs
- confidence: inferred
  path: crates/takt/src/run_config.rs
revision: 10
transitions:
- {from: "draft", to: "proposed", at: "2026-10-26T13:48:10Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-11-06T04:03:36Z", actor: "human:timo", revision: 6}
- {from: "active", to: "implemented", at: "2026-11-06T20:55:02Z", actor: "human:timo", revision: 10, decided_on: {"recorded":{"test_result":1,"review_outcome":1,"verification":1}}}
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

## Status

Built in wave 56 (unit d, 9a67088ee), released in 0.34.0 from 2c4f1d7f4, the merge of pull request 63.

- `crates/takt/src/run_config.rs`: the run file's `deployment_paths`, read once at open; `.git`, `..`, absolute paths
  and a list covering the run file are refused.
- `crates/takt/src/git.rs`, `run.rs`, `case.rs`, `frontier.rs`: an untracked or ignored file under the list is no
  change at a step, at open, at take, on the return to base and in store writes, and no discard removes it. Since
  a19b058f2 an entry without `/` covers that path only, and a link there counts as a change.
- `crates/takt/tests/deployment_paths.rs` (red, then green).

Review: review-result:w56-adv-1; follow-ups story:files-the-run-file-names-are-not-deployment-paths,
story:takt-approve-keeps-the-deployments-paths and story:git-reads-no-user-excludes.
