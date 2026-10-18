---
format: aep.planning-md/3
id: story:declared-loops
kind: story
status: draft
title: Inner, validation and outer loops as declared blocks, and --until
summary: Loops end on a claim, an outcome or a budget; the outer loop takes the next ready story
relations:
- decomposes: epic:composable-agentic-work
- depends_on: story:workflow-schema-and-loader
- depends_on: story:frontier-next-ready-story
- depends_on: story:merge-under-authority
- serves: vision:safe-agentic-coding
- depends_on: story:story-done-end-to-end
scope:
- confidence: cited
  path: crates/takt-cli/src/main.rs
- confidence: cited
  path: crates/takt/src/run.rs
- confidence: inferred
  path: crates/takt/src/workflow.rs
- confidence: inferred
  path: crates/takt/tests/loops.rs
revision: 5
---
# Story: declared-loops

## Outcome

An operator runs a workflow whose loops say when they end, a claim, an outcome or a budget, and whose outer loop takes the next ready story until none is ready or the budget is spent, so one invocation finishes several units; `--until <outcome>` bounds a run the same way.

## Context

`docs/design/takt.md` § 3 loop row: inner until `tests.pass`; validation until the gate is green; outer: the next story until none is ready or the budget is spent; § 3 Shape: `--until <outcome>` beside `--iterations`, `--hours`, `--tokens`; § 4 M3 proof: "the same story from a YAML file, and a three-iteration outer loop". Today there is one fixed loop (`crates/takt/src/run.rs:138-262`) that stops on a legitimate outcome, a spent budget, an approval or nothing admissible; `--iterations`, `--hours` and `--tokens` exist (`run.rs:118-135`, `crates/takt-cli/src/main.rs:39-47`); `--until` does not; a run opens exactly one case (`run.rs:105`).

This story executes the `loop` blocks story:workflow-schema-and-loader loads: `until` is a claim (ends when true), an outcome (ends when legitimate) or a budget (iterations, hours, tokens), checked against the decision after each pass; loops nest; the outer loop asks the `Frontier` (story:frontier-next-ready-story) for the next unit and opens a new case per unit, so a run holds many cases; one outer iteration is one unit. The run's own budgets bound every loop; `--until <outcome>` is a budget on the top-level loop. The stops of today stay: an approval needed or nothing admissible ends the unit's case, not the run, and the outer loop moves on. The M3 proof's first half, "the same story from a YAML file", is this story's: the story that story:story-done-end-to-end finished, run from a `workflow/1` file whose loops are declared, ends with the decision documents the flag run produced; story:workflow-schema-and-loader proves the loader on the intent case of the scratch crate.

## Domain relations

- Run → Case: one-to-one today, inferred from `crates/takt/src/run.rs:75` and `run.rs:105`; this story makes it one-to-many, one case per unit.
- Case → Decision: one-to-many, owned by the case. Inferred from `crates/takt/src/case.rs:175-198`.
- Story → Case: a story taken by the frontier is `active` while its case is open, inferred from aep `artifacts/lifecycles/story.yaml`; a unit whose case stopped at an approval is not reopened by the outer loop, and a later run resumes that case under its grant (story:merge-under-authority, from the cleared decision-blocker:approval-grant-and-case-resume).

## Scope

- `crates/takt/src/run.rs` (loop execution, many cases per run, `until`)
- `crates/takt/src/workflow.rs` (`until` as a typed value)
- `crates/takt-cli/src/main.rs` (`--until <outcome>`)
- new: `crates/takt/tests/loops.rs` with a frontier double

## Ordering

- depends_on story:workflow-schema-and-loader: both edit `crates/takt/src/run.rs`, `crates/takt/src/workflow.rs` and `crates/takt-cli/src/main.rs`.
- depends_on story:frontier-next-ready-story: both edit `crates/takt/src/run.rs` and `crates/takt-cli/src/main.rs`.
- depends_on story:merge-under-authority: both edit `crates/takt/src/run.rs` and `crates/takt-cli/src/main.rs`.
- depends_on story:story-done-end-to-end: both edit `crates/takt-cli/src/main.rs` and `crates/takt/src/run.rs`; the story it finishes is the subject of this story's file-run check.

## Acceptance

With a frontier double holding four ready stories and `--harness none`, a workflow whose outer loop declares a budget of three iterations opens exactly three cases, one per story in frontier order, writes at least one decision document in each, and leaves the fourth story untouched.

## Checks

- The same workflow with `--until accepted` on a case that reaches `accepted` stops before its budget is spent.
- The story that story:story-done-end-to-end finished, run from a `workflow/1` file with declared loops, ends with decision documents equal to the flag run's in every claim value, action status and outcome status (the M3 proof's "the same story from a YAML file", `docs/design/takt.md` § 4).

## Out of Scope

Granting approvals or resuming a case (story:merge-under-authority); running units in parallel; a loop that spans repositories (`docs/design/takt.md` § 5).

## Ambiguities

- What one outer iteration counts: `inferable` as recorded here, one unit's case; an inner loop ending on its own budget ends that unit's case and counts one.
- The `approver` field of an `approval` block: parsed and recorded, not enforced; the cleared decision-blocker:approval-grant-and-case-resume names who grants (`takt approve`) and does not check the approver's identity, which stays open.
