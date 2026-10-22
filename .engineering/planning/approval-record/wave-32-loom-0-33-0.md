---
format: aep.planning-md/3
id: approval-record:wave-32-loom-0-33-0
kind: approval-record
status: approved
title: Wave 32 moves takt to loom-sdk 0.33.0 and takes the Loom-blocked stories
relations:
- decides: story:the-loop-is-run-until-blocked
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-10-22T04:27:48Z", actor: "human:timo", revision: 2}
---
# Approval: wave 32 moves takt to loom-sdk 0.33.0 and takes the Loom-blocked stories

The orchestrator, under the owner's delegation, 2026-10-22: "Move takt to b10x-loom-sdk 0.33.0 and
continue your loom-blocked work (dependency-blocker:loom-runtime-stops). No trials are running."

Loom 0.33.0 is tagged at 3d56bca9 (release notes: `StopPolicy.breach_by_limit`,
`LoopContext::host_stop`, `SubstrateRunner::spawn_child`, `ChildCommand::with_stdin`, read-only
roots and files, `at_host_path`).

## Covers

- The pin move of `b10x-loom-sdk` from 0.32.0 to 0.33.0 on `wave/32`.
- Reading dependency-blocker:loom-runtime-stops and dependency-blocker:loom-child-confinement
  against 0.33.0, and clearing what that release clears.
- story:the-loop-is-run-until-blocked once its blocker clears, and the merge of the wave into
  `main` through the bot route.
