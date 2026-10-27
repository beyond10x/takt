---
format: aep.planning-md/3
id: story:the-run-shows-each-repository-obligation
kind: story
status: draft
title: The prompt, the stop line and the report name each gate obligation and its status
summary: change-aware gate context and a per-obligation table
tags:
- priority:high
relations:
- serves: vision:safe-agentic-coding
- decomposes: epic:downstream-story-delivery
scope:
- confidence: inferred
  path: crates/takt/src/harness.rs
- confidence: inferred
  path: crates/takt/src/report.rs
- confidence: inferred
  path: crates/takt/src/run.rs
- confidence: inferred
  path: crates/takt/src/validator.rs
- confidence: inferred
  path: crates/takt/tests/obligations.rs
revision: 4
---
# Story: the-run-shows-each-repository-obligation

## Outcome

takt gives the gate runner what a change-aware gate needs: `base` = the case's `base_commit`, `subject` = the
revision, and the story's id and tags. The prompt lists "Repository obligations" from the base gates document: each
gate's text with its last status at this revision, one of `open`, `pass`, `fail`, `not applicable` or `not yet run`.
The stop line and `takt report` print the same table, so "handed over, 1 obligation open: release-notes fail" is
visible. The gates document stays read from the base, so a change that edits it cannot weaken its own gate.

## Context

- A downstream trial (2026-10-27): every candidate handed over without the repository's release-note and documentation
  changes, while the configured gate passed.
- The gate is read from the base already (`crates/takt/src/validator.rs` `GateRun::observe_by`, `file_at`, 110251a).
- The prompt's "Open obligations" are the protocol's four only (`harness.rs` `prompt_with`).
- The gate side, `git.changed` and `applies_when`, is engineering-protocols' `story:a-gate-sees-the-change-and-may-not-apply`.

## Acceptance

Each is a named conformance scenario run by `task check`.

1. `obligations::a_consumer_visible_story_without_a_note_is_not_ready`: a fixture repository with a `release-notes`
   gate and a fake harness that never touches `CHANGELOG.md`. Result: `gate.green` false, no `pr.open`, and the stop
   line names `release-notes`.
2. `obligations::not_applicable_is_not_unchecked`: a gate whose `applies_when` is false shows `not applicable`, and a
   gate not yet run shows `not yet run`.
3. `obligations::a_change_to_the_gates_document_does_not_weaken_its_gate`: a candidate that deletes the gate from the
   gates document is still held to it.
4. `obligations::the_prompt_names_the_repository_obligations`: the first step's prompt lists each gate's text.
5. `obligations::the_report_prints_the_table`: after scenario 1's run, `takt report --case <id>` prints
   `release-notes` with status `fail`, and its JSON form holds the same row.

## Scope

- `crates/takt/src/validator.rs` (`GateRun` context: base, subject, story)
- `crates/takt/src/harness.rs` (`prompt_with`)
- `crates/takt/src/run.rs` (stop line)
- `crates/takt/src/report.rs` (the report table)
- `crates/takt/tests/obligations.rs`

## Delivery

This depends on engineering-protocols `story:a-gate-sees-the-change-and-may-not-apply`, recorded as
`dependency-blocker:engineering-gates-see-the-change`.
- It is delivered in one workspace: takt's `Cargo.toml` gets an uncommitted `[patch]` path override to that working
  tree, and scenario 1 is the end-to-end check.
- Then engineering-protocols releases and takt takes it as a pin-only hop. No intermediate release.
