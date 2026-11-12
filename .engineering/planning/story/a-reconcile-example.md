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
scope:
- confidence: cited
  path: crates/takt-cli/tests/reconcile_example.rs
- confidence: cited
  path: examples/reconcile
- confidence: cited
  path: website/docs/guides
revision: 5
---
# Story: a-reconcile-example

## Outcome

`examples/reconcile/` holds a placeholder-named protocol/1 file (`reconcile/1`: thread and forge reads, a render, one
status write; outcomes `reconciled` and `released`), a run file and a render script. Under option A (every write
granted per case) a test drives three cases against a fake served instance, each granted with `takt approve` and
resumed with `takt run --case`: the first posts the status message, the second edits it after an input changed, the
third sends nothing. A schedule over the same run file with `--until outcome:released` ends at the tick whose render
reports done, with no write.

## Context

Design § 6. Composes story:a-run-on-a-schedule, story:a-connector-read-action, story:a-command-action and
story:a-connector-write-under-a-grant. Each tick is a fresh case, so a grant given to one tick's case is not used by
the next: scheduled writes need story:connector-writes-under-a-delegation (decision-blocker:connector-writes-unattended).

## Acceptance

- `reconcile_three_cases`: the fake instance's request log holds, per case: case 1 three reads and one
  `chat.postMessage`; case 2 three reads and one `chat.update` of the message case 1 posted; case 3 three reads and no
  write. Each case reaches `reconciled`.
- `reconcile_schedule_until_released`: `--every 1s --until outcome:released` over the run file, the render reporting
  done on its first tick: one line in `ticks.jsonl`, `end.json` reason `outcome:released`, exit 0, no write in the log.
- The loader accepts `examples/reconcile/protocol.yaml`: both tests open a case on it with `--protocol`.

## Scope

- new `examples/reconcile/`; new `crates/takt-cli/tests/reconcile_example.rs`; `website/docs/guides/` (one new page).
