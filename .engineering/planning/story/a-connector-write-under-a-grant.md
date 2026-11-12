---
format: aep.planning-md/3
id: story:a-connector-write-under-a-grant
kind: story
status: draft
title: A protocol write action bound to Connectors operations is sent once, under a grant
summary: connector.write under takt approve, an operation from an allowed set, the idempotency key on every write
relations:
- decomposes: epic:scheduled-runs-and-connector-actions
- serves: vision:safe-agentic-coding
- depends_on: story:a-connector-read-action
- depends_on: story:a-command-action
scope:
- confidence: cited
  path: crates/takt-cli/tests/connector_write_cli.rs
- confidence: cited
  path: crates/takt/src/connector_action.rs
- confidence: inferred
  path: crates/takt/src/run.rs
- confidence: cited
  path: crates/takt/src/run_config.rs
- confidence: cited
  path: ess/domains/takt.yaml
- confidence: cited
  path: website/docs/reference/configuration.md
revision: 10
---
# Story: a-connector-write-under-a-grant

## Outcome

A protocol/1 action with `effect: write` and `requires: [{capability: connector.write}]`, bound in `connector_actions`
with `operations` (the allowed set) and `input_from` (an artifact a command action wrote: `{operation, input}`), is
performed only under a grant at the case revision and its subject's revision (`takt approve <case> connector.write`).
The call carries the shared idempotency key, so a resumed case never sends twice; refusals are recorded per action. An
operation outside `operations` is refused and nothing is sent. The write action is the run's gate when the protocol
names no other (`default_gate`, `crates/takt/src/protocol.rs:300-330`).

## Context

Design § 4 and § 5. Grants bind to revisions (`crates/takt/src/case.rs:2062-2115`, :748-759); the grant's subject comes
from the action's precondition (`crates/takt/src/protocol.rs:188-228`); only the run's one gate action is
approval-gated (`crates/takt/src/run.rs:13119-13123`). The Connectors served host writes only under a standing grant
and needs an `idempotency_key` on every write. Across ticks the key does not deduplicate (each tick is a new case);
the render's comparison with the message read from the thread does. The grant is decided by the governor and
`CaseAuthority`, never by a check inside the validator: the merge validator's is the one gate in Rust (`AGENTS.md`
§ Rules). Depends on story:a-connector-read-action and story:a-command-action.

## Acceptance

- `connector_write_waits_for_a_grant`: without a grant the run stops at exit 4 and the fake instance's request log
  holds no invoke of the write.
- `connector_write_under_a_grant`: after `takt approve <case> connector.write` and `takt run --case <case>`, the log
  holds exactly one invoke of the write, with an `idempotency_key`.
- `connector_write_grant_at_another_revision`: a grant recorded at an earlier case revision sends nothing; exit 4.
- `connector_write_resumed_same_key`: the case resumed after a lost answer sends the same key.
- `connector_write_operation_not_allowed`: an `input_from` naming an operation outside `operations` is refused, the log
  holds no write.
- `connector_write_unchanged_sends_nothing`: an `unchanged` render: the case reaches its outcome and the log holds no
  write.
- `connector_write_not_the_gate`: with `--gate` naming another approval-required action, the write is not performed
  without its own grant: the log holds no write.
- `connector_write_no_credential`: no `Authorization` header and no environment credential reach the instance.
- Spec first: `operations` and `input_from` on `takt.work.ConnectorAction`; `ess specify validate --path ess` exits 0.

## Scope

- `crates/takt/src/connector_action.rs`; `crates/takt/src/run_config.rs`; `crates/takt/src/run.rs` (inferred: the gate
  for a run-file-bound write); `ess/domains/takt.yaml`; new `crates/takt-cli/tests/connector_write_cli.rs`;
  `website/docs/reference/configuration.md`.
