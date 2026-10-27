---
format: aep.planning-md/3
id: story:step-commits-follow-a-configurable-style
kind: story
status: implemented
title: Step commits follow a configurable style, semantic by default, with the story in the trailer
summary: 'commit section: semantic, repository or action; takt writes the trailer'
tags:
- priority:high
relations:
- serves: vision:safe-agentic-coding
- decomposes: epic:downstream-story-delivery
- depends_on: story:every-harness-reads-the-repository-instructions-from-the-base
scope:
- confidence: inferred
  path: crates/takt/src/git.rs
- confidence: inferred
  path: crates/takt/src/harness.rs
- confidence: inferred
  path: crates/takt/src/run.rs
- confidence: inferred
  path: crates/takt/src/run_config.rs
- confidence: inferred
  path: crates/takt/tests/commit_style.rs
- confidence: inferred
  path: ess/domains/takt.yaml
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-10-27T12:00:44Z", actor: "human:timo", revision: 5, decided_on: {"recorded":{"review_outcome":3}}}
- {from: "proposed", to: "active", at: "2026-10-27T12:00:45Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"review_outcome":3}}}
- {from: "active", to: "implemented", at: "2026-10-27T15:59:14Z", actor: "human:timo", revision: 7, decided_on: {"recorded":{"test_result":1,"review_outcome":3}}}
---
# Story: step-commits-follow-a-configurable-style

## Outcome

A step commit's message follows a configured style. A run-file section `commit` sets it, with flags under the same
names:
- `style`: `semantic` (default), `repository` or `action`.
- `trailer`: default `Refs`; empty means none.
- `types`: a map from action to commit type. The default is `tests.write: test`, and `implementation.edit: fix` when
  the story carries the tag `bug`, else `feat`.

The styles:
- **`semantic`.** The write step's JSON reply gains `commit: {subject, body}`. takt asks for `<type>: <imperative
  subject>` of at most 72 characters, then a body of 3-4 lines saying the intent.
- **`repository`.** The harness writes the message as the repository's instructions require. Those are the base
  `AGENTS.md`/`CLAUDE.md` of `story:every-harness-reads-the-repository-instructions-from-the-base`.
  - The repository wins whenever it defines a style, as the operator decided. When a base instruction file has a line
    matching `(?i)commit (message|style|convention)`, the effective style is `repository`, whatever `style` says.
  - `style` therefore chooses only between `semantic` and `action` for a repository whose instructions say nothing.
  - The match is a deterministic test on the base bytes, and it is recorded on `opened` with the file and line.
- **`action`.** Today's form.

In every style:
- takt checks the shape of the reply. A failed check, or a reply with no `commit`, falls back to `<type>: <story
  title>` with the one-lined summary as the body, and the fallback is recorded as an event.
- takt writes the trailer itself, after a blank line, from the case's `story.yaml` id. It never takes the trailer from
  model text. A case opened on an intent gets none.

## Context

- A downstream trial (2026-10-27): every step commit read `<action>: <model summary>` with no story reference.
- The message is built at `crates/takt/src/run.rs:11959` as `format!("{}: {}", report.action, one_line(&report.summary))`
  (110251a) and committed by `git.rs:1189-1201` `commit -m`.
- Operator decision, 2026-10-27: the commit style is configurable; the repository's base instructions win when they
  define one; the default is semantic commits with a 3-4 line body and the story trailer.

## Acceptance

Each is a named conformance scenario run by `task check`.

1. `commit_style::semantic_by_default`: subjects `test: ...` and `feat: ...`, bodies of 3-4 lines, and `git
   interpret-trailers --parse` gives exactly `Refs: story:<id>` on every step and candidate commit.
2. `commit_style::the_repository_instructions_win`: a base `AGENTS.md` with a commit rule makes the effective style
   `repository`, and `opened` records it with the matching line number.
3. `commit_style::a_reply_cannot_forge_a_trailer_or_a_takt_line`: a reply whose body holds `Refs: story:other` or a
   line starting `takt:` falls back, and the commit has only takt's trailer.
4. `commit_style::a_bad_reply_falls_back_and_says_so`: a subject over the bound gives the fallback message and one
   `commit.fallback` event.
5. `commit_style::an_intent_case_has_no_trailer`.
6. `commit_style::action_keeps_todays_subject`: `style: action` gives `<action>: <summary>` plus the trailer.
7. `commit_style::the_repository_wins_over_an_explicit_style`: `style: semantic` with a base `AGENTS.md` commit rule
   gives effective style `repository` on `opened`.
8. `commit_style::a_configured_trailer_and_types_are_used`: `trailer: Story` with `types: {implementation.edit:
   build}` gives `Story: story:<id>` and a fallback subject starting `build: `. `trailer: ""` gives no trailer.
9. `commit_style::a_bug_story_is_a_fix`: a story tagged `bug` gives the fallback subject `fix: <story title>` for its
   implementation commit.

## Scope

- `crates/takt/src/run.rs` (the step-commit message, near `:11959`)
- `crates/takt/src/git.rs` (`commit_paths` takes a message file)
- `crates/takt/src/harness.rs` (the reply's `commit` field, prompt text)
- `crates/takt/src/run_config.rs` (`commit` section)
- `ess/domains/takt.yaml` (the commit-style setting, before code)
- `crates/takt/tests/commit_style.rs`
