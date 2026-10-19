---
format: aep.planning-md/3
id: story:the-sweep-ends-in-a-pid-namespace
kind: story
status: draft
title: A scenario run's sweep ends its leftovers when takt runs in its own PID namespace
summary: under bwrap --unshare-pid every tests.run stopped StepRefused after 64 rounds of killing
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
revision: 1
---
# Story: the-sweep-ends-in-a-pid-namespace

## Outcome

When takt runs inside its own PID namespace, the sweep of what a step leaves running ends those
processes, and `tests.run` is not refused for leftovers it could not kill.

## Context

Found by the wave 15 trial-numbers review (2026-10-19): five scenario runs started each under
`bwrap --unshare-pid` all stopped `StepRefused (... processes still running after 64 rounds of
killing)`. The cause was not diagnosed.

## Acceptance

- A scenario run under `bwrap --unshare-pid --die-with-parent` completes its `tests.run` step: a
  test, run where bubblewrap is present and loud where it is not.
- The cause is named in the story before the fix.
