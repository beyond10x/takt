---
format: aep.planning-md/3
id: story:property-and-fuzz-tests
kind: story
status: active
title: Parsers of untrusted input have proptest properties and cargo-fuzz targets
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
- informed_by: review-result:main-fe93286-areas
scope:
- confidence: inferred
  path: Cargo.lock
- confidence: inferred
  path: Taskfile.yml
- confidence: inferred
  path: crates/takt/Cargo.toml
- confidence: inferred
  path: crates/takt/tests/properties.rs
- confidence: inferred
  path: fuzz
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-19T01:31:10Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-19T01:31:10Z", actor: "human:timo", revision: 3}
---
## Outcome

takt's parsers of untrusted input carry property tests (`proptest`, run by `cargo test` in `task check`) and coverage-guided fuzz targets (`cargo-fuzz`, run by `task fuzz` on nightly with a time budget). A property or a fuzz target that finds a failure keeps its minimal input as a committed regression case (`proptest-regressions/`, `fuzz/corpus/` and `fuzz/artifacts/` promoted to a unit test).

## Context

Owner request of 2026-10-19 ("add proptest and cargo-fuzz"), after review-result:main-fe93286-areas: finding E8.1 (a torn multi-byte character makes `Events::read` fail the whole record, `crates/takt/src/events.rs:258@fe93286`) is the kind of defect a fuzz target over arbitrary bytes finds without a reviewer. The machine has `cargo-fuzz` 0.13.2 and a nightly toolchain; the workspace has neither `proptest` nor a `fuzz/` directory today.

First targets, each an input takt reads from something it does not control:
- `Events::read` (`crates/takt/src/events.rs`): any bytes after a valid record; any prefix of a valid record.
- the harness reply parser (`crates/takt/src/harness.rs`, `parse`): arbitrary JSON and text; the last action line wins.
- case ids (`crates/takt/src/case.rs`, `check_id`): no accepted id leaves `.engineering/cases/`.
- the scorecard rule (`crates/takt/src/scorecard.rs`): the median lies within min and max; adding a worse case never makes a candidate's verdict better.
- the git object header parser of story:loop-scorecard (`ObjectReader`), once that story is on main.

A property that fails today on a confirmed finding (E8.1) is committed `#[ignore]` naming its story (story:run-record-survives-a-torn-character), and that story removes the `#[ignore]`.

## Scope

- `crates/takt/Cargo.toml`, `Cargo.lock`: `proptest` as a dev-dependency (inferred)
- `crates/takt/tests/properties.rs` (new) (inferred)
- `fuzz/` (new): `Cargo.toml`, `fuzz_targets/*.rs`, `corpus/` (inferred)
- `Taskfile.yml`: `task fuzz` (inferred)

## Acceptance

`task check` runs every property with a fixed seed and passes; `task fuzz` runs each target for 60 s and exits 0; with `Events::read` mutated to call `std::str::from_utf8(...).unwrap()` on the whole file, the events fuzz target reports a crash within its 60 s.
