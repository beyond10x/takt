---
format: aep.planning-md/3
id: story:runs-check-free-disk
kind: story
status: active
title: takt runs stop with a named refusal when free disk is low
summary: ENOSPC at 64M free killed edits mid-step on 2026-10-19
tags:
- rsi
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-19T15:58:59Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-19T15:58:59Z", actor: "human:timo", revision: 3}
---
# Story: runs-check-free-disk

## Outcome

`takt run` and `takt scenario run` check free space on the file system of the tree and of the
build directory before a step, a test command or a gate runs, and stop with a named refusal under
a threshold, instead of failing mid-step with ENOSPC.

## Context

2026-10-19 about 11:45Z the root file system reached 64M free during parallel builds. A developer
agent's first edits failed with ENOSPC and a wave gate had to be stopped. The operator scripts
used a hand-written `df-gate.sh` (15 GiB) before every build. takt itself has no such check, so a
case can fail in a way that looks like a code defect.

## Acceptance

- With free space below the threshold (default 10 GiB, `--min-free-gib` to change), a run stops
  before the step with exit 3 and a refusal naming the file system and the free space: a test with
  an injected free-space reader.
- Above the threshold nothing changes.
