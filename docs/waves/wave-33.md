# Wave 33: takt on loom-sdk 0.34.0, and the loop on run_until_blocked

Stage: **closing**. Shape: one dispatcher in one checkout, one wave branch `wave/33` cut from
`main` at 43d6531, approved by the orchestrator under the owner's delegation
(approval-record:wave-33-loom-0-34-0).

## Units

| unit | story | where | commit |
|---|---|---|---|
| 1 | the loom-sdk pin | `Cargo.toml`, `Cargo.lock` | ee76be8 |
| 2 | story:the-loop-is-run-until-blocked | `crates/takt/src/run.rs`, tests, `ess/domains/takt.yaml` citations | a23e242 |
| plan | the wave's approval and its option A, the exit-code register, the trial's count, the new Loom blocker | `.engineering/planning/`, `.agents/trial-agent.md` | 0e0c14a, 8e43994, 545c591, 7b54bad, 6c75bc0 |

Unit 1 pins `b10x-loom-sdk` at tag 0.34.0: the 13 loom packages move to 8cfac2cc, Loom's
metaharness from 0.10.1 to 0.13.0, and the four harness packages Loom pulled on 0.33.0 are gone
from `Cargo.lock`. No takt source changes.

Unit 2 drives every step through one `run_until_blocked` call (step budget 1): the due
validator's action first, else a harness judgement step, the stop derived from how the call
ended. Before each call, `Shape::only_behind` stops `ApprovalRequired` (exit 4) and
`Shape::unperformable` stops `Unperformable` (exit 7), ahead of the budget and low-disk stops
Loom asks through `host_stop` (approval-record:loop-pre-checks-ahead-of-loom). Exit codes and
the `takt run --help` table are unchanged.

## The probe that shaped unit 2

On 0.34.0 Loom hands every action its port performs, so without the two checks a delivery case
always looks movable without a grant:

| fixture | takt's own loop | Loom 0.34.0 alone |
|---|---|---|
| only a grant moves the case, low disk | ApprovalRequired, exit 4 | DiskLow, exit 8 |
| the deciding action has no performer | Unperformable, exit 7 | IdleBound, exit 3 |


Loom's documented contract rules out both stops on its side, so takt keeps the two checks until
that contract changes. Acceptance 5 of unit 2 (the scorecard against the promoted baseline over
the full corpus) is the trial agent's, after the release.

## Log

- 2026-10-22: wave opened from `main` at 43d6531 (0e0c14a).
- 2026-10-22: unit 1 (ee76be8); `cargo test --workspace` 1666 passed, 0 failed, 5 ignored.
- 2026-10-22: the orchestrator's two conditions on option A (8e43994): design § 5 registers 7 as
  Unperformable and 8 as DiskLow, the proposed Paused and FrontierExhausted move to 9 and 10;
  the trial counts an Unperformable run over its active cap as worse.
- 2026-10-22: unit 2 stopped before any change; a probe on 0.34.0 gave the table above
  (545c591).
- 2026-10-22: the orchestrator chose a Loom change toward 0.35.0 over accepting the new codes;
  recorded in dependency-blocker:loom-handed-frontier.
- 2026-10-22: gate at 545c591, `task check` exit 0, 157 results, 1666 passed, 0 failed, 5
  ignored (ess 0.66.0).
- 2026-10-22: Loom reported both stops break its documented contract; the orchestrator approved
  option C instead, two takt checks before each Loom step (7b54bad,
  approval-record:loop-pre-checks-ahead-of-loom); dependency-blocker:loom-handed-frontier cleared
  as worked around, and unit 2 resumed.
- 2026-10-22: unit 2 (a23e242); the implementor's `task check` exit 0, 1669 passed, 0 failed,
  5 ignored. The first gate at a23e242 failed one timing test under load average 37 and the
  second one test on a binary another build had replaced in the shared target; the reviewer's
  timing over 8 runs per revision showed no slowdown.
- 2026-10-22: review at a23e242, approve with four findings (6c75bc0,
  review-result:wave-33-loop-a23e242); story:loop-completes-with-two-legitimate-outcomes filed.
