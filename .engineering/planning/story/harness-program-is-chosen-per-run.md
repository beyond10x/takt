---
format: aep.planning-md/3
id: story:harness-program-is-chosen-per-run
kind: story
status: draft
title: A run names the harness binary it uses and records it
summary: '--harness-program, later program under harness:'
tags:
- priority:medium
relations:
- serves: vision:safe-agentic-coding
- decomposes: epic:downstream-story-delivery
- informed_by: story:harness-section-is-metaharness-session-request
scope:
- confidence: inferred
  path: crates/takt-cli/src/main.rs
- confidence: inferred
  path: crates/takt/src/harness.rs
- confidence: inferred
  path: crates/takt/src/run.rs
- confidence: inferred
  path: crates/takt/tests/harness_program.rs
revision: 4
---
# Story: harness-program-is-chosen-per-run

## Outcome

A run names the harness binary it uses with `--harness-program <path>`. The `opened` record gains the program's
absolute path, its sha256 and its `--version` output. Without the flag, the binary is resolved from `PATH` as today,
and it is recorded the same way.

The run-file form is not part of this story. `story:harness-section-is-metaharness-session-request` admits exactly six
keys in the `harness:` mapping and puts "a binary path per harness" out of scope, with one story per further key. A
`program` key is that later story, after harness-section.

## Context

- A downstream trial (2026-10-27) could select the qualified Claude Code binary only by changing `PATH`.
- `SessionHarness::program` exists (`crates/takt/src/harness.rs:192-196`, 110251a), but no flag sets it.
- `story:an-unqualified-harness-stops-before-the-first-session` names this flag in its stop line.

## Acceptance

Each is a named conformance scenario run by `task check`.

1. `harness_program::a_named_binary_is_used_and_recorded`: with a fake at a path not on `PATH`, the fake records its
   invocation, and `opened` holds its absolute path, sha256 and version.
2. `harness_program::a_relative_or_missing_program_is_refused`: refused at open, exit 1, naming the value.
3. `harness_program::path_resolution_is_recorded`: without the flag, `opened` records the binary resolved from `PATH`.

## Scope

- `crates/takt-cli/src/main.rs` (`--harness-program`)
- `crates/takt/src/harness.rs` (`program`)
- `crates/takt/src/run.rs` (`opened` record)
- `crates/takt/tests/harness_program.rs`
