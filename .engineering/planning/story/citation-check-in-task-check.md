---
format: aep.planning-md/3
id: story:citation-check-in-task-check
kind: story
status: draft
title: task check fails on a stale read-from citation in ess/
summary: the citation checker is committed and runs in task check
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
scope:
- confidence: inferred
  path: AGENTS.md
- confidence: inferred
  path: Taskfile.yml
- confidence: inferred
  path: scripts/check-citations
revision: 3
---
# Story: citation-check-in-task-check

## Outcome

`task check` fails when a `read from:` citation in `ess/` names a line that no longer says what it is cited for, so a stale citation is caught in the wave that makes it stale.

## Context

Decided 2026-10-18 on the developer role's report: 68 of 300 `read from:` citations in the ESS domain went stale during wave 07 unnoticed, because `task check` has no citation step and the checker the developer role used lives in a scratch directory outside the repository. Every wave edits cited code, and each wave close spends calls re-pointing citations by hand.

## Expected before

On takt at 7b5aca3, holds; after the fix, fails:

- Change a cited line in `crates/takt/src/run.rs` so the citation no longer matches, then run `task check`: it exits 0.

## Scope

- `Taskfile.yml`: a citation step in `check` (inferred)
- the checker: a script under `scripts/` or a test under `crates/takt-cli/tests/` (inferred; the developer role scopes it)
- `AGENTS.md`: the command under Commands (inferred)

## Acceptance

`task check` runs the citation checker; with one cited line changed it exits non-zero and names the citation and the line; on main it exits 0 with every citation resolving.
