---
format: aep.planning-md/3
id: approval-record:loom-0-35-0-macos
kind: approval-record
status: approved
title: takt moves to loom-sdk 0.35.0 and ships a macOS binary again in 0.8.1
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-10-23T00:07:49Z", actor: "human:timo", revision: 2}
---
# Approval: loom-sdk 0.35.0, and the macOS binary returns in takt 0.8.1

## Decision

The orchestrator, under the owner's delegation (2026-10-23), after loom 0.35.0 shipped the macOS
build fix: pin loom tag 0.35.0, run `task check`, land through the bot route, and release takt if
its process calls for one, so that loom and takt run on macOS.

| id | decision | why |
|---|---|---|
| D1 | `Cargo.toml` pins `b10x-loom-sdk` at tag 0.35.0, on branch `wave/loom-0-35-0` from `main` | `darwin.yml` runs on `wave/**` pushes; wave 34 stays on its own branch |
| D2 | `release.yml` builds `aarch64-apple-darwin` on `macos-15` again | 0.7.0 dropped it only because a Loom dependency did not compile on macOS |
| D3 | takt 0.8.1 is released after the merge, once `darwin.yml` is green at the merged revision | the macOS binary reaches people only through a release |

## Not in this change

- Wave 34 (approval-record:wave-34-epic-close) rebases onto this pin after the merge.
