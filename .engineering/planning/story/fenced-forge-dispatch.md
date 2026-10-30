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
- depends_on: story:bot-forge-reads-through-the-app
- depends_on: story:pr-open-not-idempotent-on-resume
- depends_on: story:forge-preflight
- depends_on: story:an-unattended-run-merges-under-a-delegated-grant
revision: 5
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

- `crates/takt/src/forge.rs`: the `Forge` trait's write calls (`:369`, `:378`) return an outcome class; a timeout after the request was written is `unknown` (cited)
- the five adapters' write calls: `forge/bot.rs:544`, `:632`; `forge/connectors.rs:508`, `:570`; `forge/gitlab.rs:728`, `:777`; `forge/local.rs:599`, `:709`; `forge/command.rs:724`, `:847` (cited)
- `crates/takt/src/validator.rs`: `PrMerged` records the outcome and, on `unknown`, reads before writing (cited)
- `crates/takt/src/events.rs`: the outcome on the step's event (inferred)
- new: `crates/takt/tests/fenced_forge_dispatch.rs` (inferred)

`pr.open`'s lost answer is story:pr-open-not-idempotent-on-resume's; this story records the
outcome class of both writes and resolves the merge's `unknown`. It comes after
story:pr-open-not-idempotent-on-resume, story:forge-preflight (both edit `forge/bot.rs`) and
story:an-unattended-run-merges-under-a-delegated-grant (both edit `PrMerged`'s surroundings in
`validator.rs`).

## Acceptance

With the in-memory forge, each case read from the step's event in `events.jsonl`:

1. a merge applied and then a timeout reported: the run records `repository.merge` with outcome
   `unknown`; the next run reads the pull request, writes a `merge_observation` of `merged`, and
   the forge's call log shows one merge call across both runs.
2. a merge the forge refuses (405): outcome `refused`, no `merge_observation`.
3. a merge not sent because the head moved: outcome `not_attempted`.
4. a `pr.open` that is applied: outcome `applied`, with the pull request's number.

## Ambiguities

- Whether the fingerprint for an open is the case and the revision: `inferable`, yes, since one case has one pull request.
