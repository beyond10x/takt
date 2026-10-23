---
format: aep.planning-md/3
id: approval-record:wave-35-macos
kind: approval-record
status: approved
title: 'Wave 35 puts takt on macOS: the Linux assumptions, Seatbelt confinement, the darwin release target'
relations:
- decides: story:takt-runs-its-tests-on-macos
- decides: story:takt-confines-on-macos
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-10-23T02:33:06Z", actor: "human:timo", revision: 2}
---
# Approval: wave 35, takt on macOS

## Decision

approval-record:loom-0-35-0-macos D3, under the owner's delegation (2026-10-23): after wave 34, a
macOS wave takes story:takt-confines-on-macos and the darwin failures that are not bubblewrap's,
and ends with `darwin.yml` green and the `aarch64-apple-darwin` target back in `release.yml`. The
orchestrator confirmed the order on 2026-10-23: release 0.9.0, the trial, then this wave; the wave
opens while 0.9.0 waits on decision-blocker:main-commit-with-personal-committer, because it does
not touch `main` until its merge.

## The wave, as the dispatcher orders it

| unit | story | why in this order |
|---|---|---|
| U1 | story:takt-runs-its-tests-on-macos | first: the 68 darwin failures that are not bubblewrap's (darwin run 39880890958 at 70919da) hide which of the 37 bubblewrap ones Seatbelt has to serve |
| U2 | story:takt-confines-on-macos | after U1, on `promotion.rs`, `forge.rs` and the tests U1 leaves |
| U3 | `release.yml` builds `aarch64-apple-darwin` again | last: only once `darwin.yml` is green on the wave branch |

The units run one after another in one tree, one local build at a time. macOS results come only
from `darwin.yml` on pushes of `wave/35`.

## Not in this wave

- story:takt-harness-trait-retired and story:loop-completes-with-two-legitimate-outcomes: no
  grant names them.
- The release that ships the macOS binary: a later grant.
