---
format: aep.planning-md/3
id: story:harness-flags-spell-metaharness-names
kind: story
status: draft
title: takt's harness flags spell metaharness run's names
summary: --effort, --max-turns, --max-budget-usd, --strict-version, --context, each filling the harness mapping
tags:
- config-proxy
relations:
- decomposes: epic:run-config-proxies-llm-and-metaharness
- serves: vision:safe-agentic-coding
- depends_on: story:harness-section-is-metaharness-session-request
scope:
- confidence: cited
  path: crates/takt-cli/src/main.rs
- confidence: inferred
  path: crates/takt-cli/tests
- confidence: cited
  path: ess/ess-inputs.yaml
- confidence: cited
  path: website/docs/reference/configuration.md
revision: 4
---
# Story: harness-flags-spell-metaharness-names

## Outcome

Every key the run file's `harness` mapping admits has a `takt run` and `takt scenario run` flag of the same name as
`metaharness run`'s, and a flag wins over the same key in the file.

## Context

- The owner, 2026-10-27: "in the same sort of way how I would do that if I'm running metaharness"
  (epic:run-config-proxies-llm-and-metaharness).
- story:harness-section-is-metaharness-session-request admits exactly `kind`, `effort`, `max-turns`,
  `max-budget-usd`, `strict-version` and `context`; `kind` is already `--harness`.
- metaharness's flags for them: `--effort` (metaharness 0.19.0 `crates/metaharness-protocol/src/spec.rs:364`),
  `--max-turns` (`:368`), `--max-budget-usd` (`:378`), `--strict-version` (`:462`), `--context` (`:589`).
- A flag wins over the run file today (`crates/takt-cli/src/main.rs:630-672`).

## Decision

- Five new flags on `LoopArgs`: `--effort`, `--max-turns`, `--max-budget-usd`, `--strict-version`, `--context`
  (repeatable), each passed into the harness mapping under its key and checked there by metaharness.
- Order: after story:harness-section-is-metaharness-session-request, whose mapping these flags fill.

## Acceptance

Each item is a conformance scenario named in `ess/ess-inputs.yaml`, run by `task check` with the stub harness binary
of story:harness-section-is-metaharness-session-request.

1. `harness-flags/each-flag-observed`: for each flag, a run setting it differs from a run without it in the named
   observable, and a run file setting the key alone gives the same observable (on both commands):
   `--effort high` with Codex, the generated Codex configuration's `model_reasoning_effort = "high"`;
   `--max-turns 60` with Claude Code, `--max-turns 60` in the stub's argv; `--max-budget-usd 2` with Claude Code,
   `--max-budget-usd 2` in the stub's argv; `--strict-version` with a stub reporting a version outside metaharness's
   pin, the run stopping with metaharness's version refusal where without the flag the step runs;
   `--context notes.md`, the file's text in the prompt the stub received.
2. `harness-flags/flag-over-key`: `--max-turns 50` over `harness: {kind: claude, max-turns: 60}` starts the stub with
   `--max-turns 50`.
3. `harness-flags/kind-check-applies`: `--harness codex --max-turns 60` exits 1 before any case opens with
   metaharness's refusal.
4. `harness-flags/help-lists-names`: `takt run --help` and the generated CLI reference list all five flags.

## Out of Scope

- Flags for keys the mapping does not admit.

## Scope

- `crates/takt-cli/src/main.rs` (`LoopArgs`), `crates/takt-cli/tests/`, `website/docs/reference/configuration.md`,
  `ess/ess-inputs.yaml`.
