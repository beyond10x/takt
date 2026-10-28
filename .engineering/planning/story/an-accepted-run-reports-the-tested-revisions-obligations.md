---
format: aep.planning-md/3
id: story:an-accepted-run-reports-the-tested-revisions-obligations
kind: story
status: active
title: An accepted run reports the obligations of the tested revision
summary: The last obligations line reads the implementation revision, not the records head above it
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-10-28T06:52:41Z", actor: "human:timo", revision: 4}
- {from: "proposed", to: "active", at: "2026-10-28T06:52:42Z", actor: "human:timo", revision: 5}
---
## Outcome

A run that stops `Completed (accepted)` reports the obligations of the revision the gate and the
tests judged, the case's implementation revision, not the records head takt committed above it.
When the gate passed at that revision, the last lines carry no
`obligations: N obligation open: check not yet run`; a gate that failed or was never run there is
still reported as open.

## Design

- The obligations are read at the case's implementation revision. takt's store commits above it
  (the test result record, the story's move to implemented) do not move it (`is_store_commit`,
  `run.rs:340-350`); a code commit does, so a status an earlier revision earned still never
  carries over to changed code. The module doc of `obligations.rs` and `current`'s doc move from
  "the revision the run is at" to "the case's implementation revision".
- `recorded` (`obligations.rs:98-104`) finds the last table recorded at the wanted revision, not
  the last table of the record, and `close_obligations` (`run.rs:10318-10340`) records the table it
  computed at the revision it computed it for, so a stop-time table never hides the gate's table
  on a resume or in `takt report`.
- story:a-run-can-decline adds a `Declined` stop that passes through `close_obligations`; it
  prints the obligations line as any stop that is not `Completed` does.

## Context

- Live trials on takt 0.16.0, todo-app-example scenarios 2 to 6: every accepted run printed
  `obligations: 1 obligation open: check not yet run` beside `stopped: Completed (accepted)`
  (scenario 2 run 2, scenario 3 run 3).
- `crates/takt/src/obligations.rs:6` keeps a status only for the revision it was observed at, and
  `obligations.rs:113` (`current`) reads the last table at the revision the run is at (`of_run`,
  `git::head`). Inferred from the code, not yet traced at runtime: at an accepted stop the tree is
  at takt's store commits above the tested revision, so no table is recorded at that head.

## Scope

- `crates/takt/src/obligations.rs`: `recorded`, `current`, `of_run` and the module doc (cited)
- `crates/takt/src/run.rs`: `close_obligations` (cited: 10318-10340); the prompt's call of
  `of_run` (~11925) is checked to read the same revision
- `crates/takt-cli/tests/obligations_report_cli.rs`: new cases (cited)

## Acceptance

1. `obligations_report_cli::an_accepted_run_reports_the_tested_revisions_obligations`: a delivery
   run whose gate passed at the implementation revision stops `Completed (accepted)` and its last
   lines hold no `obligations:` line; `takt report --format json` lists the gate `pass`.
2. `obligations_report_cli::an_accepted_run_with_a_failed_gate_still_reports_it_open`: an accepted
   run whose gate failed at the implementation revision prints `obligations:` naming the gate
   `fail` in its last lines.
3. `obligations_report_cli::a_code_change_after_the_gate_still_reads_not_yet_run`: a code commit
   after the gate pass reads the gate `not yet run` in the stop's `obligations:` line and in
   `takt report`.
4. `obligations_report_cli::a_stop_time_table_does_not_hide_the_gates_table`: a resume after a
   stop at takt's store commits reads the gate's status at the implementation revision in
   `takt report`, not `not yet run`.
