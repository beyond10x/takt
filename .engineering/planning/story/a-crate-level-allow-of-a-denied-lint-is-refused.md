---
format: aep.planning-md/3
id: story:a-crate-level-allow-of-a-denied-lint-is-refused
kind: story
status: draft
title: A crate-level allow of a single denied lint does not make the gate green
summary: '#![allow(clippy::needless_return)] at the crate root turned -D warnings green'
tags:
- from-finding
- rsi
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
- informed_by: approval-record:wave-55-review-correction
revision: 1
---
# Story: a-crate-level-allow-of-a-denied-lint-is-refused

## Outcome

A step that adds a crate-level `allow` of a single lint the gate denies, with that lint failing, does not make
`gate.green` hold.

## Context

Found by the wave 55 adversary pass at d0e8fba54 (`crates/takt/src/validator.rs:4381`):
story:the-gate-holds-against-lint-allows-in-source refuses lint groups only, and
`#![allow(clippy::needless_return)]` at the crate root turns `-D warnings` green. An item-level allow of one lint must
stay admitted. The case, not committed with the wave (outside its acceptance):

```rust
#[test]
fn a_crate_level_allow_of_the_failing_lint_keeps_the_gate_from_recording() {
    let tree = repository("crate-one-lint", &project("/bin/sh", &["scripts/lint"]),
        &[("scripts/lint", LINT_ONE), ("src/lib.rs", FAILING)]);
    let root = format!("#![allow(clippy::needless_return)]\n{FAILING}");
    let outcome = observed(&tree, "crate-one-lint", &[("src/lib.rs", &root)]);
    refused(&outcome, &["src/lib.rs"]);
}
```

The helpers and constants are those of `crates/takt/tests/w55_adversary.rs`.

## Acceptance

- The case above is green.
- `#[allow(clippy::needless_return)]` on one item still passes the gate.
