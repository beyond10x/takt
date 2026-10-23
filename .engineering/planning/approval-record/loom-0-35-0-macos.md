---
format: aep.planning-md/3
id: approval-record:loom-0-35-0-macos
kind: approval-record
status: approved
title: takt moves to loom-sdk 0.35.0; the macOS binary waits for a green darwin run
revision: 4
transitions:
- {from: "draft", to: "approved", at: "2026-10-23T00:07:49Z", actor: "human:timo", revision: 2}
---
# Approval: loom-sdk 0.35.0; the macOS binary waits for a green darwin run

## Decision

The orchestrator, under the owner's delegation (2026-10-23), after loom 0.35.0 shipped the macOS
build fix: pin loom tag 0.35.0, run `task check` and land through the bot route.

| id | decision | why |
|---|---|---|
| D1 | `Cargo.toml` pins `b10x-loom-sdk` at tag 0.35.0, on branch `wave/loom-0-35-0` from `main` | `darwin.yml` runs on `wave/**` pushes; wave 34 stays on its own branch |
| D2 | merge with `darwin.yml` red; `release.yml` keeps Linux only; no release for the pin alone | darwin run 39877488594 compiles on `macos-15` and fails 105 tests, which the pin cannot fix; 0.7.0 and 0.8.0 already ship Linux only, and a macOS binary would ship untested |
| D3 | after wave 34, a macOS wave takes story:takt-confines-on-macos and the darwin failures that are not bubblewrap's, and ends with `darwin.yml` green and the `aarch64-apple-darwin` target back in `release.yml` | the owner asked for loom and takt on macOS |

The 105 failures are counted in story:takt-confines-on-macos § Baseline.
