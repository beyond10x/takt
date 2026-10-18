---
format: aep.planning-md/3
id: story:takt-in-container
kind: story
status: draft
title: takt and its harness run inside a benchmark task's container without leaking the harness login
summary: a static takt binary and the harness CLI in the container; the login mounted read-only for the run, never recorded
relations:
- decomposes: epic:benchmark-integration
- serves: vision:safe-agentic-coding
- informed_by: story:prebuilt-binaries-in-ci
scope:
- confidence: inferred
  path: Taskfile.yml
- confidence: inferred
  path: crates/takt/src/harness.rs
revision: 4
---
# Story: takt-in-container

## Outcome

takt runs as the agent inside a benchmark task's Docker container: a statically linked `takt` binary, the harness CLI, and the harness's login provided read-only for the run's duration, never written into a case record, a prompt, an event or a trace.

## Context

Both external benchmarks run each task in its own container (design:benchmark-integration). product-requirements:takt R4 and R5: credentials never enter a harness's prompt or a run record. The login is the one credential the harness needs.

## Expected before

On takt at main, holds; after the change, fails:

- No release target builds a static `takt` (`cargo build --target x86_64-unknown-linux-musl` is not in the Taskfile).

## Scope

- `Taskfile.yml` (inferred)
- `crates/takt/src/harness.rs` (inferred)

## Acceptance

In a container from a stock image, `takt run` on a fixture task completes with the login mounted read-only, and a search of the case directory, the events and the harness output for the login's token finds nothing.
