# Wave 34: Loom's session executor, and Canon's evaluation path retired

Stage: **closing**. Shape: one dispatcher in one checkout, one wave branch `wave/34` cut from
`main` at 8e52ecb, approved by the orchestrator under the owner's delegation
(approval-record:wave-34-epic-close).

## Units

| unit | story | where | commit |
|---|---|---|---|
| 1 | story:takt-uses-loom-session-executor | `crates/session-executor` removed, `crates/takt/src/harness.rs`, `Cargo.toml`, `Cargo.lock`, `fuzz/Cargo.lock`, tests | a4a2e3e |
| 2 | story:takt-evaluation-code-retired | `crates/takt/src` (`decision.rs` removed, `case.rs`, `run.rs`), `crates/takt/tests/no_own_evaluation.rs`, `fuzz/fuzz_targets/scorecard.rs`, `ess/`, `AGENTS.md` | 59c523d |
| 3 | story:design-names-loom-as-the-runtime | `docs/design/takt.md`, `README.md`, `CHANGELOG.md` | 79ec8c0 |
| plan | the wave's approval, the review's findings | `.engineering/planning/`, `ess/` citations | 4aecf9b, 44f62b4 |

Unit 1 runs every harness session through `loom_sdk::session` (the crate
`b10x-loom-session-executor` at Loom 0.35.0) and drops takt's own `crates/session-executor`.

Unit 2 removes every `b10x_canon::eval` use from `crates/takt/src` and the file `decision.rs`;
`Decision` moves into `case.rs`, and evidence records are read as `EvidenceRecord` directly. The
new test `no_own_evaluation.rs` failed before the removal (1 passed, 1 failed) and passes after
(2 passed).

Unit 3 draws the loop in the design as one `run_until_blocked` call per step and names, for each
part, the crate that performs it.

## Not closed in this wave

The loop still reads its own evaluation in seven places in `run.rs`
(finding:loop-reads-its-own-decision). That work and the `Harness` trait went to a new draft,
story:takt-harness-trait-retired, so epic:takt-on-loom-sdk stays open after this wave.

## Log

- 2026-10-22: wave opened from `main` at 8e52ecb (4aecf9b).
- 2026-10-23: unit 1 (a4a2e3e); the affected test binaries, 4 results, 120 passed, 0 failed.
- 2026-10-23: unit 2 (59c523d), narrowed in the store to the evaluation path; the loop's other
  reads filed into story:takt-harness-trait-retired.
- 2026-10-23: gate at 59c523d, `task check` exit 0, 156 results, 1657 passed, 0 failed, 5
  ignored.
- 2026-10-23: unit 3 (79ec8c0).
- 2026-10-23: review at 79ec8c0, needs-revision with seven findings: 22 ESS citations into the
  design no longer resolved, and six lower ones.
- 2026-10-23: the findings at 44f62b4: the citations re-pointed (1054 resolved, 0 unresolved),
  four labels moved to Loom 0.35.0, the design's § 5 and acceptance 1 of unit 2 amended, three
  findings filed.
- 2026-10-23: check-static at 44f62b4 exit 0; review at 44f62b4, approve
  (review-result:wave-34-44f62b4).
- 2026-10-23: the merge granted (approval-record:wave-34-merge); the three stories move to
  implemented on their test_result records.
