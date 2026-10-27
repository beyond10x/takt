---
format: aep.planning-md/3
id: story:a-connectors-forge-adapter
kind: story
status: draft
title: An optional Connectors forge whose preflight names a missing operation
summary: never the default, lowest priority
tags:
- priority:low
relations:
- serves: vision:safe-agentic-coding
- decomposes: epic:downstream-story-delivery
- informed_by: story:forge-preflight
scope:
- confidence: inferred
  path: crates/takt/src/forge.rs
- confidence: inferred
  path: crates/takt/src/forge/connectors.rs
- confidence: inferred
  path: crates/takt/src/run_config.rs
- confidence: inferred
  path: crates/takt/tests
revision: 2
---
# Story: a-connectors-forge-adapter

## Outcome

An optional `forge: connectors` adapter on the `Forge` trait. It is never the default and has the lowest priority of
the forge adapters.
- It calls `connectors operations invoke` on a named instance, set by the run-file key `forge_instance`.
- **Preflight at open.** It runs `connectors operations list --output json` and requires the operations for change
  request create, read and merge and pipeline read, with write admitted. It prints the instance's identity.
- A missing operation, or a write that is not admitted, is exit 1 before the case opens, naming the operation.
- CI evidence is bound to the change request's head, as the GitLab forge binds it.

## Context

- Operator, 2026-10-27: Connectors stays an optional adapter, not the default.
- A downstream trial (2026-10-27) noted that its repository reaches its integrations through Connectors.
- The seam is `story:scan-and-forge-are-adapters`. `story:forge-preflight` (draft) covers the bot forge only.

## Acceptance

Each is a named conformance scenario run by `task check`, on a fake `connectors` that records invocations.

1. `connectors_forge::preflight_names_the_missing_write`: a fake listing read operations only gives exit 1 before the
   case opens, naming the create operation.
2. `connectors_forge::a_run_opens_and_reads_its_change_request`: the fake records create, then read, with the named
   instance.
3. `connectors_forge::it_is_never_the_default`: a run file without `forge: connectors` never invokes `connectors`.

## Scope

- `crates/takt/src/forge/connectors.rs` (new)
- `crates/takt/src/forge.rs` (adapter registration)
- `crates/takt/src/run_config.rs` (`forge_instance`)
- `crates/takt/tests/` (conformance scenarios)

## Out of scope

- The live instance, identity and approval policy. They are asked only when its live run is next.
