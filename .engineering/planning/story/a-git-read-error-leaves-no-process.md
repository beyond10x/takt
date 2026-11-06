---
format: aep.planning-md/3
id: story:a-git-read-error-leaves-no-process
kind: story
status: implemented
title: A git call whose output cannot be read leaves no git process behind
tags:
- finding
refs:
- provider: finding
  reference: a-git-read-error-leaves-no-process
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
scope:
- confidence: cited
  path: crates/takt/src/git.rs
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-11-05T02:14:02Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-05T02:14:02Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-11-06T03:10:31Z", actor: "human:timo", revision: 5, decided_on: {"recorded":{"test_result":1,"review_outcome":1,"verification":1}}}
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

## Status

Built in wave 54 (unit a, 5ab8c4d3f), released in 0.32.0 from 9e7a3904e, the merge of pull request 61.

- `crates/takt/src/git.rs`: on a read error `run_bounded` (through `finish_bounded`) and `diff_bounded` kill and wait
  for git and join the stderr reader before the error returns; a failed stdin write in `output_with` and `Isolated`
  kills and waits for git too.
- The unit test `git::tests::a_read_error_leaves_no_git_process_and_no_reader_thread` (red, then green); after CI it
  starts its git through `command()`, inside the confinement seam (e6012f1e9).

Review: review-result:w54-adv-1, no finding on this unit.
