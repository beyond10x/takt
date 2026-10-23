# Wave 35: takt on macOS, and loom 0.37.0

Stage: **closing**. Shape: one dispatcher in one checkout, one wave branch `wave/35` cut from
`main` at 68f6b6d, approved by the orchestrator under the owner's delegation
(approval-record:wave-35-macos); loom 0.37.0 joined it on approval-record:loom-0-37-0.

## Units

| unit | story | where | commit |
|---|---|---|---|
| U1 | story:takt-runs-its-tests-on-macos | the Linux assumptions in `crates/takt/src` and the tests that tripped on darwin | 3e1f9e0 |
| U2, U2b, U2c | story:takt-confines-on-macos | `crates/takt/src/seatbelt.rs`, `marked.rs`, `forge.rs`, `promotion.rs`, `scripts/check-finding`, tests | 8db2f50, 1575f99, 5c85e16 |
| U3 | story:takt-confines-on-macos | `.github/workflows/release.yml` builds `aarch64-apple-darwin` again | 9adc328 |
| U4, U4b, U4c | story:wave-35-review-fixes | the review's findings at 9adc328; `darwin.yml` callable, `release.yml` publishes only after it | a5d6878, bf8d4f9, 3e9aa15 |
| deps | approval-record:loom-0-37-0 | `Cargo.toml`, `Cargo.lock`, `fuzz/Cargo.lock`, `README.md` | c9319f9 |
| plan | the wave's approval, the review, loom 0.37.0 | `.engineering/planning/` | 0ba3645, 6303d09, c14e0b5, 54a9885, ffb021f |

`git diff --shortstat f4c377a 3e9aa15`: 48 files changed, 3965 insertions, 603 deletions.

On macOS each step takt confines with bubblewrap on Linux runs under a Seatbelt profile, and a
sweep kills a step's processes by the mark takt gave them. The release's `publish` job needs
`darwin.yml`, so no binary ships while a macOS test fails.

## Not closed in this wave

Four pre-existing findings of the review at 3e9aa15 went to the draft
story:sibling-git-dirs-are-sealed. The macOS binary ships with the next release, on its own grant.

## Log

- 2026-10-23: wave opened from `main` at 68f6b6d (0ba3645).
- 2026-10-23: U1 (3e1f9e0), U2 (8db2f50), U2b (1575f99), U2c (5c85e16), U3 (9adc328).
- 2026-10-23: review at 9adc328, changes-required: one high, four medium, four low; L7 accepted,
  the rest became U4 (c14e0b5).
- 2026-10-23: U4 (a5d6878); darwin run 39921796824 at a5d6878 timed out at 60 minutes, which
  sent U4 back for its tests (54a9885).
- 2026-10-23: `main` at f4c377a (release 0.9.0) merged in (c528010); loom 0.37.0 (c9319f9).
- 2026-10-23: U4b (bf8d4f9); darwin run 39931195654 at bf8d4f9 failed 2 tests; U4c (3e9aa15).
- 2026-10-23: darwin run 39936935309 at 3e9aa15, test-library and test-cli success; `task check`
  on CI, run 39936941949 at 3e9aa15: 157 results, 1672 passed, 0 failed, 5 ignored.
- 2026-10-23: review at 3e9aa15, approve (review-result:wave-35-3e9aa15); the three stories move
  to implemented on their test_result records; the merge granted (approval-record:wave-35-merge).
