---
format: aep.planning-md/3
id: dependency-blocker:loom-session-executor-partial-runspec
kind: dependency-blocker
status: cleared
title: Loom's session executor takes no partial RunSpec from its caller over its per-kind defaults
relations:
- blocks: story:harness-section-is-metaharness-session-request
- blocks: story:harness-flags-spell-metaharness-names
revision: 3
transitions:
- {from: "open", to: "cleared", at: "2026-10-29T00:12:30Z", actor: "human:timo", revision: 3}
---
# Dependency blocker: loom-session-executor-partial-runspec

## What blocks

story:harness-section-is-metaharness-session-request (acceptance item 1, codex effort passed) and
story:harness-flags-spell-metaharness-names (acceptance item 1, the `--effort`, `--max-budget-usd`,
`--strict-version` and `--context` observables) need Loom's session executor to take a caller's partial `RunSpec` over
its per-kind defaults. takt's Loom 0.45.0 `SessionExecutor` has no effort field and hard-codes the `RunSpec` per kind
(approval-record:cfgproxy-accept-harness-flags).

Loom's store at `origin/main` 78a6d278a (tag 0.46.0) holds no story for it: no story under
`.engineering/planning/story/` names a partial `RunSpec` (`git grep -il`, 2026-10-28).

## What clears it

A Loom release whose session executor, reached through loom-sdk, merges a caller's partial `RunSpec` over its per-kind
defaults, so takt passes effort, budget, strict version and context to the session instead of refusing them.
