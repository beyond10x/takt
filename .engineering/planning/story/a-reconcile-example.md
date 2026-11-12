---
format: aep.planning-md/3
id: story:a-reconcile-example
kind: story
status: draft
title: 'An example reconcile: three ticks post, edit and leave one status message alone'
summary: 'examples/reconcile/: protocol, run file and render against a fake served instance'
relations:
- decomposes: epic:scheduled-runs-and-connector-actions
- serves: vision:safe-agentic-coding
- depends_on: story:a-run-on-a-schedule
- depends_on: story:a-connector-write-under-a-grant
- depends_on: story:connector-writes-under-a-delegation
scope:
- confidence: cited
  path: crates/takt-cli/tests/reconcile_example.rs
- confidence: cited
  path: examples/reconcile
- confidence: cited
  path: website/docs/guides
revision: 6
---
# Story: a-reconcile-example

## Outcome

`examples/reconcile/` holds a placeholder-named protocol/1 file (`reconcile/1`: thread and forge reads, a render, one
status write; outcomes `reconciled` and `released`), a run file with a schedule, a render script, and a
`.engineering/delegations.yaml` declaring the status write (one connector instance, `chat.postMessage` and
`chat.update`, one channel). A test runs the schedule against a fake served instance with no `takt approve`: the
first tick posts the status message, the second edits it after an input changed, the third sends nothing, and a
fourth whose render reports done reaches `released`, which ends the schedule (`--until outcome:released`).

## Context

Design § 6. Composes story:a-run-on-a-schedule, story:a-connector-read-action, story:a-command-action,
story:a-connector-write-under-a-grant and story:connector-writes-under-a-delegation (the owner's decision of
2026-11-12: no approval for a declared write).

## Acceptance

- `reconcile_scheduled_ticks`: the fake instance's request log holds, per tick: tick 1 three reads and one
  `chat.postMessage`; tick 2 three reads and one `chat.update` of the message tick 1 posted; tick 3 three reads and no
  write; tick 4 three reads and no write. `ticks.jsonl` holds 4 lines, `end.json` reason `outcome:released`, exit 0;
  no case holds a `takt approve` grant.
- `reconcile_write_outside_declaration`: the render naming another channel: the tick's case refuses the write, the log
  holds no write.
- The loader accepts `examples/reconcile/protocol.yaml`: the test opens its cases on it with `--protocol`.

## Scope

- new `examples/reconcile/`; new `crates/takt-cli/tests/reconcile_example.rs`; `website/docs/guides/` (one new page,
  by the doc-writer).
