---
format: aep.planning-md/3
id: story:every-harness-reads-the-repository-instructions-from-the-base
kind: story
status: draft
title: Every harness kind is given the configured instruction files as the base holds them
summary: context key, default AGENTS.md and CLAUDE.md, read at base, recorded
tags:
- priority:high
relations:
- serves: vision:safe-agentic-coding
- decomposes: epic:downstream-story-delivery
- informed_by: story:codex-step-context
scope:
- confidence: inferred
  path: crates/takt/src/harness.rs
- confidence: inferred
  path: crates/takt/src/run.rs
- confidence: inferred
  path: crates/takt/src/run_config.rs
- confidence: inferred
  path: crates/takt/src/validator.rs
- confidence: inferred
  path: crates/takt/tests
revision: 2
---
# Story: every-harness-reads-the-repository-instructions-from-the-base

## Outcome

Every harness kind (Claude, Codex, b10x-harness) is given the repository's instruction files before the prompt.
- A run-file key `context`, with a flag, lists them: default `["AGENTS.md", "CLAUDE.md"]`, each only when the base
  tracks it.
- Each file is read at the case's `base_commit`, never from the step's copy.
- The step's opening record names the path, sha256 and revision of each.
- The files are bounded with the scope files (`SCOPE_FILES`/`SCOPE_BYTES`). A file over the bound is named in the stop
  line and the report, never silently dropped.

Ambient discovery stays off: H11 is unchanged, and the files are declared, read from the base and recorded.

## Context

- A downstream trial (2026-10-27): three of three candidates missed the repository's documented release-note and
  documentation rules.
- A Claude step's prompt attaches only the intent's `## Scope` files (`crates/takt/src/harness.rs:1308-1333`
  `prompt_with`, 110251a). metaharness Strict switches project memory off on purpose (`--setting-sources ""`). So
  Claude never sees `AGENTS.md`.
- Codex gets the files through `context_files` (`harness.rs:530-551`, `:870-900`). That is why the draft
  `story:codex-step-context` is stale. This story closes it.

## Acceptance

Each is a named conformance scenario run by `task check`.

1. `step_context::claude_reads_agents_md_from_the_base`: a fake Claude receives `AGENTS.md` at the base's bytes.
2. `step_context::b10x_harness_reads_agents_md_from_the_base`: the same for a fake b10x-harness.
3. `step_context::a_step_edit_is_not_what_the_next_step_reads`: after a step edits `AGENTS.md`, the next step still
   gets the base bytes.
4. `step_context::the_opening_record_names_each_file`: the record has path, sha256 and revision per file.
5. `step_context::an_oversized_file_is_named`: a file over `SCOPE_BYTES` appears in the stop line.

## Scope

- `crates/takt/src/harness.rs` (`with_context_files` for every kind; `prompt_with`)
- `crates/takt/src/validator.rs` (`file_at`, read at base)
- `crates/takt/src/run_config.rs` (`context` key)
- `crates/takt/src/run.rs` (opening record)
- `crates/takt/tests/` (conformance scenarios)
