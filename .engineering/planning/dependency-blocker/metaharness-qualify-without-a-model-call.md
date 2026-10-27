---
format: aep.planning-md/3
id: dependency-blocker:metaharness-qualify-without-a-model-call
kind: dependency-blocker
status: open
title: metaharness has no qualification check without a model call
relations:
- blocks: story:an-unqualified-harness-stops-before-the-first-session
revision: 2
---
# Dependency blocker: metaharness-qualify-without-a-model-call

## What blocks

`story:an-unqualified-harness-stops-before-the-first-session` needs `metaharness::qualify`, reached through loom-sdk.
That function is metaharness `story:qualify-a-binary-without-a-model-call` (metaharness store, branch
`plan/takt-trial-review`).

## What clears it

Upstream state, observable before the takt story starts:
- The metaharness story's named tests (`qualify::*`) pass on its working tree.
- takt's workspace, with uncommitted `[patch]` path overrides metaharness -> loom -> takt, builds against that tree.

takt's own scenarios are then the end-to-end check of the chain, and the releases go out in chain order as pin-only
hops. No intermediate release is needed to clear this.
