# Wave 30: a Codex step is given its context, and Loom 0.32.0 read against the epic

Stage: **closing**. Shape: one dispatcher in one checkout, one wave branch `wave/30` cut from
`main` at ae47c8c, approved by the orchestrator under the owner's delegation.

## Units

| unit | story | where | commit |
|---|---|---|---|
| 1 | story:codex-step-context (candidate `trial-cand/2026-10-21-bb4072a3-1` at 86d0433) | `crates/takt/src/harness.rs`, `crates/session-executor/src/lib.rs`, `crates/takt/tests/harness_proposes.rs`, `ess/domains/`, `Cargo.toml`, `Cargo.lock` | 5c04be4 |
| 2 | loom-sdk 0.31.0 -> 0.32.0 | `Cargo.toml`, `Cargo.lock`, `crates/takt/src/validator.rs`, `ess/domains/takt.yaml` | ccc356c |
| plan | the candidate's merge grant, story:harness-endpoint-is-configured, three trial findings, two Loom blockers, the completion order | `.engineering/planning/` | 4367634, fe49c0f, 4d5c8e9, 7eb852b, fdeb92b |

Unit 1 was measured on Codex, scenario 1, 5 runs a side: median run tokens 101,585 at the
candidate against 114,398 at the baseline (approval-record:candidate-bb4072a3-1-merge-grant).

## What waits on Loom

| story | blocker | open |
|---|---|---|
| story:the-loop-is-run-until-blocked | dependency-blocker:loom-runtime-stops | C, one breach choice for every limit; no stop for active time or low disk |
| story:harness-child-confined | dependency-blocker:loom-child-confinement | G1-G5: a live child, standard input, both output streams whole, read-only roots, host paths |

story:takt-evaluation-code-retired, story:takt-uses-loom-session-executor and
story:design-names-loom-as-the-runtime follow the loop story and wait with it.

## Log

- 2026-10-22: wave opened from `main` at ae47c8c. The candidate merged as unit 1 (5c04be4).
- 2026-10-22: Loom 0.32.0 tagged; unit 2 moved the pin (ccc356c), 70 citation lines
  re-anchored, `check-citations` 1057 of 1057.
- 2026-10-22: story:harness-child-confined moved to active and found blocked on five gaps
  (4d5c8e9). Three trial findings filed, tagged `rsi`; none of their predicates has run.
- 2026-10-22: dependency-blocker:loom-runtime-stops read against 0.32.0: A, B and D closed, C
  open, E changed (7eb852b). Gap E settled by taking Loom's declaration order (fdeb92b).
- 2026-10-22: the wave's review at 7eb852b approved with 0 blocking findings. `task check` at
  7eb852b: exit 0, 1661 passed, 0 failed, 5 ignored.
