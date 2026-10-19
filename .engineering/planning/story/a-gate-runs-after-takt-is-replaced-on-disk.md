---
format: aep.planning-md/3
id: story:a-gate-runs-after-takt-is-replaced-on-disk
kind: story
status: draft
title: A gate observation completes after the running takt is replaced on disk
summary: current_exe() reads '(deleted)' after a promotion installs mid-run; the gate digests that path
tags:
- rsi
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
revision: 2
---
# Story: a-gate-runs-after-takt-is-replaced-on-disk

## Outcome

A gate observation in a run whose takt binary was replaced on disk while it runs (a promotion
installing a new takt) still completes: the digest of the running binary is read from the running
image, not from a path that now names a deleted file.

## Context

Found by the wave 15 timing review (2026-10-19), inferred from code plus a std probe: after a
rename-replace, `current_exe()` reads `… (deleted)`, and the gate's prepare step digests that path
(engineering-protocols `crates/canon-engineering-assertions/src/lib.rs:278` at 53a221b), which
fails with ENOENT. A promotion installs a new takt while other runs may be going on the same host.
The fix belongs upstream in engineering-protocols; takt takes it with the next pin.

## Acceptance

- A run whose binary is renamed away and replaced mid-run completes its next gate observation: a
  test that copies the takt binary, starts a run from the copy, replaces the copy, then lets the gate
  run.
- The upstream change and the pin bump are named in the commit that closes this story.
