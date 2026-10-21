---
format: aep.planning-md/3
id: story:a-session-step-loads-no-auto-memory
kind: story
status: implemented
title: A judgement step through the metaharness session loads no auto-memory or other per-user context, so its first request is no larger than the print-mode step's
summary: 'Git-state probe: first request 7,100 tokens vs 6,275-6,461 at 0.6.0; the session init shows an auto-memory path the 0.6.0 runs lacked (cause inferred)'
tags:
- rsi
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-21T08:13:39Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-21T08:13:39Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-10-21T08:25:39Z", actor: "human:timo", revision: 4, decided_on: {"recorded":{"test_result":1}}}
---
# Story: a-session-step-loads-no-auto-memory

## Outcome

A judgement step that runs through the metaharness session loads no auto-memory and no other
per-user context Claude Code would add on its own: the session's start-up names no memory path,
and the step's first request is the size the print-mode step's was for the same prompt, or the
difference is measured and attributed.

## Context

From the wave 24 git-state probe (`~/.cache/takt-r0/w24/probe/result.md`, six live runs of
scenario 1's `tests.write` at main 6ad32d3, 2026-10-20): the first request of a step was 7,100
tokens in arm B (the copy holding the case tree's git state) against 6,275-6,461 at fd395d22 (0.6.0,
print-mode in the case tree), about 2k tokens a step that the git state does not explain. The
session's `system`/`init` event shows an auto-memory path the 0.6.0 runs lacked; that this path
accounts for the gap is inferred, not run. Since story:harness-proposes-takt-performs (wave 23)
the step runs through metaharness's `HermeticMode::Strict` with its own configuration home; the
print-mode harness ran with takt's isolation settings (`crates/takt/src/harness.rs`, `ISOLATION`).
Drafted on the orchestrator's word (2026-10-20). Comes from self-improvement work, so it carries
`rsi`.

## Scope

- `crates/session-executor/src/lib.rs` (the session's settings), `crates/takt/src/harness.rs`
  (`ClaudeSession`), tests; possibly a metaharness setting, which is metaharness's to add.

## Acceptance

1. A measurement first: the same step with and without auto-memory (and each other setting the
   init shows that the print-mode run lacked), three live runs each; the first request's tokens
   and the init's fields per run, each from the stream.
2. If it moves the first request, the session starts with auto-memory off (and the other per-user
   context off) through metaharness's own setting, or a request to metaharness for one; a test
   reads the setting the executor passes.
3. A run's init names no memory path; a test with a fake `claude` that prints its environment.
