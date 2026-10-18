---
format: aep.planning-md/3
id: story:noprogress-names-the-resume
kind: story
status: draft
title: The NoProgress stop line says how to resume
relations:
- informed_by: review-result:variety-runs-a0b8576
- decomposes: epic:composable-agentic-work
- informed_by: story:approve-then-run-resumes-the-case
revision: 1
---
## Outcome

When a run stops on NoProgress, its stop line names the command that resumes it (`takt run --repo <repo> --case <id>`), not only the case path.

## Context

From the planner's variety runs of 2026-10-18 (.engineering/planning/review-result/variety-runs-a0b8576.md): the NoProgress stop line (`crates/takt-cli/src/main.rs:797`) gives the case path but not the resume command; the operator has to work out `--case <id>` from the path.

## Scope

- `crates/takt-cli/src/main.rs`: the stop line (cited)

## Acceptance

A run that stops on NoProgress prints a line containing `takt run --repo` and `--case <id>` with the stopped case's id, and running that line resumes the case.
