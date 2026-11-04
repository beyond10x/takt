---
format: aep.planning-md/3
id: story:harness-flags-spell-metaharness-names
kind: story
status: active
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
- confidence: cited
  path: crates/takt-cli/tests/harness_flags_spell_metaharness.rs
- confidence: cited
  path: crates/takt/src/run_config.rs
- confidence: cited
  path: ess/ess-inputs.yaml
- confidence: cited
  path: website/docs/reference/configuration.md
revision: 11
transitions:
- {from: "draft", to: "proposed", at: "2026-10-27T01:36:42Z", actor: "human:timo", revision: 5, decided_on: {"recorded":{"review_outcome":1}}}
- {from: "proposed", to: "active", at: "2026-10-27T01:36:42Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"review_outcome":1}}}
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

Derived 2026-11-04 by `story-scoper` at 1c3ba5178. What is left once
story:harness-section-is-metaharness-session-request has merged: the observables, the help text and the Codex flag's
message. Each line is **cited** (read from the story or the tree) or **inferred**.

- `crates/takt-cli/tests/harness_flags_spell_metaharness.rs:260-297`: the two refusal cases become acceptance 1's
  per-flag observables on `takt run` and `takt scenario run`; `:224-235`, `:249-258` take metaharness's refusal
  (acceptance 3); `:81-98` (the doubles) need a Codex double that copies its generated configuration and a version
  outside metaharness's pin for `--strict-version` — cited
- `crates/takt-cli/src/main.rs:1165-1184` (`LoopArgs` doc comments, the `--help` text) — cited
- `crates/takt/src/run_config.rs:603` (`CODEX_FLAG_MAX_TURNS_REFUSED`) and `:617-624`; `:571-573` (`--max-budget-usd`
  enters as a string where a file gives a number); `:577-584` (`--context` paths, which Loom takes only as plain
  relative paths of copied files) — cited for the lines, inferred for what changes
- `ess/ess-inputs.yaml:6`, `website/docs/reference/configuration.md:719-723`, `:733` — cited
- Shared with story:harness-section-is-metaharness-session-request: `run_config.rs:499-641`, `main.rs` `harness_of`
  and `LoopArgs`, `harness.rs` `SessionHarness`, `ess/ess-inputs.yaml`, `configuration.md` — cited
- Confidence: medium; how much is left depends on how the section story writes `refused_setting` and `harness_of`.

## Status

Built in wave 51 (unit b, merged into `wave/51` at dd2e14194, review fixes at 54d05a31c); earlier partial unit
d2294dff, merged at cb63ced3. Unit a of the wave carried the flags; unit b added their observables, with no
production change.

- 1 each-flag-observed: `each_flag_observed::{effort,max_turns,max_budget_usd,strict_version,context}_on_takt_{run,scenario_run}`.
  Each runs the flag, the run file's key alone and neither, and the first two give the observable. `--strict-version`
  is observed with Codex off metaharness 0.23.0's pinned versions: the run stops `HarnessUnqualified` with
  metaharness's H9 verdict where without the flag the step runs. Claude Code cannot show the difference, since its
  strict hermetic mode stops an off-pin version with or without the flag.
- 2 flag-over-key: `a_flag_wins_over_the_files_max_turns`; budget and effort, `w51_a_adversary::a_flag_wins_over_the_files_budget_and_effort`.
- 3 kind-check-applies: `kind_check_applies` and `max_turns_under_codex_is_refused_before_any_case_opens`, with
  metaharness's refusal text.
- 4 help-lists-names: `the_help_of_both_run_commands_lists_the_five_flags`; `config_docs_adv_carry`.

The conformance scenarios stay unrun (approval-record:wave-51-harness-mapping-carried, decision 3); each is the cargo
test above. Review: review-result:w51-adv-b-1, fixed.

Scope as built: `crates/takt-cli/tests/harness_flags_spell_metaharness.rs`, `crates/takt-cli/src/main.rs` (the
`LoopArgs` help), `website/docs/reference/configuration.md`. Not changed after all: `crates/takt/src/run_config.rs`
(its `CODEX_FLAG_MAX_TURNS_REFUSED` was removed by unit a), `ess/ess-inputs.yaml`.
