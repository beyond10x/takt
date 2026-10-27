---
format: aep.planning-md/3
id: story:takt-ships-default-protocol-settings
kind: story
status: draft
title: takt ships one defaults document for protocol settings
summary: story.delivery/2, tests_first required, gate_on after_handover
tags:
- priority:high
relations:
- serves: vision:safe-agentic-coding
- decomposes: epic:protocol-settings-in-the-run-file
- depends_on: story:the-run-file-sets-protocol-settings
scope:
- confidence: inferred
  path: crates/takt-cli/src/main.rs
- confidence: inferred
  path: crates/takt/src/protocol.rs
- confidence: inferred
  path: crates/takt/src/protocol_defaults.yaml
- confidence: inferred
  path: crates/takt/tests/protocol_defaults.rs
- confidence: inferred
  path: website/docs
revision: 4
---
# Story: takt-ships-default-protocol-settings

## Outcome

takt ships one built-in defaults document, as data rather than Rust logic per protocol. It sets:
- the protocol for a store story: `story.delivery/2`;
- its inputs: `tests_first: required`, `gate_on: after_handover`.

The configuration reference page is generated from the document and drift-checked. Canon has no default on an input,
so takt supplies every value. A case whose declared input nobody sets (no flag, tag, run file or default) is refused at
open, naming the input and its values.

## Context

- Canon inputs are "fixed when the case opens" (canon `crates/canon/src/model/case.rs:41-44`) and have no `default:`.
- Owner decision of 2026-10-19 (`story:story-delivery-tests-first`): a story case writes its tests first.
- A downstream trial (2026-10-27) ran the gate three times per case, once on a known-red revision. `gate_on:
  after_handover` makes it once.

## Acceptance

Each is a named conformance scenario run by `task check`.

1. `protocol_defaults::a_store_story_opens_on_story_delivery_2`: with no run-file `protocol`, a store story opens on
   `story.delivery/2` with `tests_first: required` and `gate_on: after_handover`, source `default`.
2. `protocol_defaults::an_unset_input_is_refused_by_name`: a protocol whose input has no value anywhere is refused at
   open, naming the input and its values.
3. `protocol_defaults::the_reference_page_matches_the_document`: `task check` fails when the generated page drifts.

## Scope

- `crates/takt/src/protocol_defaults.yaml` (new data)
- `crates/takt/src/protocol.rs` (load defaults)
- `crates/takt-cli/src/main.rs` (default protocol, replacing `:2715`)
- `website/docs/` (generated configuration reference)
- `crates/takt/tests/protocol_defaults.rs`

## Delivery

This depends on engineering-protocols `story:story-delivery-2-with-settings`, recorded as
`dependency-blocker:story-delivery-2-with-settings`.
- It is delivered in one workspace: takt's `Cargo.toml` gets an uncommitted `[patch]` path override to that working
  tree, and scenario 1 is the end-to-end check.
- Then engineering-protocols releases and takt takes it as a pin-only hop. No intermediate release.
