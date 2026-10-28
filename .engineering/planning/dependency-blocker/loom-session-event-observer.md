---
format: aep.planning-md/3
id: dependency-blocker:loom-session-event-observer
kind: dependency-blocker
status: open
title: Loom's session executor calls no caller's observer for a session's events while it runs
summary: cleared by the first Loom release that ships Loom's story:session-executor-event-observer
tags:
- loom
relations:
- blocks: story:run-console-shows-the-session-live
revision: 1
---
# Dependency blocker: loom-session-event-observer

## What blocks

story:run-console-shows-the-session-live prints each event of a harness session while the
session runs. takt reaches the session only through Loom's session executor
(`b10x-loom-session-executor`, reached as `loom_sdk::session`; takt builds it at
`crates/takt/src/harness.rs:878`). In Loom 0.46.0, which takt pins (`Cargo.toml:16`), the
executor polls the session in one loop (`crates/loom-session-executor/src/lib.rs:967-986` at tag
0.46.0, `run.poll_event()` at `:974`), keeps each event in the report's transcript and calls
nothing of its caller's per event; takt sees the events only in the report, after the session has
ended. takt builds no substitute (`epic:takt-on-loom-sdk` § Out of scope).

## What clears it

The first Loom release that ships Loom's story:session-executor-event-observer: a
`SessionExecutor` that calls an observer its caller sets for every event while the session runs,
from that poll loop, and is reached through loom-sdk. takt then takes it as a pin hop in
story:run-console-shows-the-session-live. On 2026-10-28 that story was not yet in Loom's store at
`origin/main` 586b0ed83.

## How it is being delivered

The owner told the dispatcher (2026-10-28) to implement the Loom observer itself. It is being
written in a managed worktree of Loom. Loom's own delivery runs its adversarial pass, merges and
releases it, possibly together with the other two session-executor stories of that Loom wave.
