---
format: aep.planning-md/3
id: story:a-git-read-error-leaves-no-process
kind: story
status: draft
title: A git call whose output cannot be read leaves no git process behind
tags:
- finding
refs:
- provider: finding
  reference: a-git-read-error-leaves-no-process
relations:
- decomposes: epic:composable-agentic-work
revision: 1
---
## Outcome

A git call whose stdout cannot be read leaves no git process behind: takt kills and waits on the
child before it returns the read error.

## Context

`run_git` (`crates/takt/src/git.rs:136`) returns on a stdout read error with `?` before
`child.kill()` and `child.wait()`, so the child is dropped unwaited and stays a zombie until takt
exits; its stderr reader thread is never joined. Found by a read-only review of 2026-10-21 at
0a61e14a.

## Scope

- `crates/takt/src/git.rs`, tests.

## Acceptance

1. On a stdout read error `run_git` kills and waits on the child and joins the stderr reader
   before it returns the error: a test.
