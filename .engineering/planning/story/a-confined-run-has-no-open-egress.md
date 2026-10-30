---
format: aep.planning-md/3
id: story:a-confined-run-has-no-open-egress
kind: story
status: draft
title: A confined local run has no open egress
summary: a network namespace with only the model and git relays
tags:
- trial-0.24.0
revision: 1
---
## Outcome

A confined local run has no open egress: the only network it reaches is the model relay and the
git relay it was given.

## Context

In the 0.24.0 trial cycle, scenario 1 ran to ApprovalRequired inside bubblewrap with a read-only
root, an empty home holding only an access token and pid, ipc and uts namespaces, but egress was
open (github.com:443 reachable). `--unshare-net` needs a model relay, which 0.24.0 offered only for
`--harness harness`. An earlier sandbox that bound the real Claude settings read-write and left
the docker socket reachable had two escapes.

## Acceptance

1. A confined run of scenario 1 completes inside a network namespace whose only routes are the
   loopback model relay and git relay takt starts or is given.
2. From inside, a connection to an outside host on 443 fails, and the test shows it.
3. The documented sandbox binds no writable harness settings and no docker or other control
   socket; a test lists the binds and fails on either.
