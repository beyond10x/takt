---
format: aep.planning-md/3
id: story:scorecard-reader
kind: story
status: implemented
title: takt scorecard reads run events per scenario against the promoted baseline
summary: outcome, steps, tokens, refusals, errors, wall time from events.jsonl only
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
- depends_on: story:run-record-and-story-evidence
- depends_on: story:scenario-corpus
scope:
- confidence: cited
  path: crates/takt-cli/src/main.rs
- confidence: cited
  path: crates/takt/src/lib.rs
- confidence: inferred
  path: crates/takt/src/scorecard.rs
- confidence: inferred
  path: crates/takt/tests/scorecard.rs
revision: 10
transitions:
- {from: "draft", to: "proposed", at: "2026-10-18T15:28:21Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"review_outcome":4}}}
- {from: "proposed", to: "active", at: "2026-10-18T15:28:21Z", actor: "human:timo", revision: 7, decided_on: {"recorded":{"review_outcome":4}}}
- {from: "active", to: "implemented", at: "2026-10-18T18:21:53Z", actor: "human:timo", revision: 9, decided_on: {"recorded":{"test_result":1,"review_outcome":6}}}
---
# Story: scorecard-reader

## Outcome

`takt scorecard` prints, per scenario run, outcome, steps, tokens, refusals, `NoProgress` and
errors, and wall time, read only from the runs' events under `.engineering/cases/`; `--save`
writes the scorecard as a file named by its revision; `--against <saved>` compares each scenario
with the same scenario in a saved scorecard, marks a row worse when its outcome is worse, and
prints the token difference.

## Context

design:recursive-self-improvement § Scorecard. Tokens come from the runs' events, never from a
`task_spend` file a candidate could plant. The events exist after story:run-record-and-story-evidence
(`events.jsonl` per case). Which saved scorecard is the promoted baseline, and the median and
daily rules, are story:promotion's. A metric's definition is a judge surface: for one cycle after
a definition changes, the scorecard prints the figure under the previous definition beside the new
one. story:week-of-daily-runs reads the same records for its weekly report and builds on this
reader.

## Domain relations

- Scorecard → Scenario run: one row per run. UNMAPPED: whether a scorecard covers one candidate
  revision or a period.
- Scorecard → Revision: a scorecard covers one takt revision, the one its saved file `scorecard-<revision>.json` is named by (takt.work.Scorecard). Decided 2026-10-18 (settles the marker above).

## Scope

Derived 2026-10-18 by `story-scoper`. Every line is **cited** (read from the story or the tree) or
**inferred** (a reading that could be wrong).

- **Primary surface:** `crates/takt` (a new `scorecard` module) — inferred, from the story's own Scope, which marks it inferred
- **Files:** new `crates/takt/src/scorecard.rs` — inferred, the story's Scope lists it
- **Files:** `crates/takt/src/lib.rs:8-17` (the `pub mod scorecard;` line among the module list) — cited, the story names the file and the tree shows where modules are registered
- **Files:** `crates/takt-cli/src/main.rs:28-37` (`enum Commands`, a `Scorecard` variant) and `:190` (`match cli.command`, its arm) — cited, the story names the file and `scorecard`, and the tree shows the subcommand enum and dispatch
- **Files:** new `crates/takt/tests/scorecard.rs` (two recorded scenario-1 case directories as fixtures, plus a planted `task_spend` file) — inferred, from the Acceptance and Checks
- **Symbols:** `Commands`, `CASES_DIR` (`crates/takt/src/case.rs:21`, `.engineering/cases`), `Stop::NoProgress` (`crates/takt/src/run.rs:61`) — cited
- **Also likely:** read only, not edited: the event types in `crates/takt/src/events.rs`, which story:run-record-and-story-evidence creates — inferred, the file does not exist in this tree
- **Documents:** none — inferred, the acceptance is all CLI output and nothing names a document
- **Confidence:** medium — the three files are named in the story and confirmed in the tree, but the event format the reader parses does not exist yet (no `events.jsonl` match anywhere in `crates/`)
- **Would collide with:** any unit adding or changing a `takt-cli` subcommand in `crates/takt-cli/src/main.rs` (story:scenario-corpus, story:promotion and story:week-of-daily-runs all edit it), and any unit registering a module in `crates/takt/src/lib.rs` — cited, from those stories' Scope sections
- **Safety fact:** tokens from the events can be trusted over a `task_spend` file because the run loop snapshots `.engineering/cases` before every harness step and restores whatever the step touched (`crates/takt/src/run.rs:566-578`), so a candidate cannot plant or edit a case's events — step 2, unproven; it does not cover writes made outside `Run::step`, such as an `events.jsonl` emitted from another code path, or a case directory supplied by hand

Not established: the event format (`events.jsonl`/`events.rs` do not exist yet); `task_spend` matches nothing in `crates/`; no source names where `--save` writes; no revision source for the saved file name; no home for metric definitions; refusals are printed, not counted (`run.rs:384`, `:608`).

Typed entries: `crates/takt/src/scorecard.rs` (inferred), `crates/takt/src/lib.rs` (cited), `crates/takt-cli/src/main.rs` (cited), `crates/takt/tests/scorecard.rs` (inferred).

## Ordering

- depends_on story:run-record-and-story-evidence: the events are its output.
- depends_on story:scenario-corpus: the rows are scenario runs; both edit `crates/takt-cli/src/main.rs`, so this story follows it on that file.

## Acceptance

Over the case directories of two scenario 1 runs, `takt scorecard` prints each run's tokens equal
to the sum of the harness-reported tokens in that run's events.

## Checks

- A planted `task_spend` evidence file in the candidate tree changes no number the scorecard prints.
- `--against` a saved scorecard in which scenario 1 was `accepted` marks a `NoProgress` row worse.
- A changed metric definition prints both figures for one cycle.

## Out of Scope

Choosing the baseline and promoting (story:promotion); the weekly report and defects escaped
(story:week-of-daily-runs); the loop scorecard (story:loop-scorecard).

## Landed (wave 10)

Read from `git diff --stat 8b0afc5^1 8b0afc5` (the unit's merge into the wave branch) at the close of wave 10 (wave branch head 8858b49). Comparison by dominance per outcome rank; a case accepted in any run stays accepted. Saved-file provenance, case identity and the flakiness and token questions are carried by story:promotion.

- `crates/takt-cli/src/main.rs`
- `crates/takt-cli/tests/scorecard_adv1.rs`
- `crates/takt-cli/tests/scorecard_cli.rs`
- `crates/takt/src/events.rs`
- `crates/takt/src/harness.rs`
- `crates/takt/src/lib.rs`
- `crates/takt/src/run.rs`
- `crates/takt/src/scorecard.rs`
- `crates/takt/tests/run_record.rs`
- `crates/takt/tests/scorecard.rs`
- `crates/takt/tests/scorecard_adv1.rs`
- `crates/takt/tests/scorecard_adv2.rs`
- `ess/domains/takt.citations.tsv`
- `ess/domains/takt.yaml`
