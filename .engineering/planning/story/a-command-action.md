---
format: aep.planning-md/3
id: story:a-command-action
kind: story
status: active
title: A protocol action bound to an argument list produces evidence by its exit status
summary: 'command_actions: no shell, no credential, one declared output file, pass, unchanged, done or fail'
relations:
- decomposes: epic:scheduled-runs-and-connector-actions
- serves: vision:safe-agentic-coding
scope:
- confidence: cited
  path: crates/takt-cli/src/main.rs
- confidence: cited
  path: crates/takt-cli/tests/command_action_cli.rs
- confidence: cited
  path: crates/takt/src/command_action.rs
- confidence: cited
  path: crates/takt/src/lib.rs
- confidence: cited
  path: crates/takt/src/run_config.rs
- confidence: cited
  path: crates/takt/src/validator.rs
- confidence: cited
  path: ess/domains/takt.yaml
- confidence: cited
  path: website/docs/reference/configuration.md
revision: 12
transitions:
- {from: "draft", to: "proposed", at: "2026-11-12T20:59:46Z", actor: "human:timo", revision: 11, executor: "agent:b10x-takt", correlation: "wave/58"}
- {from: "proposed", to: "active", at: "2026-11-12T20:59:46Z", actor: "human:timo", revision: 12, executor: "agent:b10x-takt", correlation: "wave/58"}
---
# Story: a-command-action

## Outcome

A protocol/1 action that the run file's `command_actions` binds to an argument list is performed by one generic
validator: the argv without a shell, in the tree, with no credential, `{artifacts}` the case's artifacts directory;
its one output file is the declared `writes`, kept under `artifacts/`; exit 0 gives `pass`, a declared `unchanged_exit`
`unchanged`, a declared `done_exit` `done`, any other `fail`, in a `canon-evidence/1` record of the bound kind at the
case revision. The smallest piece of takt.md's deterministic step (M3), bound by name. It also adds the registration
of run-file-bound validators that story:a-connector-read-action reuses.

## Context

Design § 4. No run-file command produces evidence today (`crates/takt/src/evidence.rs:147-153`, :313-319); forge and
work commands run through `forge::Process` without credentials (`crates/takt/src/forge.rs:842-848`). Undeclared writes
under the case are refused by `Validator::writes` (`crates/takt/src/validator.rs:116`).

## Acceptance

- `command_action_results`: a command action writing its file and exiting 0, 3 (`unchanged_exit: 3`) and 4
  (`done_exit: 4`) gives three records with `pass`, `unchanged` and `done`, the file under `artifacts/`; exiting 1 gives
  `fail`.
- `command_action_undeclared_write`: a command writing a second file under the case, or a file in the tree, is refused
  and the change reverted.
- `command_action_no_credential`: with `GITHUB_TOKEN` and `CONNECTORS_TOKEN` set in takt's environment, the child's
  environment (printed into its file) holds neither.
- `command_action_unknown_action`: a binding of an action id the protocol does not declare is refused at its line,
  exit 2.
- Spec first: `takt.work.CommandAction` and the field `command_actions` on `takt.work.RunConfiguration`;
  `ess specify validate --path ess` exits 0.

## Scope

- new `crates/takt/src/command_action.rs`; `crates/takt/src/lib.rs` (one line); `crates/takt/src/validator.rs`
  (registration); `crates/takt/src/run_config.rs`; `crates/takt-cli/src/main.rs`; `ess/domains/takt.yaml`; new
  `crates/takt-cli/tests/command_action_cli.rs`; `website/docs/reference/configuration.md`.
