---
format: aep.planning-md/3
id: story:evidence-port-is-the-only-writer
kind: story
status: draft
title: Only the case's evidence port can write an evidence record
tags:
- loom
relations:
- decomposes: epic:takt-on-loom-sdk
- depends_on: story:validators-perform-as-effects
revision: 1
---
## Outcome

Only the case's evidence port can write an evidence record: `CaseDir::add_evidence` is not
callable from outside `b10x-takt`'s evidence port, and no test or guard depends on finding a
literal path to know it.

## Context

The wave 29 review (finding 4) found `CaseDir::add_evidence` public
(`crates/takt/src/case.rs:2983`), so any crate linking `b10x-takt` can write a record past the
port. Unit 8 of wave 29 widened the guard
(`crates/takt/tests/effects.rs:694`, `the_port_guard_finds_a_record_written_past_it_in_any_crate`)
to scan `crates/takt-cli/src` too, and left the method public: about 100 test call sites seed
cases through it. The guard finds a raw write only where the evidence path is spelled as a
literal.

## Scope

- `crates/takt/src/case.rs`, `crates/takt/src/evidence.rs`, the tests that seed records.

## Acceptance

1. `add_evidence` is `pub(crate)` or behind a test-only feature; tests seed records through a
   named test helper or the port: the workspace builds and `cargo test` passes.
2. A crate outside `b10x-takt` that calls `add_evidence` does not compile: a compile-fail test.
