---
format: aep.planning-md/3
id: story:a-connector-read-action
kind: story
status: active
title: A protocol read action bound to a Connectors operation produces evidence and keeps the answer
summary: 'connector_actions read: one invoke on a served instance, the answer as the artifact, the record at the case revision'
relations:
- decomposes: epic:scheduled-runs-and-connector-actions
- serves: vision:safe-agentic-coding
- depends_on: story:one-served-connectors-client
- depends_on: story:a-command-action
scope:
- confidence: cited
  path: crates/takt-cli/src/main.rs
- confidence: cited
  path: crates/takt-cli/tests/connector_action_cli.rs
- confidence: cited
  path: crates/takt/src/connector_action.rs
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
- {from: "draft", to: "proposed", at: "2026-11-12T23:18:13Z", actor: "human:timo", revision: 11, executor: "agent:b10x-takt", correlation: "wave/59"}
- {from: "proposed", to: "active", at: "2026-11-12T23:18:13Z", actor: "human:timo", revision: 12, executor: "agent:b10x-takt", correlation: "wave/59"}
---
# Story: a-connector-read-action

## Outcome

A protocol/1 action with `effect: read` that the run file's `connector_actions` binds to a Connectors operation is
performed by one generic validator: one invoke on the served instance `connectors.endpoint` names, after one describe
that names `connectors.instance`; the answer is the observation's artifact `connector/<action>.json`; the record is
`canon-evidence/1` of the kind the binding names, `pass` on success and `fail` on a refusal, bound to the case
revision. `connectors.endpoint` is `https://`, or `http://` to a loopback host alone, as `forge_endpoint` is. The
protocol stays data: no action is modelled in Rust.

## Context

Design § 4. Validators bind to actions by name (`crates/takt/src/validator.rs:62-72`); the built-ins are a closed
table (:8191-8273); story:a-command-action adds the registration of run-file-bound validators, which this story reuses.
canon-evidence/1 has no payload field (`crates/takt/src/evidence.rs:23-62`). Depends on story:one-served-connectors-client
(the client) and story:a-command-action (the shared registration in `validator.rs`, `run_config.rs`,
`ess/domains/takt.yaml` and `configuration.md`).

## Acceptance

- `connector_read_pass`: a protocol file with one read action bound to a fake served instance: the case gets a `pass`
  record and `artifacts/connector/<action>.json` holding the answer, and the claim reading the record is TRUE.
- `connector_read_refused`: the fake instance answers the invoke with a refusal: the record is `fail` and the claim is
  FALSE.
- `connector_read_missing_operation`: a binding to an operation the describe does not list is refused before the case
  opens, naming it, exit 1.
- `connector_read_write_binding_refused`: a binding of an action the protocol declares `effect: write` without
  `operations` is refused at its line, exit 2.
- `connector_read_endpoint_rule`: `connectors.endpoint = "http://example.com"` is refused at its line, exit 2.
- `connector_read_no_credential`: no `Authorization` header and no environment credential reach the fake instance.
- Spec first: `takt.work.ConnectorAction`, `takt.work.ConnectorsEndpoint`, the fields `connectors` and
  `connector_actions` on `takt.work.RunConfiguration`; `ess specify validate --path ess` exits 0.

## Scope

- new `crates/takt/src/connector_action.rs`; `crates/takt/src/lib.rs` (one line); `crates/takt/src/validator.rs`
  (registration); `crates/takt/src/run_config.rs`; `crates/takt-cli/src/main.rs`; `ess/domains/takt.yaml`; new
  `crates/takt-cli/tests/connector_action_cli.rs`; `website/docs/reference/configuration.md`.
