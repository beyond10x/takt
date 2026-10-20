---
format: aep.planning-md/3
id: story:check-finding-runs-one-at-a-time
kind: story
status: implemented
title: check-finding runs one build at a time per machine and refuses under 20 GiB free
summary: parallel check-finding builds from two sessions took the root file system from 69 GiB free to under 1 GiB
tags:
- rsi
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
scope:
- confidence: cited
  path: scripts/check-finding
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-10-20T01:48:34Z", actor: "human:timo", revision: 4}
- {from: "proposed", to: "active", at: "2026-10-20T01:48:34Z", actor: "human:timo", revision: 5}
- {from: "active", to: "implemented", at: "2026-10-20T03:02:45Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"test_result":1}}}
---
# Story: check-finding-runs-one-at-a-time

## Outcome

`scripts/check-finding` runs at most one build at a time on a machine, whichever session starts
it, and refuses to build when the file system it builds on has less than 20 GiB free, naming the
free space and the threshold, before anything is built.

## Context

On 2026-10-20 the root file system went from 69 GiB free to under 1 GiB within minutes; another
project's wave stopped on it. The likelier cause was another project's parallel builds, not
`scripts/check-finding`, but several check-finding runs were in flight in the same hour, each a
full takt build in a run directory of its own, removed on exit, so their peak is the sum of the
runs in flight. A lock and a free-space refusal close that risk at little cost. Comes from reviewing self-improvement work (the predicate runs of a wave's findings),
so it carries `rsi`.

## Scope

- `scripts/check-finding`
- its tests (`crates/takt/tests/finding_scripts*.rs`)

## Acceptance

1. Two `scripts/check-finding` runs started at once on one machine build one after the other: the
   second waits on a lock under the cache directory check-finding already uses
   (`$XDG_CACHE_HOME/takt/check-finding`, `~/.cache/takt/check-finding` without it), and says it
   is waiting. A test with two runs whose build step is a stand-in that records its start and end.
2. A run started while the file system of that cache directory has less than 20 GiB free
   refuses before building, exit status the script uses for an error, naming the free space and
   the threshold; nothing is built. A test with the threshold lowered or the free space faked
   through an environment variable the script documents for tests.
3. A run killed while it holds the lock leaves no lock that blocks the next run (an advisory
   `flock` released with the process).

## Predicate

To be drafted with the wave's batched predicate run; until then this story is checked by its
acceptance tests only.
