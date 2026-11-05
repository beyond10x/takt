---
format: aep.planning-md/3
id: approval-record:wave-54-four-findings
kind: approval-record
status: approved
title: 'Wave 54: four finding stories'
summary: one wave of four finding stories for 0.32.0, and the drafts it settles
relations:
- decides: story:a-git-read-error-leaves-no-process
- decides: story:resume-names-a-missing-branch
- decides: story:test-cmd-checked-before-open
- decides: story:gate-flag-names-a-protocol-action
- decides: story:decline-test-orders-cases-by-random-nonce
- decides: story:a-session-error-without-subtype-says-why
- decides: story:an-open-pull-request-is-found-on-resume
- decides: story:a-second-run-on-a-tree-is-refused-on-0-24
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-11-05T02:12:37Z", actor: "human:timo", revision: 2}
---
# Approval: wave 54

## Decision

The owner's standing delegation, 2026-11-04, through the orchestrator: takt has no planned wave left, and the one
active story, story:harness-child-confined, is blocked (dependency-blocker:loom-child-confinement). Wave 54 is drawn
from finding drafts, as waves 52 and 53 were. Each unit was checked as still present on `main` at 410aaddd2.

The wave is four units, run one after the other in one tree in the order a, c, b, d:

- unit a: story:a-git-read-error-leaves-no-process, the git command runner in `crates/takt/src/git.rs`.
- unit c: story:resume-names-a-missing-branch, the resume checkout in `crates/takt/src/case.rs`.
- unit b: story:test-cmd-checked-before-open, `TestsRun::new` in `crates/takt/src/validator.rs` and the test command's
  handling in `crates/takt-cli/src/main.rs`.
- unit d: story:gate-flag-names-a-protocol-action, the run flags' checks in `crates/takt-cli/src/main.rs`.

All four are accepted by this record, not by the agents that filed them. Units b and d both touch `main.rs`, in
different functions; d runs after b is committed. The close-out of wave 53 (`store/close-wave-53`) is merged into
`wave/54` first.

The same approval settles other drafts:

- story:decline-test-orders-cases-by-random-nonce is rejected: wave 52's
  story:two-trials-started-in-one-second-have-no-case-order delivered it.
- story:a-session-error-without-subtype-says-why is rejected as stale: the crate it names,
  `crates/session-executor`, no longer exists.
- story:an-open-pull-request-is-found-on-resume is rejected as delivered by 0.31.0. The command, GitLab and local
  forges implement `find_open_pull_request`, and the connectors forge keys each open, so a lost answer replays
  (story:pr-open-cut-off-by-a-crash-is-read-back).
- story:a-second-run-on-a-tree-is-refused-on-0-24 is merged into story:one-run-per-tree: its 0.24.0 reproduction and
  acceptance move there, and it is rejected as a duplicate. No tree-level hold exists on `main` yet.
- story:git-excluded-files-are-not-step-changes and story:git-reads-no-user-excludes contradict each other:
  decision-blocker:excluded-files-and-user-excludes asks which holds.

## Release

0.32.0: units b and d refuse input that was accepted before (a `--test-cmd` that cannot start or holds shell quoting,
a `--gate` naming no approval-required action, `--case-id` beside `--case`). Units a and c fix behaviour without
changing a contract.
