---
format: aep.planning-md/3
id: story:takt-approve-keeps-the-deployments-paths
kind: story
status: draft
title: takt approve takes the run file's deployment_paths
summary: 'approve --config applies deployment_paths: a deployment''s file neither refuses the approval nor is discarded'
relations:
- serves: vision:safe-agentic-coding
- decomposes: epic:takt-as-a-loom-agent
- informed_by: story:git-excluded-files-are-not-step-changes
revision: 1
---
# Story: takt-approve-keeps-the-deployments-paths

## What is wrong

`takt approve` reads no run file, so it has no `deployment_paths`. A deployment's untracked file in the tree refuses
the approval as uncommitted work, and its record commit (`crates/takt/src/frontier.rs`, `commit_record`) discards with
`git clean`, so a deployment log written while `approve` runs would be deleted. Both are documented as a limitation in
wave 56; neither was seen in a trial.

Found by the wave 56 security review.

## Outcome

`takt approve --config <file>` takes the run file's `deployment_paths` and applies them as `takt run` does: a
deployment's untracked file neither refuses the approval nor is discarded or committed.

## Acceptance

- With `.autodev/run-1.log` untracked and the run file naming `.autodev/`, `takt approve --config` records the approval,
  the log is unchanged, and no commit names it: a test.
- Without `--config`, `takt approve` behaves as today: a test.
