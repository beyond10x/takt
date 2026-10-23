# Wave 36: takt on a GitLab host through glab

Stage: **open**. Shape: one dispatcher in one managed worktree, one wave branch `wave/36` cut
from `main` at a2bf296, approved by the orchestrator under the owner's delegation
(approval-record:wave-36-gitlab); decision-blocker:gitlab-trial-project cleared on the owner's
answer of 2026-10-23: a private project, kept outside the repository.

## Units

| unit | story | where | commit |
|---|---|---|---|
| U1 | story:gitlab-forge | `crates/takt/src/forge/gitlab.rs`, `forge.rs`, `run.rs` (the judge surface), `crates/takt-cli/src/main.rs`, `ess/`, tests | 2d0239f |
| U2 | story:gitlab-ci-wait | the pipeline read as `ci.read`'s checks, a scripted `glab` double, `ess/` | |
| U3 | story:harness-endpoint-is-configured | `crates/takt/src/harness.rs`, the `harness` arm of `crates/takt-cli/src/main.rs`, `ess/`, tests | 8c171e0 |
| U3b | story:harness-endpoint-is-configured, acceptance 5 | `crates/takt/src/events.rs`, `run.rs` at the opened event, `harness.rs`, `ess/`, tests | |
| trial | U1, U2 | one real run per story on the private project, from a configuration outside the repository | |
| plan | the wave's approval, the blocker's answer, U3's scheduling | `.engineering/planning/` | |

U3 joined the wave on the orchestrator's word of 2026-10-23 (approval-record:trial-harness-arm-endpoint
§ Scheduling), ahead of the GitLab units in priority and beside them in the tree. The pinned
session executor (loom 0.39.0) carries no secret-name credential, so U3 delivers the flags, the
file and both credential kinds, and refuses the secret kind by name until loom ships it.

## Log

- 2026-10-23: wave opened from `main` at a2bf296; U1 dispatched.
- 2026-10-23: U3 joined; U3 dispatched beside U1.
- 2026-10-24: U1 committed at 2d0239f, U3 at 8c171e0; the ESS citations of both land in 8c171e0,
  their lines computed on the tree carrying both. U2 dispatched; U3b (acceptance 5, the case
  records the endpoint's reference) dispatched.
- 2026-10-24: the first real run on the private project, on `software.change/1` (the protocol an
  `--intent` opens on), stopped `ApprovalRequired` at step 4, tokens 31945, exit 4, before any
  push: that protocol has no `pr.open`. The second run is on `story.delivery/1`, from a base
  branch of the project that carries a gates document (the project ignores `.*/`).
