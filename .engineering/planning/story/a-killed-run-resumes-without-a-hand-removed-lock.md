---
format: aep.planning-md/3
id: story:a-killed-run-resumes-without-a-hand-removed-lock
kind: story
status: draft
title: A killed run resumes without removing run.lock by hand
summary: the lock names its holder, and a dead holder's lock is taken over
tags:
- trial-0.24.0
revision: 1
---
## Outcome

A case whose run was killed with SIGKILL can be resumed with `takt run --case <id>` without anyone
removing `run.lock` by hand, while a live holder still keeps a second run out.

## Context

The 0.24.0 trial cycle killed takt with SIGKILL during step 3: `run.lock` stayed, holding only an
instant, and the resume exited 1 asking for the file to be removed. A pod killed for memory left
the same lock. `take_hold` (`crates/takt/src/case.rs:517`) records no pid, host or boot id, so no
holder can be told dead.

## Acceptance

1. The lock records the holder's pid, host and boot id. After SIGKILL of the holder,
   `takt run --case <id>` takes the hold over, naming the dead holder, and continues the case.
2. While the holder is alive, a second `takt run --case <id>` exits 1 naming the lock and the live
   holder, as today.
3. A lock written on another host, or one whose holder cannot be read, is not taken over; the
   message says which.
