---
format: aep.planning-md/3
id: story:the-gate-holds-against-lint-allows-in-source
kind: story
status: implemented
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
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-11-06T04:03:36Z", actor: "human:timo", revision: 4}
- {from: "proposed", to: "active", at: "2026-11-06T04:03:36Z", actor: "human:timo", revision: 5}
- {from: "active", to: "implemented", at: "2026-11-06T12:28:43Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"test_result":1,"review_outcome":1,"verification":1}}}
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

## Status

Built in wave 55 (unit c, 5bcde0279, corrected in dbd7bba5f), released in 0.33.0 from fedfbc28c, the merge of pull request 62.

- `crates/takt/src/validator.rs`: lint-level attributes naming `warnings` or a lint group, keyed by scope, are compared at the base for every changed `.rs` file and the files it pulls in through `include!` and `#[path]`; one added refuses the gate; a linked `.rs` file, `cfg_attr` nesting past its cap and an unclosed attribute fail closed.
- `crates/takt/tests/gate_holds_against_lint_allows.rs` (red, then green) and the review cases in `w55_adversary.rs` and `w55_security.rs`.

Review: review-result:w55-adv-1, the scope, include, link and nesting findings fixed; a single-lint crate allow is story:a-crate-level-allow-of-a-denied-lint-is-refused.
