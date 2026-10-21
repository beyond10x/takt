# Wave 29: file-finding without GNU realpath, and the Loom governor holds takt's cases

Stage: **open**. Shape: one dispatcher in one checkout, one wave branch `wave/29` cut from
`wave/28` at bbc87ef (loom-sdk 0.31.0), approved by the orchestrator under the owner's delegation.

## Units

| unit | story | where | commit |
|---|---|---|---|
| 1 | story:file-finding-resolves-paths-without-gnu-realpath | `scripts/file-finding` | 22476ae |
| 2 | story:governor-holds-takts-cases | `crates/takt/src/case.rs`, `ess/domains/`, `AGENTS.md` § Rules, `crates/takt/tests/governor_case_store.rs`, `crates/takt/tests/merge_under_authority.rs` | 32a0085 |
| 3 | story:darwin-job-has-flock | `.github/workflows/darwin.yml` | 015eee3 |
| 4 | story:takt-runs-its-tests-on-macos (aep 0.76.0 in the darwin job) | `.github/workflows/darwin.yml` | 42d8efa |
| 5 | story:validators-perform-as-effects | `crates/takt/src/validator.rs`, `evidence.rs`, `forge.rs`, `run.rs`, `ess/domains/`, `crates/takt/tests/effects.rs` | 87893a5 |
| 6 | story:confinement-seam | `crates/takt/src/confinement.rs`, `run.rs`, `git.rs`, `harness.rs`, `validator.rs`, `crates/takt-cli/src/main.rs`, `crates/takt/tests/confinement.rs`, `ess/domains/` | 79da7e4 |
| 7 | story:replay-offers-granted-actions | `crates/takt/src/case.rs`, `replay.rs`, `ess/domains/`, `crates/takt/tests/replay_after_grant.rs` | fc585c0 |
| review 6 | story:darwin-job-has-flock (flock 0.4.0 from its pinned source) | `.github/workflows/darwin.yml` | a01556d |
| review 1, 3, 4 | story:governor-holds-takts-cases, story:replay-offers-granted-actions, story:validators-perform-as-effects | `crates/takt/src/case.rs`, `replay.rs`, `ess/domains/`, `crates/takt/tests/effects.rs`, `governor_case_store.rs`, `replay_after_grant.rs` | cef0264 |
| review 2, 5 | story:confinement-seam | `crates/takt/tests/confinement.rs` | 4402062 |

story:the-loop-is-run-until-blocked is blocked by
dependency-blocker:loom-runtime-stops (a587509) and waits for a Loom release.

The next wave takes story:takt-evaluation-code-retired, story:harness-child-confined,
story:takt-uses-loom-session-executor and story:design-names-loom-as-the-runtime.

## Log

- 2026-10-21: wave opened from `wave/28` at bbc87ef. Units 1 and 2 dispatched. Unit 2 changes
  `AGENTS.md` § Rules, a judge surface; its merge waits for the owner's grant.
- 2026-10-21: darwin run 39624435516 at 015eee3 failed 203 tests on aep's protocol cache, 48 log
  lines of `Permission denied (os error 13)`; unit 4 moves the darwin job to aep 0.76.0.
- 2026-10-21: unit 2 committed. The governor's `decisions/NNNN.json` finds a granted action
  approval-required, so `takt replay` of a step taken after a grant refuses on a prompt mismatch;
  open for the wave's review. Unit 5 dispatched.
- 2026-10-21: darwin run 39628244272 at 42d8efa failed 104 tests: 0 on the protocol cache, 92 on
  the missing `bwrap`, 12 others. Unit 5 committed; unit 6 dispatched.
- 2026-10-21: story:takt-uses-loom-session-executor filed as a draft (5dbe12d). Three stories move
  to the next wave with it: takt-evaluation-code-retired, harness-child-confined and
  design-names-loom-as-the-runtime.
- 2026-10-21: story:the-loop-is-run-until-blocked blocked on Loom 0.31.0, gaps A-E in
  dependency-blocker:loom-runtime-stops (a587509). takt's exit codes stay as documented; the
  blocker stays open until Loom 0.32.0 is tagged. Unit 6 is story:confinement-seam.
- 2026-10-21: the owner granted the governor unit's merge at the wave's close, after a clean
  reviewer verdict at the merge sha, on condition that the replay gap is fixed in this wave
  (decision-blocker:governor-judge-surface-merge, 8aa91f9). story:replay-offers-granted-actions
  became unit 7.
- 2026-10-21: one grant covers the whole diff `main..<merge sha>` on a clean reviewer verdict at
  that sha (decision-blocker:wave-29-merge-grant, 0ad8de9). No delegation was written.
- 2026-10-21: unit 6 committed (79da7e4). The session-executor's test module keeps three
  `std::process::Command` starts: the crate depends on no takt crate (`harness_parity.rs:1198`).
- 2026-10-21: unit 7 committed (fc585c0). A replayed step is offered a granted action only where
  the record's `frontier` line shows the run had that grant.
- 2026-10-21: the wave's review at fc585c0 returned needs-revision with six findings: a denial
  after a grant read as a grant (1), the confinement scan evaded (2), `authority.yaml` read
  through a link (3), `add_evidence` public and the port guard narrow (4), two confinement tests
  failing with `CARGO_TARGET_DIR` set (5), and flock installed unpinned (6).
- 2026-10-21: darwin run 39674142829 at a01556d: the flock step built flock 0.4.0; 104 tests
  failed, 0 on flock, GNU realpath or `declare -A`; 101 log lines on the missing `bwrap`, 1 on a
  non-UTF-8 name APFS refuses. story:takt-runs-its-tests-on-macos stays active into the next
  wave: its acceptance needs zero failures with no test skipped.
- 2026-10-22: findings 1, 3 and 4 fixed (cef0264), 2 and 5 fixed (4402062), 6 fixed
  (a01556d). What is left of 4 and 2 is filed as story:evidence-port-is-the-only-writer and
  story:process-starts-refused-by-type.
