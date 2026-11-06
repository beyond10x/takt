---
format: aep.planning-md/3
id: story:an-include-the-gate-cannot-resolve-fails-closed
kind: story
status: draft
title: An include! the gate cannot resolve fails closed from an unchanged file too
summary: include!(concat!(env!(...))) in an unchanged .rs file pulls in a file the gate does not scan
tags:
- from-finding
- rsi
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
- informed_by: approval-record:wave-55-review-correction
revision: 1
---
# Story: an-include-the-gate-cannot-resolve-fails-closed

## Outcome

An `include!` whose path the gate cannot resolve as a literal (`include!(concat!(env!("OUT_DIR"), "/x.rs"))`) fails
closed when the file it pulls in could hold a lint group allow a step wrote, also when the `.rs` file holding the
`include!` is unchanged.

## Context

Left by the wave 55 review correction (dbd7bba5f): files pulled in through `include!` and `#[path]` are scanned from
changed `.rs` files, and an unresolvable one fails closed there; an unchanged `.rs` file whose `include!` names a path
built by `concat!`/`env!` is not read, so a step can write the file it pulls in.

## Acceptance

- A base `src/lib.rs` with `include!(concat!(env!("CARGO_MANIFEST_DIR"), "/gen.in"))`, unchanged, and a step that adds
  `#![allow(clippy::all)]` to `gen.in`: the gate records no `gate.green`: a test.
