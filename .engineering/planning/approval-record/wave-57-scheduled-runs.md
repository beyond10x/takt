---
format: aep.planning-md/3
id: approval-record:wave-57-scheduled-runs
kind: approval-record
status: approved
title: 'Wave 57: a run on a schedule and one served Connectors client'
summary: two units toward scheduled runs and connector actions
relations:
- decides: story:a-run-on-a-schedule
- decides: story:one-served-connectors-client
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-11-12T18:32:57Z", actor: "human:timo", revision: 2}
---
# Approval: wave 57

## Decision

The owner's request of 2026-11-12: a team's reconcile runs on takt with Connectors instead of scripts, "the loop being
in the takt app basically, instead of sending execution of just one such protocol, send it periodically. The schedule
would be given by command line basically, or by a config". Design: `docs/design/scheduled-runs-and-connector-actions.md`;
the plan-critic panel approved the set (review-result:scheduled-runs-acceptance-round-3,
review-result:scheduled-runs-design-round-2, review-result:scheduled-runs-parallel-round-2,
review-result:scheduled-runs-scope-round-2).

Wave 57 takes two units in one tree:

- unit a: story:a-run-on-a-schedule.
- unit b: story:one-served-connectors-client.

Both add one `pub mod` line to `crates/takt/src/lib.rs`; unit a is committed first. The plan's pull request and this
wave's merge into `main` wait for the owner's grant: the delegation of `repository.merge.judge_surface` to the planner
was revoked on 2026-10-29 (`.engineering/delegations.yaml`).
