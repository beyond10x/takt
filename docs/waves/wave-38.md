# Wave 38: the defects of the second kind-cluster trial

Stage: **open**. Shape: one dispatcher in one managed worktree (`takt-wave-38`), one wave branch
`wave/38` cut from `main` at 368e356, one `target/`, approved by the orchestrator under the
owner's delegation (approval-record:wave-38-trial-s7-defects). The stories are the takt defects
of the second trial of takt as a `LoomAgent` in a local kind cluster (takt 0.13.1): 12 runs, 101
steps, 17,200,642 tokens, 0 accepted results.

## Units

| unit | story | where | commit |
|---|---|---|---|
| U1 | story:repeated-identical-test-failures-stop-the-run, story:a-red-base-stops-at-open | `crates/takt/src/run.rs`, `validator.rs`, tests | |
| U2 | story:edits-keep-ess-citations-resolved | `crates/takt/src/validator.rs`, `harness.rs`, tests | |
| U3 | story:fresh-home-first-test-run-is-not-refused, story:staging-tests-pass-under-a-shared-temp-directory | `program_dirs` in `run.rs`, `harness::endpoint_tests` | |
| U4 | story:a-harness-step-without-an-action-is-retried | `crates/takt/src/run.rs`, tests | |
| U5 | story:run-configuration-names-the-story-and-token-variable, story:run-state-directory-is-configurable, story:scenario-run-takes-config | `run_config.rs`, `crates/takt-cli/src/main.rs`, `ess/`, tests | |

Order: U1 and U2 first, then U3 to U5. Commits: one per unit and its review fixes, the store
commits, the merge into `main`, release 0.14.0.

## Log

- 2026-10-26: the wave opened from `main` at 368e356; the nine stories accepted under
  approval-record:wave-38-trial-s7-defects; story:edits-keep-ess-citations-resolved's mechanism
  decided there and written into the story.
