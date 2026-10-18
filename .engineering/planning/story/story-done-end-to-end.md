---
format: aep.planning-md/3
id: story:story-done-end-to-end
kind: story
status: draft
title: One ready story done end to end with the merge approval the only manual step
summary: 'The M2 proof: frontier, protocol, gate, pull request, grant, resume, merge and the store''s evidence joined on one real repository'
relations:
- decomposes: epic:composable-agentic-work
- depends_on: story:frontier-next-ready-story
- depends_on: story:forge-pull-request-evidence
- depends_on: story:merge-under-authority
- depends_on: story:run-record-and-story-evidence
- serves: vision:safe-agentic-coding
scope:
- confidence: inferred
  path: crates/takt-cli/src/main.rs
- confidence: inferred
  path: crates/takt/src/case.rs
- confidence: inferred
  path: crates/takt/src/run.rs
revision: 4
---
# Story: story-done-end-to-end

## Outcome

One ready story of a repository with a planning store is finished by `takt run` with the merge approval the only manual step: the binary takes the story, implements it under the harness, earns `tests.pass`, `gate.green`, `pr.opened` and `ci.green` from validators, stops for the grant, and after `takt approve` the next run merges and the store shows the story `implemented` with the validators' evidence on it.

## Context

`docs/design/takt.md` § 4 M2 proof: "one ready story of a small repository done with no manual step but the merge approval"; the epic's acceptance says the same and adds that every claim in the final decision document was established by a validator's evidence record, none by the harness's text. The pieces are the sibling stories: the unit from the frontier (story:frontier-next-ready-story), the protocol (story:story-delivery-protocol), the gate (story:gate-validator), the pull request and the CI read (story:forge-pull-request-evidence), the grant and the resume (story:merge-under-authority, following the cleared decision-blocker:approval-grant-and-case-resume), the record and the move to `implemented` (story:run-record-and-story-evidence). None of them proves the whole path on a real repository.

This story: the path joined and run once on a small repository with a planning store and one ready story, through the real `claude` adapter, the real `aep` adapter and the real bot-route forge adapter. The first `takt run --repo R` takes the story, loops until `ApprovalRequired (repository.merge)` and stops with the pull request open. The operator runs `takt approve <case> repository.merge` and nothing else. The second `takt run --repo R` resumes the case, merges through the bot route, stops Completed with `accepted`, and moves the story to `implemented`. What the two runs printed, the case directory and the store's evidence on the story are the proof. Whatever did not join (a seam that needs a value the design does not name, a step that needed a hand) is fixed here when it is in how the pieces are joined, or sent back to the sibling story that owns the piece.

## Domain relations

- Story → Case: at most one open case; `active` while it is open; `implemented` once a `test_result` record exists and the case terminated `accepted`. Decided in decision-blocker:approval-grant-and-case-resume; the rung inferred from aep `artifacts/lifecycles/story.yaml`.
- Case → Protocol: exactly one, `protocols/story-delivery/1.yaml`. Settled by b10x-canon 0.6.0 `ess/domains/protocol.yaml:644-651`, entity `canon.protocol.Case`, relation `protocol`.
- Claim → EvidenceRecord: every claim true in the final decision has a validator's record about `implementation` at the merged revision. Inferred from b10x-canon 0.6.0 `crates/canon/src/model/evidence.rs:30-35` and the hard rule of `docs/design/takt.md` § 3.

## Scope

- `crates/takt/src/run.rs` (what joining the path needs that no sibling owns)
- `crates/takt/src/case.rs` (the same)
- `crates/takt-cli/src/main.rs` (the same)

## Ordering

- depends_on story:frontier-next-ready-story: both edit `crates/takt/src/run.rs` and `crates/takt-cli/src/main.rs`; the frontier takes the story.
- depends_on story:forge-pull-request-evidence: both edit `crates/takt/src/case.rs` and `crates/takt-cli/src/main.rs`; the pull request and the CI read.
- depends_on story:merge-under-authority: both edit `crates/takt/src/run.rs`, `crates/takt/src/case.rs` and `crates/takt-cli/src/main.rs`; the grant and the resume.
- depends_on story:run-record-and-story-evidence: both edit `crates/takt/src/run.rs`; the evidence on the story and the move to `implemented`.

## Acceptance

A ready story of a repository with a planning store is taken by `takt run`, which stops at `ApprovalRequired (repository.merge)`; after `takt approve`, the next `takt run` resumes the case, merges through the bot route, and the story is `implemented` with the validators' evidence recorded, and no other manual step occurred.

## Checks

- The first run's last decision document shows `tests.pass`, `gate.green`, `pr.opened` and `ci.green` true and `repository.merge` approval-required, each true claim backed by a `test_result`, `gate_result`, `pull_request` or `ci_result` record about `implementation` at the pull request's head.
- The second run's last decision document shows `pr.merged` true and `accepted` legitimate, and the forge reports the pull request merged at that head.
- `aep plan artifact history <story>` shows the `test_result` and `approval` records and the move `active → implemented`, and `aep plan artifact show <story>` reads `implemented`.
- Between the first run's stop and the second run's start the only command the operator ran was `takt approve <case> repository.merge`.
- No evidence record in the case names the harness as its producer.

## Out of Scope

Several units in one run (story:declared-loops); the run from a `workflow/1` file, which is story:declared-loops' check on the story this story finishes; the parity run under the second adapter, which is story:second-harness-adapter's check on the same story; the week of runs (story:week-of-daily-runs); a fix inside a seam's adapter, which goes to the sibling that owns the adapter.

## Ambiguities

- Which repository: `inferable`, a small public repository with a planning store and one ready story that the bot route can reach; the operator picks it when the story is run, and no private name enters this repository.
- Whether a fix found here belongs here or in a sibling: `inferable`, a fix in how the pieces are joined (`run.rs`, `case.rs`, `main.rs`) stays here; a fix inside an adapter goes to the sibling that owns it.
