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
- supersedes: story:codex-step-context
- informed_by: story:harness-section-is-metaharness-session-request
- informed_by: story:harness-flags-spell-metaharness-names
scope:
- confidence: inferred
  path: crates/takt/src/harness.rs
- confidence: inferred
  path: crates/takt/src/run.rs
- confidence: inferred
  path: crates/takt/src/validator.rs
- confidence: inferred
  path: crates/takt/tests/step_context.rs
revision: 4
---
# Story: every-harness-reads-the-repository-instructions-from-the-base

## Outcome

Every harness kind (Claude, Codex, b10x-harness) is given the repository's instruction files before the prompt, as
the case's base holds them.
- **Defaults.** With nothing configured, the files are `AGENTS.md` and `CLAUDE.md`, each only when the base tracks it.
- **Configuration.** There is no new key or flag. The configured list is metaharness's own `context`: `harness:
  {context}` in the run file (`story:harness-section-is-metaharness-session-request`) and `--context`
  (`story:harness-flags-spell-metaharness-names`). This story adds the rule for every path in that list, default or
  configured.
- **The rule.** Each file is read at the case's `base_commit` (`crates/takt/src/validator.rs` `file_at`), never from
  the step's copy. It is handed to metaharness as that `context`. The step's opening record names the path, sha256 and
  revision of each.
- **Bounds.** The files are bounded with the scope files (`SCOPE_FILES`/`SCOPE_BYTES`). A file over the bound is named
  in the stop line and the report, never silently dropped.

Ambient discovery stays off: H11 is unchanged, and the files are declared, read from the base and recorded.

## Context

- A downstream trial (2026-10-27): three of three candidates missed the repository's documented release-note and
  documentation rules.
- A Claude step's prompt attaches only the intent's `## Scope` files (`crates/takt/src/harness.rs:1308-1333`
  `prompt_with`, 110251a). metaharness Strict switches project memory off on purpose (`--setting-sources ""`). So
  Claude never sees `AGENTS.md`.
- Codex gets the files through `context_files` (`harness.rs:530-551`, `:870-900`), but read from the step's tree, not
  the base. This story supersedes the draft `story:codex-step-context`, whose Codex half is already in code.

## Acceptance

Each is a named conformance scenario run by `task check`.

1. `step_context::claude_reads_agents_md_from_the_base`: a fake Claude receives `AGENTS.md` at the base's bytes.
2. `step_context::codex_reads_agents_md_from_the_base`: the same for a fake Codex.
3. `step_context::b10x_harness_reads_agents_md_from_the_base`: the same for a fake b10x-harness.
4. `step_context::a_step_edit_is_not_what_the_next_step_reads`: after a step edits `AGENTS.md`, the next step still
   gets the base bytes.
5. `step_context::an_untracked_default_is_skipped`: a base that tracks only `CLAUDE.md` gives that file alone, and no
   error.
6. `step_context::the_opening_record_names_each_file`: the record has path, sha256 and revision per file.
7. `step_context::an_oversized_file_is_named`: a file over `SCOPE_BYTES` appears in the stop line.

A configured list (`--context notes.md`) is observed by harness-flags' own acceptance. Once that story lands, its file
is read at the base by the same code, and scenario 4 is repeated with it.

## Scope

- `crates/takt/src/harness.rs` (context handed to every kind; `prompt_with`)
- `crates/takt/src/validator.rs` (`file_at`, read at base)
- `crates/takt/src/run.rs` (opening record)
- `crates/takt/tests/step_context.rs`
