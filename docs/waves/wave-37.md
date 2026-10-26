# Wave 37: takt as a Loom agent

Stage: **open**. Shape: one dispatcher in one managed worktree, one wave branch `wave/37` cut
from `main` at 378159a, approved by the orchestrator under the owner's delegation
(approval-record:wave-37-takt-as-a-loom-agent). The stories are the ten takt gaps of the first
trial of takt 0.12.0 as a `LoomAgent` on loom 0.42.0 in a local kind cluster
(epic:takt-as-a-loom-agent).

## Units

| unit | story | where | commit |
|---|---|---|---|
| U1 | story:image-runs-the-toolchain-it-tests-with, story:gate-exec-finds-the-toolchain-in-the-image, story:image-ships-b10x-gates, story:image-cargo-home-is-writable | `Dockerfile`, `README.md` § Container | |
| U2 | story:endpoint-credential-from-environment | `crates/takt/src/harness.rs`, `EndpointArgs` in `crates/takt-cli/src/main.rs`, `ess/`, tests | |
| U3 | story:one-config-contract-with-loom | `crates/takt/src/run_config.rs`, `RunArgs::config` in `main.rs`, `ess/`, `Dockerfile` entrypoint, tests | |
| U4 | story:run-names-its-story | `RunArgs` and the run's unit selection in `main.rs`, `ess/`, tests | |
| U5 | story:image-harness-0-20-0 | `Cargo.toml`, the lockfiles, `Dockerfile`; only on a loom tag that locks metaharness 0.19.0 | |

Not in the wave: story:git-excluded-files-are-not-step-changes and
story:edits-keep-ess-citations-resolved, proposed.

## Log

- 2026-10-26: the epic and its ten stories filed; the wave opened from `main` at 378159a.
