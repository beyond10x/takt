---
format: aep.planning-md/3
id: epic:scheduled-runs-and-connector-actions
kind: epic
status: draft
title: 'takt runs on a schedule and acts through Connectors: a resident run per slot, connector reads as evidence, writes under a grant'
summary: --every/--window/--until and the run file's schedule; protocol actions bound to Connectors operations
relations:
- serves: vision:safe-agentic-coding
revision: 3
---
# Epic: scheduled-runs-and-connector-actions

## Outcome

A team's reconcile runs on takt alone: `takt run` stays resident and starts one pass per slot of a schedule given by
flags or the run file (an interval inside calendar windows in a named time zone), and stops on a declared condition;
and a protocol/1 file's actions read and write through a served Connectors instance, reads as evidence bound to the
case revision, writes only under a grant (a declaration the repository owns, or `takt approve`), no credential in
takt, a harness or a case. One status message in a chat thread is posted once, edited when it changes and left alone
when it does not.

## Acceptance

- `takt run --every … --window … --time-zone … --until …` runs ticks and ends as story:a-run-on-a-schedule's tests say.
- A protocol/1 read action bound in `connector_actions` writes a `canon-evidence/1` record at the case revision
  (story:a-connector-read-action).
- A write action sends only under a grant and once per case (story:a-connector-write-under-a-grant); a write the
  repository declares needs no `takt approve`, any other is refused (story:connector-writes-under-a-delegation).
- `crates/takt-cli/tests/reconcile_example.rs` passes: post, edit, nothing, released (story:a-reconcile-example).

## Design

`docs/design/scheduled-runs-and-connector-actions.md`: § 3 the schedule, § 4 the connector and command actions, § 5 the
owner's decision on unattended writes, § 7 the slices and waves.

## Stories and waves

- Wave 57: story:a-run-on-a-schedule, story:one-served-connectors-client (both add one line to `crates/takt/src/lib.rs`).
- Wave 58: story:a-command-action.
- Wave 59: story:a-connector-read-action.
- Wave 60: story:a-connector-write-under-a-grant.
- Wave 61: story:connector-writes-under-a-delegation (decision-blocker:connector-writes-unattended, cleared 2026-11-12).
- Wave 62: story:a-reconcile-example.

## Out of scope

Cron expressions, schedules across repositories, Loom's CronJob route, a chat or forge model in takt, Connectors'
local CLI writes.
