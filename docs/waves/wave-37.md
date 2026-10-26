# Wave 37: takt as a Loom agent

Stage: **closing**. Shape: one dispatcher in one managed worktree, one wave branch `wave/37` cut
from `main` at 378159a, approved by the orchestrator under the owner's delegation
(approval-record:wave-37-takt-as-a-loom-agent). The stories are the ten takt gaps of the first
trial of takt 0.12.0 as a `LoomAgent` on loom 0.42.0 in a local kind cluster
(epic:takt-as-a-loom-agent), with engineering-protocols 0.10.0
(approval-record:wave-37-engineering-protocols-0-10-0) and loom 0.43.0
(approval-record:wave-37-loom-0-43-0) joining on the orchestrator's word.

## Units

| unit | story | where | commit |
|---|---|---|---|
| U1 | story:image-runs-the-toolchain-it-tests-with, story:gate-exec-finds-the-toolchain-in-the-image, story:image-ships-b10x-gates, story:image-cargo-home-is-writable | `Dockerfile`, `README.md` § Container | dfd4b0d, cda0d23 |
| U2 | story:endpoint-credential-from-environment | `crates/takt/src/harness.rs`, `EndpointArgs` in `crates/takt-cli/src/main.rs`, `ess/`, tests | 6f244ac |
| U3 | story:one-config-contract-with-loom | `crates/takt/src/run_config.rs`, `RunArgs::config` in `main.rs`, `ess/`, `Dockerfile` entrypoint, tests | fee81cb, 7d774c0 |
| U4 | story:run-names-its-story | `RunArgs` and the run's unit selection in `main.rs`, `ess/`, tests | e9ae544, 93f3c8d |
| U5 | story:image-harness-0-20-0 | `Cargo.toml`, both lockfiles, `Dockerfile` | 35873d9, b8fa404 |
| U6 | story:protocols-are-engineering-protocols-built-ins | `Cargo.toml`, `crates/takt/src/protocol.rs`, `protocols/`, tests, the viewer | f92b523 |
| review | the wave review's findings at cda0d23 | `harness.rs`, `run_config.rs`, README, CHANGELOG, `ess/`, the design | 18fc7de |

Not in the wave: story:git-excluded-files-are-not-step-changes and
story:edits-keep-ess-citations-resolved, proposed.

## Log

- 2026-10-26: the epic and its ten stories filed; the wave opened from `main` at 378159a.
- 2026-10-26: engineering-protocols 0.10.0 joined as U6 (9e4e3e8); U6 committed at f92b523,
  its adversary's three pre-existing lookup findings fixed in it.
- 2026-10-26: U1 at dfd4b0d, U4 at e9ae544, U2 at 6f244ac, U3 at fee81cb, each after or
  followed by its adversary: U2's blocker (a staged API key reached b10x-harness as a
  subscription OAuth token on `anthropic-messages`) fixed by admitting the environment
  credential on `openai-responses` only; U4's refused `--story` left the tree on the base, fixed
  at 93f3c8d; U3's Loom-strict reading at 7d774c0, a duplicate key's line left as a note.
- 2026-10-26: loom 0.42.0 was the newest tag and locked metaharness 0.15.0, so U5 waited
  (option B). `task check` at 7d774c0: 169 results, 1796 passed, 0 failed, 6 ignored, exit 0.
- 2026-10-26: U1's adversary: `/opt/rust` world-writable in the rust image (blocker), fixed at
  cda0d23 with the README's policy and toolchain notes; `task check` at cda0d23 the same counts.
- 2026-10-26: the wave review at cda0d23 (review-result:wave-37-cda0d230): approve, two
  warnings (PID 1 drops takt's re-raised signal; YAML `.inf` read as no `hours`) and four notes.
- 2026-10-26: loom 0.43.0 tagged with metaharness 0.19.0 before the gate closed: U5 at 35873d9
  on option A; the review's findings at 18fc7de; `task check` at 18fc7de: 169 results, 1797
  passed, 0 failed, 6 ignored, exit 0. U5's adversary: the loom line Breaking and takt as PID 1
  with no handler outside a staged token, fixed at b8fa404 (the image runs takt under `tini`).
- 2026-10-26: `task check` at b8fa404: 169 results, 1797 passed, 0 failed, 6 ignored, exit 0;
  citations 1158/1158. Nine stories implemented on it and their reviews; the probe image
  as uid 10001: takt, b10x-gates 0.1.23, cargo 1.99.1, b10x-harness 0.20.0.
- 2026-10-26: `release/0.11.0` and `release/0.12.0` kept: neither tip is on `main`.
