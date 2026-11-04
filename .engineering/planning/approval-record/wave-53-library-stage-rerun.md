---
format: aep.planning-md/3
id: approval-record:wave-53-library-stage-rerun
kind: approval-record
status: approved
title: 'Wave 53: the test-library stage counts on a targeted rerun'
summary: three GitLab double failures fixed in fixtures only; the stage is not run again in full
relations:
- decides: story:pr-open-cut-off-by-a-crash-is-read-back
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-11-04T22:07:11Z", actor: "human:timo", revision: 2}
---
# Approval: the wave 53 test-library stage counts on a targeted rerun

## Decision

The owner's standing delegation, 2026-11-04, through the orchestrator: the `task test-library` stage of wave 53's gate is
not run again in full. It ran at f54dd0c3a with 2056 passed and 3 failed, all three in
`crates/takt/tests/gitlab_ci_wait.rs`. The cause came in with story:pr-open-cut-off-by-a-crash-is-read-back: the
GitLab forge now reads the open merge requests from the case's branch before `pr.open` opens one, and that file's
`glab` double answered no such read. The fix changes only the test doubles, there and in
`crates/takt-cli/tests/gitlab_forge_cli.rs`, and no source.

The stage counts as green once `cargo test -p b10x-takt --test gitlab_ci_wait --test gitlab_api` passes on the commit
of that fix. The gate's record names both runs: the full run with its three failures and the targeted run. `task
test-cli` then runs at its floor, and CI runs the full gate on the pull request.
