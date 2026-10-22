# Wave 32: takt on loom-sdk 0.33.0, and what still waits on Loom

Stage: **closing**. Shape: one dispatcher in one checkout, one wave branch `wave/32` cut from
`main` at 7a4d396, approved by the orchestrator under the owner's delegation
(approval-record:wave-32-loom-0-33-0).

## Units

| unit | story | where | commit |
|---|---|---|---|
| 1 | the loom-sdk pin | `Cargo.toml`, `Cargo.lock` | 59b67b0 |
| plan | the wave's approval; both Loom blockers read against 0.33.0, and the decision on them | `.engineering/planning/` | 8eb9fac, 7c2ed9d, c7c7f06 |

Unit 1 pins `b10x-loom-sdk` at tag 0.33.0 and moves the 13 loom packages in `Cargo.lock` to
3d56bca9. No takt source changes: none of 0.33.0's additions is used yet.

## What waits on Loom

| blocker | closed on 0.33.0 | still open |
|---|---|---|
| dependency-blocker:loom-runtime-stops | the token stop (`breach_by_limit`), the active-time stop (`host_stop`) | `host_stop` is asked before the approval and admissibility checks, so a low disk ends exit 8 where takt ends exit 4 |
| dependency-blocker:loom-child-confinement | G1, G2 and G4; G3 mostly; G5 for reads | a child writes only in `/workspace`; a run has one egress aperture, and the bot route needs two hosts |

Option A was taken: each blocker stays open until a Loom release closes its own row above.
Option B, accepting exit 8 where takt gives 4, was not taken: it changes takt's exit contract and
leaves confinement blocked.

## Log

- 2026-10-22: wave opened from `main` at 7a4d396 (8eb9fac).
- 2026-10-22: unit 1 (59b67b0); `cargo test --workspace` 1666 passed, 0 failed, 5 ignored, before
  and after; `check-citations` 1058 of 1058.
- 2026-10-22: both blockers read against 0.33.0, option A recorded (7c2ed9d).
- 2026-10-22: the wave's review at 7c2ed9d approved with 0 blocking findings; four notes on the
  blockers' text fixed (c7c7f06): each blocker's clearing condition, two headings, a gates citation
  and two line ranges.
- 2026-10-22: `task check` at c7c7f06: exit 0, 1666 passed, 0 failed, 5 ignored.
