---
format: aep.planning-md/3
id: story:the-gate-holds-against-lint-allows-in-source
kind: story
status: active
title: A step cannot switch off the lint gate from source
summary: '#![allow(clippy::all)] in a crate root turns a failing clippy -D warnings green'
tags:
- rsi
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
scope:
- confidence: cited
  path: crates/takt/src/validator.rs
- confidence: inferred
  path: crates/takt/tests/gate_holds_against_lint_allows.rs
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-11-06T04:03:36Z", actor: "human:timo", revision: 4}
- {from: "proposed", to: "active", at: "2026-11-06T04:03:36Z", actor: "human:timo", revision: 5}
---
# Story: the-gate-holds-against-lint-allows-in-source

## Outcome

A step that adds a crate-level `allow` for a lint group the gate denies (`#![allow(clippy::all)]`,
`#![allow(warnings)]`) does not make `gate.green` hold: the gate refuses the change or runs with the
allows overridden.

## Context

Found by the review of story:the-gate-compares-its-scripts-and-tool-config-at-the-base
(2026-10-19): comparing `[lints]` at the base closes the manifest route, but source is the step's to
write, and an `allow` attribute in a crate root also turns a failing `cargo clippy -D warnings`
(exit 101) into exit 0.

## Acceptance

- A step that adds `#![allow(clippy::all)]` to a crate root, with a lint failure present, does not
  record `gate.green`: a test.
- An `allow` on a single item for a single lint still passes the gate.
