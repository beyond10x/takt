---
format: aep.planning-md/3
id: dependency-blocker:metaharness-qualify-without-a-model-call
kind: dependency-blocker
status: open
title: metaharness has no qualification check without a model call
relations:
- blocks: story:an-unqualified-harness-stops-before-the-first-session
revision: 1
---
# Dependency blocker: metaharness-qualify-without-a-model-call

## What blocks

`story:an-unqualified-harness-stops-before-the-first-session` needs `metaharness::qualify`, re-exported by loom-sdk.
That function is metaharness `story:qualify-a-binary-without-a-model-call` (metaharness store, branch
`plan/takt-trial-review`).

## What clears it

The metaharness story's `qualify` exists in a working tree that takt's workspace reaches through uncommitted `[patch]`
path overrides (metaharness -> loom -> takt), and takt's scenario
`harness_preflight::an_unqualified_claude_stops_before_a_model_call` passes on that workspace. The releases then go out
in chain order as pin-only hops. No intermediate release is needed to clear this.
