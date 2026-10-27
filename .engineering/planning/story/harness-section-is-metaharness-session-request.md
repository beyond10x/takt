---
format: aep.planning-md/3
id: story:harness-section-is-metaharness-session-request
kind: story
status: draft
title: The run file's harness section is metaharness's session request, checked by metaharness
summary: Claude, Codex and b10x-harness keys as metaharness run takes them; takt-owned keys refused; waits on metaharness and Loom
tags:
- config-proxy
relations:
- decomposes: epic:run-config-proxies-llm-and-metaharness
- serves: vision:safe-agentic-coding
- depends_on: story:model-names-an-llm-route
- depends_on: story:one-run-file-for-host-and-loom-agent
scope:
- confidence: cited
  path: Cargo.lock
- confidence: cited
  path: Cargo.toml
- confidence: cited
  path: crates/takt-cli/src/main.rs
- confidence: cited
  path: crates/takt/src/harness.rs
- confidence: cited
  path: crates/takt/src/run_config.rs
- confidence: cited
  path: ess/domains/takt.yaml
- confidence: cited
  path: ess/ess-inputs.yaml
- confidence: cited
  path: website/docs/reference/configuration.md
revision: 4
---
# Story: harness-section-is-metaharness-session-request

## Outcome

The run file's `harness` key, written as a mapping, configures Claude Code, Codex and b10x-harness with the keys
`metaharness run` takes, parsed and checked by metaharness's own code; the string form keeps today's meaning, and keys
that make up takt's confinement or its endpoint are refused in the mapping.

## Context

- The owner, 2026-10-27: "I'm able to just configure Claude, Codex, and so on in the same sort of way how I would do
  that if I'm running metaharness" (epic:run-config-proxies-llm-and-metaharness).
- `harness` is today a string, the run file's spelling of `--harness` (`crates/takt/src/run_config.rs:81`).
- metaharness's settings are `RunSpec`, spelt as `metaharness run <kind>` flags (metaharness 0.19.0
  `crates/metaharness-protocol/src/spec.rs:165-600`): among them `effort` (`:364`), `max-turns` (`:368`),
  `max-budget-usd` (`:378`), `strict-version` (`:462`), `context` (`:589`). It has no document form and no
  `deny_unknown_fields`; `check_spec` validates a spec (`crates/metaharness/src/builder.rs:1860-1976`), while a
  per-kind refusal such as Codex's of `max-turns` is raised only at launch (`crates/metaharness-codex/src/launch.rs:919-927`).
- takt sets none of effort, the cost cap or the version check, and hard-codes max turns 40 and a 1800 s step timeout
  (`crates/takt/src/harness.rs:265-267`); Loom's executor hard-codes hermetic mode, decisions and tools per kind (loom
  0.43.0 `crates/loom-session-executor/src/lib.rs:805-833`).

## Decision

- `harness: <string>` keeps today's meaning. `harness: <mapping>` is metaharness's session request; its `kind` is
  metaharness's (`claude`, `codex`, `b10x`) and sets the harness; `--harness` on the command line wins over it.
- The mapping admits exactly `kind`, `effort`, `max-turns`, `max-budget-usd`, `strict-version` and `context`, each
  under metaharness's name and passed to metaharness unchanged. Every other key is refused before any case opens,
  naming the key: the confinement keys (`cwd`, `scratch-cwd`, `frame`, `decisions`, `hermetic`, `driver`, `hooks`,
  `tool-surface`, `catalogue`, `substrate`, `substrate-embedded`, `cgroup-root`, `write-scope`,
  `process-write-subtree`, `allow-program`, `plugin`, `plugin-dir`, `spec`) as takt's, and the endpoint keys
  (`model`, `model-endpoint`, `model-wire`, `credentials`, `subscription-token-*`, `api-key-secret`) as the run's
  `model` and `llm` section's (story:model-names-an-llm-route). An admitted list, not a refused one, so a key a later
  metaharness adds is refused until a story admits it (security first, `product-requirements:takt` § 3).
- Waits on two upstream stories: metaharness publishes its session request as a document whose check includes the
  per-kind refusals (unknown keys refused, a public parse), and Loom's session executor takes a caller's partial
  `RunSpec` over its per-kind defaults. Until both release, this story is not implementable.
- Spec first: `takt.work.RunConfiguration.harness` becomes a union of `HarnessName` and an opaque reference to
  metaharness's session request; `takt.work.HarnessSettingRefused` names a refused key and whose it is.
- Order: after story:one-run-file-for-host-and-loom-agent, because both move the loom pin in `Cargo.toml` and
  `Cargo.lock` (this one to the Loom release with the executor overlay), and both extend `ess/ess-inputs.yaml`.

## Acceptance

Each item is a conformance scenario named in `ess/ess-inputs.yaml`, run by `task check` with a stub harness binary
that writes the argv it was started with to a file.

1. `harness-section/codex-effort-passed`: `harness: {kind: codex, effort: high}` starts the stub, and the Codex
   configuration metaharness generated for it, which the stub copies beside its argv file, holds
   `model_reasoning_effort = "high"` (metaharness 0.19.0 `crates/metaharness-codex/src/launch.rs:1264-1265`).
2. `harness-section/max-turns-replaces-default`: `harness: {kind: claude, max-turns: 60}` starts the stub with
   `--max-turns 60`, and no argv file of that run holds `40`.
3. `harness-section/unadmitted-key-refused`: a key outside the admitted six (`retain-dir`, and a key no metaharness
   has) exits 1 before any case opens with takt's message naming the key as not admitted, and no stub is started.
4. `harness-section/bad-value-refused-by-metaharness`: `harness: {kind: claude, effort: extreme}` exits 1 before any
   case opens with metaharness's own message, and no stub is started.
5. `harness-section/kind-check-refused`: `harness: {kind: codex, max-turns: 60}` exits 1 before any case opens with
   metaharness's refusal, and no stub is started.
6. `harness-section/takt-owned-key-refused`: `harness: {kind: claude, hermetic: off}` exits 1 before any case opens,
   naming `hermetic` as takt's, and no stub is started.
7. `harness-section/string-form-unchanged`: `harness: codex` starts the stub with the same argv as `--harness codex`.

## Out of Scope

- The two upstream stories in metaharness and Loom.
- Admitting further metaharness keys: one story per key, naming why it does not widen confinement.
- A binary path per harness: neither metaharness nor takt has one.

## Scope

- `crates/takt/src/run_config.rs`, `crates/takt/src/harness.rs` (`SessionExecutor` construction, `harness.rs:527-640`),
  `crates/takt-cli/src/main.rs` (`harness_of`), `Cargo.toml`, `Cargo.lock`, `ess/domains/takt.yaml`,
  `ess/ess-inputs.yaml`, `website/docs/reference/configuration.md`.
