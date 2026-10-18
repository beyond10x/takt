---
format: aep.planning-md/3
id: story:second-harness-adapter
kind: story
status: draft
title: A second harness adapter behind the Harness trait yields the same decisions
summary: The brief and report contract shared; a second program drives the judgement step
relations:
- decomposes: epic:composable-agentic-work
- depends_on: story:workflow-schema-and-loader
- serves: vision:safe-agentic-coding
- depends_on: story:story-done-end-to-end
- depends_on: story:declared-loops
scope:
- confidence: cited
  path: crates/takt-cli/src/main.rs
- confidence: cited
  path: crates/takt/src/harness.rs
- confidence: inferred
  path: crates/takt/tests/harness_seam.rs
revision: 6
---
# Story: second-harness-adapter

## Outcome

An operator switches the judgement actor with `--harness <name>` and gets decisions that depend only on evidence, never on which model took the step, proven by a second adapter behind the same trait.

## Context

`docs/design/takt.md` § 3 Seams: `Harness (the judgement actor)`, one adapter at first; § 4 M4: "a second harness adapter proves the seam ... the M2 story runs under both adapters with the same decision documents". Today `Harness` (`crates/takt/src/harness.rs:42-47`) has one real adapter, `ClaudeCode` (`harness.rs:49-199`: `claude -p`, file tools only, the last JSON line names the action), and `NoHarness` in the binary (`crates/takt-cli/src/main.rs:65-79`). The brief is built inside the adapter (`harness.rs:84-115`) and the reply is parsed there (`harness.rs:117-132`); the refusal of a step outside the offered actions is in the loop and adapter-independent (`crates/takt/src/run.rs:227-245`).

This story: the brief and the reply contract (one JSON line naming the action and a summary) move out of `ClaudeCode` into the seam so every adapter is given the same brief and held to the same report; a second adapter for a program that takes a prompt and edits files in a directory, for example the Codex command line in non-interactive mode; `--harness` accepts it; tokens are summed when the program reports them (`run.rs:212-214`). "The same decision documents" means the same claim values, obligation statuses, action statuses and outcome statuses per evaluation, not the same instants or commit ids. The parity proof on a planning-store story (`docs/design/takt.md` § 4 M4: the M2 story under both adapters with the same decision documents) is this story's: the story that story:story-done-end-to-end finished is run again under the second adapter from the same base commit, so this story lands after that one; the acceptance proves the seam on the scratch crate first.

## Domain relations

- Harness step → Action: exactly one of the offered actions, the admissible ones no validator owns. Inferred from `crates/takt/src/run.rs:188-193` and `crates/takt/src/harness.rs:42-47`.

## Scope

- `crates/takt/src/harness.rs` (the shared brief and report; `ClaudeCode` on top of them; the second adapter's module declared here)
- new: `crates/takt/src/harness/<second>.rs` (the second adapter)
- `crates/takt-cli/src/main.rs` (`--harness` names it)
- new: `crates/takt/tests/harness_seam.rs` (a scripted adapter proves the contract without a model)

`crates/takt/src/run.rs` is not touched: the call site (`run.rs:211`) is already the seam.

## Ordering

- depends_on story:workflow-schema-and-loader: both edit `crates/takt/src/harness.rs` and `crates/takt-cli/src/main.rs`.
- depends_on story:declared-loops: both edit `crates/takt-cli/src/main.rs`.
- depends_on story:story-done-end-to-end: both touch `crates/takt-cli/src/main.rs`; `crates/takt/src/run.rs` is edited there and is the call site this story's adapter is driven from (`run.rs:211`); the story it finishes is the subject of this story's parity check.

## Acceptance

On the scratch crate with one failing test, `takt run --iterations 3` under `--harness claude` and under the second adapter both end with decision documents whose claim values, action statuses and outcome statuses are equal (`tests.pass` true, `repository.merge` approval-required).

## Checks

- A scripted adapter that answers an action it was not offered is refused and reverted by the loop exactly as the first adapter is.
- The story that story:story-done-end-to-end finished, run again under the second adapter from the same base commit, ends with decision documents equal in every claim, obligation, action and outcome status (the M4 parity proof, `docs/design/takt.md` § 4).

## Out of Scope

A hosted-model adapter or a runtime of its own (`docs/design/takt.md` § 5); per-adapter tool permissions beyond file tools; prompt tuning.

## Ambiguities

- Which program the second adapter drives: `inferable` as a choice for the implementer, any program that edits files from a prompt and can be denied commands; the story names one example and constrains none.
