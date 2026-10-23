---
format: aep.planning-md/3
id: approval-record:wave-35-merge
kind: approval-record
status: approved
title: The merge of wave 35 into main is granted on the approve at 3e9aa15
relations:
- decides: story:takt-runs-its-tests-on-macos
- decides: story:takt-confines-on-macos
- decides: story:wave-35-review-fixes
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-10-23T10:32:38Z", actor: "human:timo", revision: 2}
---
# Approval: the merge of wave 35

## Decision

approval-record:wave-35-macos, the orchestrator under the owner's delegation (2026-10-23), sets
the wave's end: `darwin.yml` green and `aarch64-apple-darwin` back in `release.yml`; the
orchestrator added loom 0.37.0 (approval-record:loom-0-37-0) and set the takt trial after the
wave. Both ends hold at 3e9aa15: darwin run 39936935309 success, `release.yml` builds the target
and publishes only after `darwin.yml`. The reviewer approves at 3e9aa15
(review-result:wave-35-3e9aa15). The diff touches judge surfaces (`crates/**`, `scripts/**`,
`.github/**`, `Cargo.toml`, the lockfiles, `.engineering/planning/**`);
`.engineering/delegations.yaml` names no delegation, so the grant is the owner's side's, recorded
here, and the planner merges on it, at the store commit that follows 3e9aa15 once the reviewer
confirms that commit.

## What it does not grant

A release, and so the macOS binary; the takt trial, which the orchestrator starts.
