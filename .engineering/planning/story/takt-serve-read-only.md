---
format: aep.planning-md/3
id: story:takt-serve-read-only
kind: story
status: draft
title: takt serve answers the declared views on localhost and writes nothing
summary: a read-only local HTTP surface over case files, held to the model by ess verify conform
relations:
- decomposes: epic:terminal-frontend
- serves: vision:safe-agentic-coding
- depends_on: story:tui-read-model
scope:
- confidence: inferred
  path: crates/takt-cli/src/main.rs
- confidence: inferred
  path: crates/takt-serve
revision: 3
---
# Story: takt-serve-read-only

## Outcome

`takt serve --repo <path>` serves the read views of takt's model over HTTP on localhost only, reading the repository's case files, and writes nothing: no case, no store, no git. `ess verify conform` holds it to the model.

## Context

epic:terminal-frontend, step 3. Depends on story:tui-read-model. The generated terminal app reads over HTTP (beyond10x/ess `crates/ui/ess-ui-tui/src/lib.rs:242`), and takt's state is files. Security: bound to 127.0.0.1 (or a Unix socket if ESS accepts one), no write route, no credential read or served, case files read through `takt::case` and `takt::events` only.

## Expected before

On takt at main, holds; after the change, fails:

- `takt serve --help` exits non-zero (no such command).

## Scope

- `crates/takt-cli/src/main.rs`: the `serve` command (inferred)
- a new module or crate for the read-only server (inferred)

## Acceptance

`takt serve --repo <scratch repository with the recorded cases>` answers each declared view, `ess verify conform` against it passes every generated scenario for those views, a request to any path not declared returns 404, and the process writes no file under the repository (checked by a tree digest before and after).
