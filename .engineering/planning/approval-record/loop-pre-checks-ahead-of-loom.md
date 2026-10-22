---
format: aep.planning-md/3
id: approval-record:loop-pre-checks-ahead-of-loom
kind: approval-record
status: approved
title: The loop keeps takt's grant-only and unperformable checks ahead of each Loom step
relations:
- decides: story:the-loop-is-run-until-blocked
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-10-22T13:02:21Z", actor: "human:timo", revision: 2, executor: "agent:takt-dispatcher"}
---
# Approval: the loop keeps two of takt's stop checks ahead of each Loom step

## Decision

The orchestrator, under the owner's delegation (2026-10-22): "C, approved under the grant.
Pre-check before every Loom step (run.rs Shape::only_behind/unperformable), plus 1 order test,
with 0 Loom and 0 ESS change. Record in your store an approval-record that decides
story:the-loop-is-run-until-blocked. It holds the exception to the epic's no-substitute rule,
limited to these two pre-checks, citing this message and loom's finding (loom AGENTS.md §
Commission, documented contract). Note in story:takt-evaluation-code-retired that this part stays
until a Loom contract change, which is Timo's. Close dependency-blocker:loom-handed-frontier as
worked around. Propose the unit, then build it; one build at a time."

## Why Loom does not take the change

Loom's documented contract rules out both stops dependency-blocker:loom-handed-frontier asked for.
Its `AGENTS.md` § Commission (loom `main` at 14040334) fixes where the host stop is asked
(`AGENTS.md:329-338`: before an executor call whose handed frontier admits an action without a
grant), and states that "the frontier relates no action to a claim" (`AGENTS.md:378`), so Loom
cannot tell which open claim an action would decide. Changing either is a change to a Loom
contract, which is the owner's.

## The exception

`epic:takt-on-loom-sdk` § Out of scope says a story blocked by a loom-sdk gap builds no takt-side
substitute. This record makes one exception, limited to two checks takt runs on its case before
every Loom step it hands over:

| check | code | stop | exit |
|---|---|---|---|
| only a grant moves an open obligation | `Shape::only_behind` (`crates/takt/src/run.rs:913-929`) | ApprovalRequired | 4 |
| the action that decides an open claim has no performer | `Shape::unperformable` (`run.rs:9984-10004`) | Unperformable | 7 |

Nothing else of takt's own evaluation stays on the loop's path under this record. A test pins the
order: a case in either state on a low disk stops 4 or 7, never DiskLow 8, and no harness is
called. No Loom change and no ESS change.

## What follows

story:takt-evaluation-code-retired keeps the code these two checks need until Loom's contract
changes; that change is the owner's decision.
