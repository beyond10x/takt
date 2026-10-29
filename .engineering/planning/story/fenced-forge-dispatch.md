---
format: aep.planning-md/3
id: story:fenced-forge-dispatch
kind: story
status: active
title: pr.open and repository.merge record an outcome class; unknown is resolved by a read
summary: applied, refused, not_attempted or unknown; a write that may have been sent is never sent again
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-10-29T21:22:43Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-29T21:22:43Z", actor: "human:timo", revision: 3}
---
# Story: fenced-forge-dispatch

## Outcome

`pr.open` and `repository.merge` each record an outcome class for the forge write they attempt (`applied`, `refused`, `not_attempted` or `unknown`); a write whose request may have been sent but whose answer never arrived is `unknown`, and the next step resolves it by reading the forge, never by sending the write again.

## Context

From the tool-calling brainstorm of 2026-10-18. A connector layer it compared fences every external write: the dispatch moves from prepared to dispatching by compare-and-swap so it is sent at most once, its outcome is one of applied, refused, not attempted or unknown, a write that "may have sent" is unknown, and a retry is keyed by a fingerprint of the request so it cannot duplicate.

takt's forge writes today: `Forge::open_pull_request` and `Forge::merge` (`crates/takt/src/forge.rs:293`, `forge.rs:302`; the bot-route adapter at `forge.rs:1051` and `forge.rs:1149`) run a child under a timeout and kill it past that (`forge.rs:651-676`); a timeout or a non-zero exit is an error, with no record of whether the request reached the host. The validators already guard part of this: `PrOpen` trusts a pull-request number only once the forge reports its head (`crates/takt/src/validator.rs:701-711`), and the merge step pushes before merging so "a push the forge took but reported failed is merged by the next run" (`validator.rs:1181-1200`). What is missing is the outcome as a record: a run that timed out on the merge leaves nothing that says the merge may have happened, and the next run decides from the forge's state alone.

## Scope

- `crates/takt/src/forge.rs`: the write calls return an outcome class; a timeout after the request was written is `unknown` (cited)
- `crates/takt/src/validator.rs`: `PrOpen` and the merge step record the outcome and, on `unknown`, read before writing (cited)
- `crates/takt/src/events.rs`: the outcome on the step's event (inferred)
- `ess/domains/takt.yaml`: the outcome class, if it is a new noun (inferred)
- new: `crates/takt/tests/fenced_forge_dispatch.rs` (inferred)

## Acceptance

With the in-memory forge set to apply a merge and then report a timeout, the run records `repository.merge` with outcome `unknown`, and the next run writes a `merge_observation` of `merged` after reading the pull request, with the forge's call log showing exactly one merge call across both runs.

## Ambiguities

- Whether the fingerprint for an open is the case and the revision: `inferable`, yes, since one case has one pull request.
