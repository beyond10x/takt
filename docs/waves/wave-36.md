# Wave 36: takt on a GitLab host through glab

Stage: **open**. Shape: one dispatcher in one managed worktree, one wave branch `wave/36` cut
from `main` at a2bf296, approved by the orchestrator under the owner's delegation
(approval-record:wave-36-gitlab); decision-blocker:gitlab-trial-project cleared on the owner's
answer of 2026-10-23: a private project, kept outside the repository.

## Units

| unit | story | where | commit |
|---|---|---|---|
| U1 | story:gitlab-forge | `crates/takt/src/forge/gitlab.rs`, `forge.rs`, `run.rs` (the judge surface), `crates/takt-cli/src/main.rs`, `ess/`, tests | 2d0239f |
| U2 | story:gitlab-ci-wait | the pipeline read as `ci.read`'s checks, a scripted `glab` double, `ess/` | 6f8c42d |
| U3 | story:harness-endpoint-is-configured | `crates/takt/src/harness.rs`, the `harness` arm of `crates/takt-cli/src/main.rs`, `ess/`, tests | 8c171e0 |
| U3b | story:harness-endpoint-is-configured, acceptance 5 | `crates/takt/src/events.rs`, `run.rs` at the opened event, `harness.rs`, `ess/`, tests | 1782919 |
| fixes | the review's findings at 1782919 | `harness.rs`, `forge.rs`, `forge/gitlab.rs`, `run.rs`, `main.rs`, tests, `ess/`, README, CHANGELOG, the design | 370c9ed, db65aad |
| U3c | story:harness-endpoint-is-configured on loom 0.39.1 | `Cargo.toml`, both lockfiles, `harness.rs`, `main.rs`, tests, `ess/` | 731d111 |
| trial | U1, U2 | one real run on the private project, from a configuration outside the repository: case takt-20261023T225103Z, its merge request closed unmerged | |
| docs | U1, U3, U3b | `docs/design/takt.md`, `website/docs/getting-started.md`, by the doc-writer | a7e5fb0, a29fac3 |
| plan | the wave's approval, the blocker's answer, U3's scheduling | `.engineering/planning/` | 36937be |

U3 joined the wave on the orchestrator's word of 2026-10-23 (approval-record:trial-harness-arm-endpoint
§ Scheduling), ahead of the GitLab units in priority and beside them in the tree. The session
executor pinned at the wave's open (loom 0.39.0) carried no secret-name credential, so U3
delivered the flags, the file and both credential kinds and refused the secret kind by name;
loom 0.39.1 (released 2026-10-23T23:49Z) added `SessionExecutor::api_key_secret`, and U3c moved
the pin and handed the name down.

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
- 2026-10-24: the second real run, case takt-20261023T225103Z on `story.delivery/1`, `--harness
  claude --forge gitlab --test-cmd true --hours 2`: 52 steps, tokens 31892, 22:50:56Z to
  23:15:38Z. Step 8 `pr.open` pushed the case branch through glab and opened the merge request;
  44 `ci.read` steps, 43 pending at 30 s each, the 44th `ci_result` pass once the head pipeline
  (1084 s) succeeded; stopped `ApprovalRequired (repository.merge)`, exit 4; evidence files
  `0007-pull-request-8.json` and `0008-ci-result-95.json` in the case. The merge request was
  closed unmerged and both branches deleted on the project. Both GitLab stories have their run.
- 2026-10-24: U2 committed at 6f8c42d, U3b at 1782919, docs at a7e5fb0 and a29fac3.
  `cargo test --workspace` at a29fac3 found 2 stale forge assertions in the binary's unit tests,
  which U1's implementor had not run; fixed at 370c9ed. `task check` at 370c9ed: 160 results,
  1709 passed, 0 failed, 5 ignored, exit 0, run while the reviewer's two reverted mutations may
  have been in `gitlab.rs`, so it is run again at the close.
- 2026-10-24: the reviewer at 1782919: reject on F1 (the two assertions, 370c9ed), else accept
  with findings, six medium (review-result:wave-36-1782919). Fix round: A (F2, F3, F7, F10, the
  endpoint) and B (F4 to F9, F11, F13, F14, F17, the forge) dispatched; F12, F15 and the five
  unchecked edges filed as story:gitlab-forge-hardening.
- 2026-10-24: the fix round committed at db65aad, each finding with a test that fails without
  its fix; `cargo test --workspace --no-fail-fast` on that tree: 160 results, 1728 passed, 0
  failed, 5 ignored, exit 0; citations 1123/1123. story:gitlab-forge and story:gitlab-ci-wait
  moved to implemented on the run and the tests.
- 2026-10-24: loom 0.39.1 released with `SessionExecutor::api_key_secret` (the orchestrator's
  word, its verify-release exit 0); U3c at 731d111 moves the pin and hands a secret's name to
  b10x-harness, the endpoint naming no token; by the implementor: lib 113, 17 library suites 253,
  6 CLI suites 131 passed, 0 failed; citations 1127/1127. The design names 0.39.1 and the
  hand-down as built (c319340).
- 2026-10-24: `task check` at 731d111: 159 results, 1726 passed, 0 failed, 5 ignored, once
  `version_cli` was run again at a stable HEAD (its one failure compared a binary built at
  731d111 with a HEAD that had moved to c319340 during the run); citations 1127/1127; every
  other step exit 0. darwin.yml on wave/36 at 370c9ed: run 40038026587 success.
- 2026-10-24: the reviewer approves at 731d111 (review-result:wave-36-731d111): 14 findings
  fixed with tests, F12 and F15 filed, three new lows: N1 (a CHANGELOG line overstated what
  drops `GLAB_CONFIG_DIR`) fixed in the store commit, N2 and N3 filed in
  story:gitlab-forge-hardening. approval-record:wave-36-merge grants the merge under the
  orchestrator's grant; story:gitlab-forge, story:gitlab-ci-wait and
  story:harness-endpoint-is-configured are implemented. Stage: **closing**.
- 2026-10-24: the reviewer at 05c5708: needs-revision on four misstatements in the two review
  records as filed, none a finding or a verdict, and no code since 731d111: in wave-36-731d111
  the CLI suites' counts stood under the wrong names (forge_cli 3, gitlab_forge_cli 5,
  harness_endpoint_cli 14 is the order cargo runs the suites in) and the checks were placed at
  731d111 where they ran at c319340; in wave-36-1782919, F1 read as observed where it was
  established by reading the assertions, and F6 lacked "when glab is logged in to the host" and
  "read, not run". A review record is immutable, so both are archived and superseded by
  review-result:wave-36-1782919-2 and review-result:wave-36-731d111-2; the grant's merge point
  is the store commit after 731d111 that the reviewer confirms.
