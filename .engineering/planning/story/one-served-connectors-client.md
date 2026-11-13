---
format: aep.planning-md/3
id: story:one-served-connectors-client
kind: story
status: implemented
title: One served Connectors client for the forge, connector actions and work sources
summary: describe, invoke with any operation, the idempotency key and a per-caller refusal record
relations:
- decomposes: epic:scheduled-runs-and-connector-actions
- serves: vision:safe-agentic-coding
scope:
- confidence: cited
  path: crates/takt/src/connectors_client.rs
- confidence: cited
  path: crates/takt/src/forge/connectors.rs
- confidence: cited
  path: crates/takt/src/lib.rs
revision: 9
transitions:
- {from: "draft", to: "proposed", at: "2026-11-12T18:32:48Z", actor: "human:timo", revision: 7}
- {from: "proposed", to: "active", at: "2026-11-12T18:32:48Z", actor: "human:timo", revision: 8}
- {from: "active", to: "implemented", at: "2026-11-12T23:01:39Z", actor: "human:timo", revision: 9, decided_on: {"recorded":{"test_result":1,"review_outcome":1}}, executor: "agent:b10x-takt", correlation: "wave/57"}
---
# Story: one-served-connectors-client

## Outcome

One module, `crates/takt/src/connectors_client.rs`, calls a served Connectors instance: `GET /v1/describe`,
`POST /v1alpha2/invoke` with an operation name given at run time, the idempotency key, and the refusal record under a
file name the caller gives. The connectors forge runs on it with no change in behaviour. The connector actions
(story:a-connector-read-action, story:a-connector-write-under-a-grant) build on it, and
story:work-from-jira-github-and-gitlab can.

## Context

Design § 4. Today the client is private to the forge: `exchange` (`crates/takt/src/forge/connectors.rs:2082-2129`),
`preflight_served` (:1062-1121), `served_answer` (:1400-1445), the refusal record `pull-request.refusals.json` (:849,
:378-418), operations as `&'static str` on a forge-shaped struct; only `idempotency_key` (:182-194) is `pub`.

Shared file in wave 57: `crates/takt/src/lib.rs`, one `pub mod connectors_client;` line; story:a-run-on-a-schedule adds
one line there too and is committed first.

## Acceptance

- `forge/connectors.rs` defines no HTTP exchange, describe or invoke of its own: `grep -n "fn exchange\|fn served_answer\|fn preflight_served" crates/takt/src/forge/connectors.rs` prints nothing, and the forge calls `connectors_client`.
- Every connectors forge test passes: `cargo test -p b10x-takt --test connectors_forge` (12 tests) and
  `cargo test -p b10x-takt-cli --test connectors_forge_cli` (4 tests) exit 0, the same test names passing as before.
- `client_invokes_a_runtime_operation`: against a fake served instance, an operation name from a `String` is invoked
  and its answer returned; the request carries no `Authorization` header (unit test).
- `client_names_a_refusal`: an answer `approval_required` returns an error naming that code (unit test).
- `client_refusal_records_are_per_caller`: two callers in one case with two file names keep two records (unit test).

## Scope

- new `crates/takt/src/connectors_client.rs`; `crates/takt/src/forge/connectors.rs`; `crates/takt/src/lib.rs` (one line).
