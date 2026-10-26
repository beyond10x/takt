# Wave 38: the defects of the second kind-cluster trial

Stage: **closed**, cut to its smallest slice (approval-record:wave-38-smallest-slice). Shape: one
dispatcher in one managed worktree (`takt-wave-38`), one wave branch `wave/38` cut from `main` at
368e356, one `target/`, approved by the orchestrator under the owner's delegation
(approval-record:wave-38-trial-s7-defects). The stories are the takt defects of the second trial
of takt as a `LoomAgent` in a local kind cluster (takt 0.13.1): 12 runs, 101 steps, 17,200,642
tokens, 0 accepted results.

## Units

| unit | story | where | commit |
|---|---|---|---|
| U1 | story:repeated-identical-test-failures-stop-the-run, story:a-red-base-stops-at-open | `crates/takt/src/run.rs`, `validator.rs`, `run_config.rs`, `crates/takt-cli/src/main.rs`, `ess/`, tests | ae146ab |
| re-pin | story:a-duplicate-yaml-key-is-refused-at-its-line (draft) | `crates/takt/tests/run_config_adv1.rs` | 9f3ac91 |
| U2 | story:edits-keep-ess-citations-resolved | `crates/takt/src/validator.rs`, `harness.rs`, tests | next wave |
| U3 | story:fresh-home-first-test-run-is-not-refused, story:staging-tests-pass-under-a-shared-temp-directory | `program_dirs` in `run.rs`, `harness::endpoint_tests` | next wave |
| U4 | story:a-harness-step-without-an-action-is-retried | `crates/takt/src/run.rs`, tests | next wave |
| U5 | story:run-configuration-names-the-story-and-token-variable, story:run-state-directory-is-configurable, story:scenario-run-takes-config | `run_config.rs`, `crates/takt-cli/src/main.rs`, `ess/`, tests | next wave |

U2 to U5 stay `active` in the store for the next wave. Their code, with each unit's adversary
pass, is on the branch `carry/wave-38-u4-u5` at 2db3e1d, which also holds U1 at 3 results in a
row; the next wave takes it from there against this wave's U1.

## Log

- 2026-10-26: the wave opened from `main` at 368e356; the nine stories accepted under
  approval-record:wave-38-trial-s7-defects; story:edits-keep-ess-citations-resolved's mechanism
  decided there and written into the story.
- 2026-10-26: the scope cut to U1 and the re-pin (approval-record:wave-38-smallest-slice); a
  repeated failure stops at 2 results in a row; no run on the host, the stops proven by
  `test_failure_stops_cli.rs` and its adversary file with a harness double that counts its calls.
- 2026-10-26: U1 at ae146ab after adversary pass 1 (review-result:adversary-u1-pass-1, two
  blockers and two warnings fixed); the re-pin at 9f3ac91. `cargo test --workspace --locked` at
  9f3ac91: 173 results, 1818 passed, 0 failed, 5 ignored, exit 0; citations 1166/1166.
- 2026-10-26: release 0.14.0 from this wave.
