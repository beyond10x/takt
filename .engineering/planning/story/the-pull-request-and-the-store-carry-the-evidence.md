---
format: aep.planning-md/3
id: story:the-pull-request-and-the-store-carry-the-evidence
kind: story
status: active
title: The pull request and the store record carry the case's evidence
summary: store test_result names result, revision, producer and sha256; the PR body summarises evidence, steps, tokens and stop
tags:
- runtime-records
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
- informed_by: story:cases-live-under-dot-takt
scope:
- confidence: cited
  path: crates/takt/src/frontier.rs
- confidence: cited
  path: crates/takt/src/run.rs
- confidence: cited
  path: crates/takt/src/validator.rs
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-27T01:44:48Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-27T01:44:48Z", actor: "human:timo", revision: 4}
---
# Story: the-pull-request-and-the-store-carry-the-evidence

## Outcome

What a reviewer reads in the pull request and in the planning store's history stands without the case directory:
the `test_result` record takt writes on a story names the case's evidence record by its result, the revision it was
observed at, its producer and its sha256, and the pull request's body carries an evidence summary of the case.

## Context

- The owner, 2026-10-27, on what a trial case put in version control: "this is difficult for me to judge whether we
  need this stuff in version control or not". The case is not in version control by default
  (story:cases-live-under-dot-takt), so what a reviewer needs must travel in the store record and the pull request.
- The store record takt writes names the case file by path only: `--source takt --ref
  .engineering/cases/<case>/evidence/<n>.json` (`crates/takt/src/run.rs:9845-9885`,
  `crates/takt/src/frontier.rs:42`, `frontier.rs:643-672`). On another clone that path names nothing.
- On the trial a person wrote the record by hand instead, and typed the summary into its source: the test command,
  "failed before fix at <sha>, passed after fix at <sha>".
- The pull request's body is the intent, the case id, the revision and the claims that hold
  (`crates/takt/src/validator.rs:2364-2380`); the case id names a directory no reviewer can open.

## Acceptance

Each item is a named test, run by `task check`.

1. `evidence_travels::store_record_names_the_evidence`: a story run to green writes a `test_result` record whose
   source reads `takt.tests-run pass at <subject_revision>` and whose reference ends in `#sha256:<hex>` of the case
   record's bytes; `aep plan artifact validate` passes.
2. `evidence_travels::pull_request_body_summarises_the_case`: the body of the pull request `pr.open` requests lists
   each evidence record of the case (kind, result, short revision, producer), the run's steps, tokens and stop as
   `events.jsonl` states them, and states "not stated" for a figure the record does not hold.
3. `evidence_travels::body_is_bounded`: a case with 500 evidence records gives a body under 60,000 characters that
   says how many records it left out.
4. `evidence_travels::body_passes_scan_text`: the body still passes the scan the bot forge runs before it opens the
   pull request (as today, `forge.rs:2440-2450`).

## Scope

- `crates/takt/src/run.rs`: the record's source and reference (cited)
- `crates/takt/src/frontier.rs`: `record_test_result` (cited)
- `crates/takt/src/validator.rs`: `body_of` (cited)
