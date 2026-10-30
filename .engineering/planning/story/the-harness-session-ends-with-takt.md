---
format: aep.planning-md/3
id: story:the-harness-session-ends-with-takt
kind: story
status: draft
title: The harness session ends when takt dies
summary: a parent-death signal ends the harness child of a killed takt
tags:
- trial-0.24.0
revision: 1
---
## Outcome

When takt dies, the harness session it started ends with it, so a killed run spends no further
tokens that no record holds.

## Context

In the 0.24.0 trial cycle a harness stand-in was still running after takt was killed with
SIGKILL. `crates/takt/src/harness.rs:1053` starts the harness under `setsid`, in a session of its
own, and nothing ends it when takt dies: there is no parent-death signal.

## Acceptance

1. A harness child of a takt killed with SIGKILL is gone within 1 s, on Linux through a
   parent-death signal set in the child before exec, or an equivalent that survives `setsid`.
2. On a platform with no such mechanism the limit is stated in the documentation, and a resumed
   case names a session of the previous run still alive.
3. A harness session that ends normally is unaffected; its transcript is kept as today.
