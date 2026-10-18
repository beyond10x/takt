---
format: aep.planning-md/3
id: story:authority-hardening
kind: story
status: active
title: A grant authorises one revision and a case is held by one run
summary: Atomic lock, revision-bound grant, sanitised case ids
tags:
- wave-06
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
scope:
- confidence: cited
  path: crates/takt/src/case.rs
- confidence: cited
  path: crates/takt/src/frontier.rs
- confidence: cited
  path: crates/takt/src/run.rs
- confidence: cited
  path: crates/takt/src/validator.rs
- confidence: inferred
  path: crates/takt/tests/authority.rs
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-10-18T09:49:59Z", actor: "human:timo", revision: 7}
- {from: "proposed", to: "active", at: "2026-10-18T09:49:59Z", actor: "human:timo", revision: 8}
---
# Story: authority-hardening

## Outcome

A grant authorises exactly the revision it was given at, a case is held by one run at a time without a race, and no operator-supplied case id reaches outside the cases directory.

## Context

Findings of the 2026-10-18 code review, confirmed by independent probes:

| # | defect | where |
|---|---|---|
| 11 | `run.lock` is check-then-write, not `create_new`; two runs held one case in 272 of 300 rounds; `grant` races `hold` | `crates/takt/src/case.rs:51-70`, `case.rs:285-331`, `case.rs:651-656` |
| — | `resume` checks the lock and takes it later, a second window | `case.rs:618`, `crates/takt/src/run.rs:414` |
| 12 | a grant is not bound to a revision: commit B merges under the grant given for A; `grant` does not check the case reached the gate | `case.rs:152-161`, `case.rs:205-220`, `crates/takt/src/validator.rs:716-727` |
| 19 | `approve` writes the grant before the store record; a failed record cannot be retried | `crates/takt/src/frontier.rs:184-204`, `case.rs:316-322` |
| 26 | `approve`'s case id is not sanitised (`../`, absolute paths) | `case.rs:286` |
| — | `run --case <id>` and `RunOptions.case_id` likewise | `run.rs:255`, `case.rs:612` |
| 27 | `has_evidence(..).unwrap_or(true)`; a `.yaml` record makes every validator never due | `run.rs:90-92`, `case.rs:801`, `case.rs:819` |
| 29 | `latest_decision` sorts names lexically and breaks after 9999 | `case.rs:371`, `case.rs:851` |

`decision-blocker:approval-grant-and-case-resume` (cleared) chose a grant per case. This story narrows it to a grant per case at a revision: a new commit after the grant needs a new grant.

## Scope

- `crates/takt/src/case.rs`: the lock, `Authority`/`grant`, `resume`, `latest_decision`, `has_evidence`, the case-id check (cited)
- `crates/takt/src/frontier.rs`: `approve` (around 184-204) only (cited)
- `crates/takt/src/validator.rs`: `PrMerged` grant check (around 700-790) only (cited)
- `crates/takt/src/run.rs`: `is_due` (around 83-96) only (cited)
- new: `crates/takt/tests/authority.rs` (inferred)

## Acceptance

Two threads holding one case at once: exactly one succeeds in 300 of 300 rounds; a grant written while a run holds the case is refused; a grant records the implementation revision it was given at and `repository.merge` is not admitted at any other revision; `grant` refuses a case that is not at the gate; `approve` records on the story first and retries cleanly after a failed record; case ids containing `/`, `..` or starting with `/` are refused by `approve`, `run --case` and `RunOptions.case_id`; an unreadable evidence record stops the run naming the file; `latest_decision` returns the highest sequence number past 9999.
