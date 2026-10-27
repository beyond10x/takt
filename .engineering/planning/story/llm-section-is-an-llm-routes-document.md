---
format: aep.planning-md/3
id: story:llm-section-is-an-llm-routes-document
kind: story
status: draft
title: The run file's llm section is an llm.routes/1 document, validated by llm
summary: inline or by path, parsed by llm 0.16.0's Routes::parse or parse_toml; takt types none of its keys
tags:
- config-proxy
relations:
- decomposes: epic:run-config-proxies-llm-and-metaharness
- serves: vision:safe-agentic-coding
scope:
- confidence: cited
  path: Cargo.lock
- confidence: cited
  path: Cargo.toml
- confidence: cited
  path: Taskfile.yml
- confidence: cited
  path: crates/takt-cli/tests/conformance.rs
- confidence: cited
  path: crates/takt-cli/tests/run_config_cli.rs
- confidence: cited
  path: crates/takt/Cargo.toml
- confidence: cited
  path: crates/takt/src/run_config.rs
- confidence: cited
  path: ess/domains/takt.yaml
- confidence: cited
  path: ess/ess-inputs.yaml
- confidence: cited
  path: website/docs/reference/configuration.md
revision: 5
---
# Story: llm-section-is-an-llm-routes-document

## Outcome

`takt run --config <file>` reads an `llm` member that is an `llm.routes/1` document, inline or by path, and llm's own
parser validates it: takt types none of its keys, a run whose `llm` section llm refuses opens no case, and the run's
opened record names the digest of the document it ran under.

## Context

- The owner, 2026-10-27: "I would be able to configure model routes in my config" (epic:run-config-proxies-llm-and-metaharness).
- takt's run file is `RunConfig`, eight flat keys, unknown keys refused (`crates/takt/src/run_config.rs:73-92`); it has
  no place for routes. takt links no llm crate of its own; Loom 0.43.0 links llm 0.8.1 (`b10x-llm-*` packages).
- llm 0.16.0 parses `llm.routes/1` from text, YAML by `Routes::parse` and TOML by `Routes::parse_toml`, refusing an
  unknown key and naming the field, never a value (`crates/llm-models/src/routes.rs:1484`, `:1510`, `:63-72`), and
  gives a digest of the validated content (`Routes::digest`, `:1605`). `Routes` has no `Deserialize` (`:1359-1368`),
  so takt hands llm the section's text.

## Decision

- `llm` is a new member of the run file, beside today's keys, in the plain form and in the `takt` member of a
  `loom.agent/1` document alike. No new `format` value: takt's own keys stay where they are.
- `llm` is either a mapping holding a whole `llm.routes/1` document, re-serialised and passed to `Routes::parse`, or
  `{path: <file>}`, read beside the run file and parsed by extension (`.yaml`/`.yml` by `parse`, `.toml` by
  `parse_toml`). takt adds a direct dependency on llm 0.16.0's `llm-models`, beside Loom's 0.8.1.
- Spec first: `takt.work.RunConfiguration` (`ess/domains/takt.yaml:539-582`) gains `llm`, an opaque reference to llm's
  `llm.routes/1` document, and `takt.work.RunOpened` (`:1064`) gains `llm_digest: Optional<String>`, before code builds
  on either.
- This story wires the conformance runner the epic's scenarios need, since `task check` runs none today
  (`Taskfile.yml:11`, `ess/ess-inputs.yaml` `scenarios: []`): the scenarios are authored in `ess/ess-inputs.yaml` and
  compiled by `ess conform author`; a Rust test target, `crates/takt-cli/tests/conformance.rs`, runs each against the
  built binary; `ess conform report` turns its results into a report; `task check` runs all three and fails on a
  failed or skipped scenario.
- Order: first of the epic; every other story reads the section this one parses and adds scenarios to its runner.

## Acceptance

Each item is a conformance scenario named in `ess/ess-inputs.yaml` (empty today), run by `task check` through the
runner above; item 6 is the runner itself.

1. `run-config/llm-inline-accepted`: a run file whose `llm` member is a valid `llm.routes/1` document opens the run,
   and the case's `opened` record carries `llm_digest` equal to `Routes::digest` of that document.
2. `run-config/llm-path-accepted`: `llm: {path: llm.toml}` and `llm: {path: llm.yaml}` holding the same routes give the
   same `llm_digest` as scenario 1's inline form.
3. `run-config/llm-unknown-key-refused`: an `llm` section with an unknown key exits 1 before any case opens, the
   message is llm's (names the field) and repeats no value from the file.
4. `run-config/llm-literal-secret-refused`: an endpoint whose `auth.secret` is a literal rather than an `env:`, `file:`
   or `keychain:` reference exits 1 before any case opens, and the literal appears in no output and no file takt wrote.
5. `run-config/no-llm-section-no-digest`: a run file without `llm` opens a run whose `opened` record has no
   `llm_digest`, and the existing tests of `crates/takt-cli/tests/run_config_cli.rs` pass unchanged.
6. `conformance/runner-fails-on-red`: `task check` exits non-zero when one of scenarios 1 to 5 is made to fail, and
   the report `ess conform report` wrote names that scenario as failed.

## Out of Scope

- Using a route in a run: story:model-names-an-llm-route.
- Reading `LLM_CONFIG` or llm's default paths: takt reads an llm document only where the run names one.

## Scope

- `crates/takt/src/run_config.rs`, `crates/takt/Cargo.toml`, `Cargo.toml`, `Cargo.lock`, `Taskfile.yml`,
  `crates/takt-cli/tests/run_config_cli.rs`, `crates/takt-cli/tests/conformance.rs`, `ess/domains/takt.yaml` (`RunConfiguration`, `RunOpened`),
  `ess/ess-inputs.yaml`, `website/docs/reference/configuration.md`.
